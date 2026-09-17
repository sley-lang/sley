//! Deterministic server tests (S20-410 slice B) over a trusted genesis
//! repository built through the repository test-support fixtures.

#![allow(clippy::too_many_lines, clippy::cast_possible_truncation)]

use sley_id::{SchemaEpochId, TransactionId};
use sley_query::{
    ContextCapsuleError, ContextCapsuleErrorCode, ModeledEntityKind, QueryLimits, RestrictedQuery,
    RootQuery, SnapshotContext, build_index_snapshot, build_restricted_query_request,
};
use sley_repo::test_support::{
    complete_bodies, complete_dependency_root, executable_bodies, genesis,
};
use sley_repo::{CompleteRootRequest, run_root_query};
use sley_scb1::{MAX_BYTE_PAYLOAD, encode_bytes, encode_record, encode_uvar};

use crate::server::{
    FUNCTION_UNKNOWN_DETAIL, REPORT_UNKNOWN_DETAIL, RESERVED_SEAM_370_DETAIL,
    RESERVED_SEAM_620_DETAIL, Server,
};
use crate::session::{CapsuleBindError, HeadBinding};
use crate::{
    BoundedContext, DecodedFrame, FEATURE_CANCEL, FEATURE_EXTENDED_EXECUTE, FEATURE_STREAM,
    FrameKind, Hello, LimitProfile, MAX_FRAME_BYTES, Method, PROTOCOL_VERSION, ProtocolErrorCode,
    ProtocolFailure, ProtocolFrame, SessionId, decode_frame, encode_frame, negotiate,
    negotiate_identity,
};

fn epoch(byte: u8) -> SchemaEpochId {
    SchemaEpochId::from_bytes([byte; 32])
}

/// Shared maintenance over a test repository, as production callers must
/// hold it before touching the index cache.
fn maintenance_guard(repository: &std::path::Path) -> sley_txn::RepositoryMaintenanceGuard {
    std::fs::create_dir_all(repository).unwrap();
    sley_txn::initialize_repository_maintenance(repository).unwrap();
    sley_txn::acquire_shared_repository_maintenance(repository).unwrap()
}

fn hello(methods: Vec<u32>, inflight: u32) -> Hello {
    Hello {
        protocol_versions: vec![1],
        schema_epochs: vec![epoch(0x11)],
        limits: LimitProfile {
            max_frame_bytes: 8_388_608,
            max_entities: 65_535,
            max_edges: 400_000,
            max_depth: 65_535,
            max_response_bytes: 8_388_608,
            max_work: 100_000_000,
            max_inflight: inflight,
            max_sessions: 256,
        },
        methods,
        features: 1,
        adapters: vec![],
        effects: vec![],
    }
}

fn all_methods() -> Vec<u32> {
    Method::ALL
        .iter()
        .filter(|method| !method.is_reserved())
        .map(|method| method.tag())
        .collect()
}

struct Harness {
    _temp: sley_repo::test_support::TempDir,
    repository: std::path::PathBuf,
    genesis: TransactionId,
    client_hello: Hello,
    server_hello: Hello,
    server: Server,
    session: SessionId,
    next_request: u64,
}

impl Harness {
    fn new(label: &str) -> Self {
        let client_hello = hello(all_methods(), 4);
        Self::with_hellos(label, client_hello, hello(all_methods(), 8))
    }

    fn with_hellos(label: &str, client_hello: Hello, server_hello: Hello) -> Self {
        let (temp, _transactions, genesis) =
            genesis(label, complete_bodies(), &[complete_dependency_root()]);
        let repository = temp.child("repo");
        let mut server = Server::new(&repository, &client_hello, &server_hello).unwrap();
        let handshake = server.handshake_id();
        let open = server
            .answer(&request_frame(
                None,
                0,
                Method::SessionOpen,
                handshake.as_bytes().to_vec(),
            ))
            .unwrap();
        assert!(!open.failed);
        let (DecodedFrame::Response(frame), _) =
            decode_frame(&open.frame.bytes, MAX_FRAME_BYTES).unwrap()
        else {
            panic!("session open response");
        };
        let session = SessionId::from_bytes(frame.body.as_slice().try_into().unwrap());
        Self {
            _temp: temp,
            repository,
            genesis,
            client_hello,
            server_hello,
            server,
            session,
            next_request: 1,
        }
    }

    fn call(&mut self, method: Method, body: Vec<u8>) -> (bool, ProtocolFrame) {
        let answer = self
            .server
            .answer(&request_frame(
                Some(self.session),
                self.next_request,
                method,
                body,
            ))
            .unwrap();
        self.next_request += 1;
        let (DecodedFrame::Response(frame), _) =
            decode_frame(&answer.frame.bytes, MAX_FRAME_BYTES).unwrap()
        else {
            panic!("response frame");
        };
        assert_eq!(frame.method, method.tag());
        assert_eq!(frame.session, Some(self.session));
        (answer.failed, frame)
    }

    fn ok(&mut self, method: Method, body: Vec<u8>) -> ProtocolFrame {
        let (failed, frame) = self.call(method, body);
        assert!(
            !failed,
            "{method:?} failed: {:?}",
            ProtocolFailure::decode(&frame.body)
        );
        frame
    }

    /// The repository's current accepted head root, for naming in
    /// `handle.expand` requests (contract section 4).
    fn head_root(&self) -> sley_id::StateRoot {
        sley_txn::TransactionRepository::new(&self.repository)
            .accepted_head()
            .unwrap()
            .verified_revision()
            .state_root()
            .root
    }

    /// A `handle.expand` request body naming position and expected root.
    fn expand_body(position: u64, root: sley_id::StateRoot) -> Vec<u8> {
        let mut body = encode_uvar(position);
        body.extend_from_slice(root.as_bytes());
        body
    }

    fn fail(&mut self, method: Method, body: Vec<u8>) -> ProtocolFailure {
        let (failed, frame) = self.call(method, body);
        assert!(failed, "{method:?} unexpectedly succeeded");
        ProtocolFailure::decode(&frame.body).unwrap()
    }
}

fn request_frame(
    session: Option<SessionId>,
    request_id: u64,
    method: Method,
    body: Vec<u8>,
) -> Vec<u8> {
    encode_frame(&ProtocolFrame {
        protocol_version: PROTOCOL_VERSION,
        session,
        request_id,
        kind: FrameKind::Request,
        method: method.tag(),
        flags: 0,
        bounds: BoundedContext::none(),
        body,
    })
    .unwrap()
    .bytes
}

fn tx(id: TransactionId) -> Vec<u8> {
    id.as_bytes().to_vec()
}

/// A complete root without the dependency binding, exportable as a pack.
fn dependency_free_bodies() -> Vec<(u8, sley_mutate::value::EntityBodyValue)> {
    use sley_mutate::value::{EntityBodyValue, PackageBody};
    use sley_repo::test_support::{id, set};
    complete_bodies()
        .into_iter()
        .filter(|(byte, _)| *byte != 17)
        .map(|(byte, body)| {
            if byte == 3 {
                (
                    byte,
                    EntityBodyValue::Package(PackageBody {
                        workspace: id(1),
                        root_namespace: id(4),
                        dependencies: set(&[]),
                        exports: set(&[6]),
                    }),
                )
            } else {
                (byte, body)
            }
        })
        .collect()
}

#[test]
fn owner_retryability_is_explicit_and_word_order_independent() {
    use crate::Retryability;
    use crate::server::owner_retryability;
    for symbol in [
        "REF_CAS_STALE",
        "REF_NAMED_CAS_STALE",
        "SESSION_ROOT_ADVANCED",
        "SESSION_STALE_HANDLE",
        "STALE_ROOT",
    ] {
        assert_eq!(
            owner_retryability(symbol),
            Retryability::AfterRequery,
            "{symbol} is retryable after a requery"
        );
    }
    for symbol in [
        "SCB_RESOURCE_LIMIT",
        "VM_EXEC_RESOURCE_LIMIT",
        "QUERY_REQUIRED_FACT_OMITTED",
        "PROTOCOL_LIMIT_EXCEEDED",
    ] {
        assert_eq!(
            owner_retryability(symbol),
            Retryability::AfterLimitChange,
            "{symbol} is retryable after a limit change"
        );
    }
    // Anything unlisted stays fail closed, including a merge conflict, which
    // is a result rather than a condition a retry can clear. The limit
    // list is explicit, never a suffix rule: a future limit symbol stays
    // fail closed until it is listed.
    for symbol in [
        "MERGE_CONFLICT_DIGEST_MISMATCH",
        "TYPE_DEPTH_LIMIT",
        "CAP_EXPIRED",
        "GC_ROOT_MISSING",
        "FUTURE_RESOURCE_LIMIT",
        "FUTURE_REQUIRED_FACT_OMITTED",
    ] {
        assert_eq!(owner_retryability(symbol), Retryability::Never, "{symbol}");
    }
}

#[test]
fn exchange_export_transports_the_pack_of_a_dependency_free_root() {
    let (temp, _transactions, genesis_id) = genesis("smp1-export", dependency_free_bodies(), &[]);
    let repository = temp.child("repo");
    let client_hello = hello(all_methods(), 4);
    let server_hello = hello(all_methods(), 8);
    let mut server = Server::new(&repository, &client_hello, &server_hello).unwrap();
    let handshake = server.handshake_id();
    let open = server
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            handshake.as_bytes().to_vec(),
        ))
        .unwrap();
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&open.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    let session = SessionId::from_bytes(frame.body.as_slice().try_into().unwrap());
    // The exchange lists visible branches, so one named branch exists first.
    let created = server
        .answer(&request_frame(
            Some(session),
            1,
            Method::BranchCreate,
            encode_record(&[(1, b"main".to_vec()), (2, tx(genesis_id))]).unwrap(),
        ))
        .unwrap();
    assert!(!created.failed);
    let answer = server
        .answer(&request_frame(
            Some(session),
            2,
            Method::ExchangeExport,
            Vec::new(),
        ))
        .unwrap();
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&answer.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert!(
        !answer.failed,
        "export failed: {:?}",
        ProtocolFailure::decode(&frame.body)
    );
    assert_eq!(&frame.body[..8], b"SLEYSCB1");
    assert_eq!(frame.bounds.returned_bytes, frame.body.len() as u64);
    let again = server
        .answer(&request_frame(
            Some(session),
            3,
            Method::ExchangeExport,
            Vec::new(),
        ))
        .unwrap();
    let (DecodedFrame::Response(second), _) =
        decode_frame(&again.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert_eq!(second.body, frame.body, "exports are byte-identical");
}

#[test]
fn session_repository_and_transaction_methods_answer_deterministically() {
    let mut harness = Harness::new("smp1-server");
    let genesis = harness.genesis;
    // Session family.
    let capabilities = harness.ok(Method::SessionCapabilities, Vec::new());
    assert_eq!(
        capabilities.body,
        harness.server.profile().preimage().unwrap()
    );
    let budgets = harness.ok(Method::SessionBudgets, Vec::new());
    assert!(!budgets.body.is_empty());
    let renewed = harness.ok(Method::SessionRenew, harness.session.as_bytes().to_vec());
    assert_eq!(renewed.body, harness.session.as_bytes().to_vec());
    // Repository family over the genesis.
    let empty = harness.ok(Method::RefsList, encode_uvar(16));
    assert_eq!(empty.bounds.returned_entities, 0);
    let created = harness.ok(
        Method::BranchCreate,
        encode_record(&[(1, b"main".to_vec()), (2, tx(genesis))]).unwrap(),
    );
    assert_eq!(created.body, encode_uvar(1));
    let present = harness.ok(
        Method::BranchCreate,
        encode_record(&[(1, b"main".to_vec()), (2, tx(genesis))]).unwrap(),
    );
    assert_eq!(present.body, encode_uvar(3));
    let advanced = harness.ok(
        Method::BranchAdvance,
        encode_record(&[(1, b"main".to_vec()), (2, tx(genesis)), (3, tx(genesis))]).unwrap(),
    );
    assert_eq!(
        advanced.body,
        encode_uvar(3),
        "same head is present, not advanced"
    );
    let listed = harness.ok(Method::RefsList, encode_uvar(16));
    assert_eq!(listed.bounds.returned_entities, 1);
    let resolved = harness.ok(Method::RefsResolve, b"main".to_vec());
    assert!(resolved.body.windows(4).any(|window| window == b"main"));
    let missing = harness.fail(Method::RefsResolve, b"other".to_vec());
    assert!(missing.symbol.starts_with("REF_"), "{}", missing.symbol);
    let revision = harness.ok(Method::RevisionRead, tx(genesis));
    assert_eq!(revision.bounds.returned_entities, 1);
    let unknown = harness.fail(
        Method::RevisionRead,
        tx(TransactionId::from_bytes([0xEE; 32])),
    );
    assert!(
        !unknown.symbol.is_empty() && !unknown.symbol.starts_with("PROTOCOL_"),
        "the owner's code is preserved: {}",
        unknown.symbol
    );
    assert_ne!(unknown.code, 0, "the owner's numeric code is preserved");
    let delta = harness.ok(
        Method::Compare,
        encode_record(&[(1, tx(genesis)), (2, tx(genesis))]).unwrap(),
    );
    assert_eq!(&delta.body[..8], b"SLEYSCB1");
    assert_eq!(
        delta.bounds.returned_entities, 0,
        "a root compared to itself has no entity deltas"
    );
    let merged = harness.ok(
        Method::MergeJudge,
        encode_record(&[(1, tx(genesis)), (2, tx(genesis)), (3, tx(genesis))]).unwrap(),
    );
    assert_eq!(merged.body[0], 1, "three equal roots merge");
    // The complete fixture binds a dependency root that no packed root
    // provides, so the pack owner rejects the export and its code is kept.
    let refused = harness.fail(Method::ExchangeExport, Vec::new());
    assert_eq!(refused.symbol, "PACK_ROOT_INVALID");
    let recovered_refs = harness.ok(Method::RefsRecover, Vec::new());
    assert_eq!(recovered_refs.bounds.returned_entities, 1);
    // Transaction family.
    let receipt = harness.ok(Method::ReceiptRead, tx(genesis));
    assert_eq!(&receipt.body[..8], b"SLEYRCP1");
    let checkout = harness.ok(Method::Checkout, tx(genesis));
    assert_eq!(checkout.bounds.returned_entities, 7);
    let recovery = harness.ok(Method::Recovery, Vec::new());
    assert!(!recovery.body.is_empty());
    let cancel = harness.ok(Method::Cancel, encode_uvar(3));
    assert!(cancel.body.is_empty());
    // Slice C methods decode their bodies before any engine runs; reserved
    // methods fail with the versioned reason.
    let malformed_gc = harness.fail(Method::GcDryRun, Vec::new());
    assert_eq!(
        malformed_gc.code,
        ProtocolErrorCode::PayloadInvalid.numeric()
    );
    // This genesis names a dependency root the repository does not hold, so
    // the server-derived snapshot fails closed inside the S20-180 owner.
    let no_pins = encode_record(&[(1, sley_scb1::encode_list(&[]).unwrap())]).unwrap();
    let dependency_missing = harness.fail(Method::GcDryRun, no_pins);
    assert_eq!(dependency_missing.symbol, "GC_DEPENDENCY_MISSING");
    let reserved = harness.fail(Method::Diagnostics, Vec::new());
    assert_eq!(reserved.details, RESERVED_SEAM_620_DETAIL);
    // Malformed bodies fail before any engine runs.
    let malformed = harness.fail(Method::RevisionRead, vec![1, 2, 3]);
    assert_eq!(malformed.code, ProtocolErrorCode::PayloadInvalid.numeric());

    // A second server instance over the same repository issues a
    // different identity for the same ordinal: the per-instance nonce
    // keeps identity spaces disjoint (contract section 1, threat T56).
    // The first server's live name is unknown to the second, and a
    // revision read under the second server's own session answers byte
    // for byte.
    let mut again = Server::new(
        &harness.repository,
        &harness.client_hello,
        &harness.server_hello,
    )
    .unwrap();
    let open = again
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            again.handshake_id().as_bytes().to_vec(),
        ))
        .unwrap();
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&open.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert_ne!(
        frame.body,
        harness.session.as_bytes().to_vec(),
        "twin instances never share an identity space"
    );
    let twin_session = SessionId::from_bytes(frame.body.as_slice().try_into().unwrap());
    let foreign = again
        .answer(&request_frame(
            Some(harness.session),
            2,
            Method::RevisionRead,
            tx(genesis),
        ))
        .unwrap();
    assert!(foreign.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&foreign.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().symbol,
        "SESSION_UNKNOWN"
    );
    for index in 0..128_u64 {
        let request_id = 3 + index;
        let first = again
            .answer(&request_frame(
                Some(twin_session),
                request_id,
                Method::RevisionRead,
                tx(genesis),
            ))
            .unwrap();
        let (DecodedFrame::Response(frame), _) =
            decode_frame(&first.frame.bytes, MAX_FRAME_BYTES).unwrap()
        else {
            panic!();
        };
        assert_eq!(frame.body, revision.body);
    }
}

#[test]
fn query_family_transports_the_frozen_engine_records() {
    let mut harness = Harness::new("smp1-query");
    let transactions = sley_txn::TransactionRepository::new(&harness.repository);
    let revision = transactions.verified_revision(harness.genesis).unwrap();
    let limits = QueryLimits::profile_maximum();
    // query.root: the request preimage of the S20-310 engine is the body.
    let outcome = run_root_query(
        &harness.repository,
        &revision,
        &maintenance_guard(&harness.repository),
        RootQuery::GetRootSummary,
        limits,
        false,
        None,
    )
    .unwrap();
    let summary = harness.ok(Method::QueryRoot, outcome.request.preimage().to_vec());
    assert_eq!(summary.body, outcome.response.record());
    assert_eq!(summary.bounds.returned_entities, 1);
    assert!(!summary.bounds.truncated);
    // A paged class with continuation.
    let paged = QueryLimits {
        max_returned_entities: 1,
        ..limits
    };
    let first = run_root_query(
        &harness.repository,
        &revision,
        &maintenance_guard(&harness.repository),
        RootQuery::ListEntitiesByKind {
            kind: ModeledEntityKind::Namespace,
        },
        paged,
        true,
        None,
    )
    .unwrap();
    let page = harness.ok(Method::QueryRoot, first.request.preimage().to_vec());
    assert!(page.bounds.truncated && page.bounds.continuation);
    assert_eq!(page.bounds.omitted, 1);
    let second = run_root_query(
        &harness.repository,
        &revision,
        &maintenance_guard(&harness.repository),
        RootQuery::ListEntitiesByKind {
            kind: ModeledEntityKind::Namespace,
        },
        paged,
        true,
        first.response.next_after(),
    )
    .unwrap();
    let continued = harness.ok(Method::QueryContinue, second.request.preimage().to_vec());
    assert!(!continued.bounds.truncated);
    let not_continuation = harness.fail(Method::QueryContinue, outcome.request.preimage().to_vec());
    assert_eq!(
        not_continuation.code,
        ProtocolErrorCode::PayloadInvalid.numeric()
    );
    // capsule.
    let capsule = harness.ok(Method::Capsule, outcome.request.preimage().to_vec());
    assert_eq!(&capsule.body[..8], b"SLEYCCP1");
    // A request bound to another snapshot is the owner's mismatch.
    let mut foreign = outcome.request.preimage().to_vec();
    foreign[16] ^= 1;
    let mismatch = harness.fail(Method::QueryRoot, foreign);
    assert_eq!(mismatch.symbol, "QUERY_SNAPSHOT_MISMATCH");
    assert_eq!(mismatch.code, 31_003);
    // query.restricted over the head's restricted kinds.
    let request = CompleteRootRequest::extract(&revision).unwrap();
    let borrowed = request.borrowed();
    let restricted: Vec<_> = borrowed
        .iter()
        .filter(|e| e.kind().restricted_kind())
        .copied()
        .collect();
    let snapshot = build_index_snapshot(
        SnapshotContext {
            schema_epoch: request.schema_epoch_id(),
            claimed_root_context: Some(request.root()),
        },
        &restricted,
    )
    .unwrap();
    let typedef = restricted[0].entity_id();
    let restricted_request = build_restricted_query_request(
        &snapshot,
        RestrictedQuery::GetModeledEntityKind { entity: typedef },
        limits,
    )
    .unwrap();
    let answer = harness.ok(
        Method::QueryRestricted,
        restricted_request.preimage().to_vec(),
    );
    assert_eq!(&answer.body[..8], b"SLEYQRS1");
    assert_eq!(answer.bounds.returned_entities, 1);
    // Garbage query bodies fail as payloads.
    let garbage = harness.fail(Method::QueryRoot, b"not a query".to_vec());
    assert_eq!(garbage.code, ProtocolErrorCode::PayloadInvalid.numeric());
}

#[test]
fn identity_session_and_frame_rules_hold_at_the_server() {
    let mut harness = Harness::new("smp1-rules");
    let genesis = harness.genesis;
    // Reused identifier: identifier 1 was admitted by the first call.
    harness.ok(Method::RevisionRead, tx(genesis));
    let reused = harness
        .server
        .answer(&request_frame(
            Some(harness.session),
            1,
            Method::RevisionRead,
            tx(genesis),
        ))
        .unwrap();
    assert!(reused.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&reused.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::RequestIdConflict.numeric()
    );
    // Unknown session: liveness precedes admission, so the name is
    // refused as a session with SESSION_UNKNOWN.
    let unknown = harness
        .server
        .answer(&request_frame(
            Some(SessionId::from_bytes([9; 32])),
            1,
            Method::RevisionRead,
            tx(genesis),
        ))
        .unwrap();
    assert!(unknown.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&unknown.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert_eq!(ProtocolFailure::decode(&frame.body).unwrap().code, 33_000);
    // Wrong handshake identity at open is a downgrade.
    let downgrade = harness
        .server
        .answer(&request_frame(None, 0, Method::SessionOpen, vec![0; 32]))
        .unwrap();
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&downgrade.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::Downgrade.numeric()
    );
    // Frame-level garbage is answered without a session.
    let garbage = harness.server.answer(b"garbage").unwrap();
    assert!(garbage.failed);
    assert_eq!(garbage.session, None);
    assert_eq!(garbage.request_id, 0);
    // A report identity nothing was stored under is a payload failure with
    // the appendix C detail.
    let (failed, frame) = harness.call(Method::Report, vec![0; 32]);
    assert!(failed);
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().details,
        REPORT_UNKNOWN_DETAIL
    );
    // Close, then everything fails closed.
    harness.ok(Method::SessionClose, Vec::new());
    let closed = harness.fail(Method::RevisionRead, tx(genesis));
    assert_eq!(closed.code, ProtocolErrorCode::SessionClosed.numeric());
}

#[test]
fn mutation_side_methods_dispatch_with_owner_codes_preserved() {
    let mut harness = Harness::new("smp1-mutation");
    let genesis_id = harness.genesis;
    let transactions = sley_txn::TransactionRepository::new(&harness.repository);
    let revision = transactions.verified_revision(genesis_id).unwrap();
    // workspace.open summarises the accepted head.
    let opened = harness.ok(Method::WorkspaceOpen, Vec::new());
    assert_eq!(&opened.body[..0], b"");
    assert_eq!(opened.bounds.returned_entities, 1);
    // workspace.create over the genesis inputs into a fresh repository
    // yields the same transaction identity (trusted genesis is exact).
    let fresh = sley_repo::test_support::TempDir::new("smp1-create");
    let fresh_root = fresh.child("repo");
    std::fs::create_dir(&fresh_root).unwrap();
    let mut other = Server::new(&fresh_root, &harness.client_hello, &harness.server_hello).unwrap();
    // A repository without an accepted head cannot bind a session yet.
    let premature = other
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            other.handshake_id().as_bytes().to_vec(),
        ))
        .unwrap();
    assert!(premature.failed, "no head, no session");
    let objects: Vec<Vec<u8>> = revision
        .objects()
        .iter()
        .map(|o| o.stored_bytes().to_vec())
        .collect();
    let body = encode_record(&[
        (1, revision.state_root().stored_bytes.clone()),
        (2, revision.policy_root().stored_bytes().to_vec()),
        (3, sley_scb1::encode_list(&objects).unwrap()),
        (4, sley_scb1::encode_list(&[]).unwrap()),
    ])
    .unwrap();
    let created = other
        .answer(&request_frame(
            None,
            0,
            Method::WorkspaceCreate,
            body.clone(),
        ))
        .unwrap();
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&created.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert!(
        !created.failed,
        "{:?}",
        ProtocolFailure::decode(&frame.body)
    );
    assert_eq!(
        frame.body,
        tx(genesis_id),
        "trusted genesis over identical inputs is one identity"
    );
    // Replay semantics (contract section 3): once a head exists the
    // session-less path is closed, and a replay with a session runs the
    // owner, which refuses the second genesis.
    let replayed = other
        .answer(&request_frame(
            None,
            0,
            Method::WorkspaceCreate,
            body.clone(),
        ))
        .unwrap();
    assert!(replayed.failed);
    let (DecodedFrame::Response(replay_frame), _) =
        decode_frame(&replayed.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert_eq!(
        ProtocolFailure::decode(&replay_frame.body).unwrap().symbol,
        "SESSION_BINDING_INVALID"
    );
    let open = other
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            other.handshake_id().as_bytes().to_vec(),
        ))
        .unwrap();
    assert!(!open.failed);
    let (DecodedFrame::Response(open_frame), _) =
        decode_frame(&open.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    let replay_session = SessionId::from_bytes(open_frame.body.as_slice().try_into().unwrap());
    let (failed, owned) = call_frame(&mut other, replay_session, 1, Method::WorkspaceCreate, body);
    assert!(failed);
    assert_eq!(
        ProtocolFailure::decode(&owned.body).unwrap().symbol,
        "TXN_ALREADY_INITIALIZED"
    );
    // candidate.create with a structurally empty record keeps the owner's code.
    let empty_candidate = harness.fail(Method::CandidateCreate, vec![0]);
    assert!(
        empty_candidate.symbol.starts_with("CANDIDATE_")
            || empty_candidate.symbol.starts_with("SCB_"),
        "{}",
        empty_candidate.symbol
    );
    // candidate.inspect and discard over garbage bytes keep the owner's code.
    let inspect = harness.fail(Method::CandidateInspect, b"not a candidate".to_vec());
    assert!(
        !inspect.symbol.starts_with("PROTOCOL_"),
        "{}",
        inspect.symbol
    );
    let discard = harness.fail(Method::CandidateDiscard, b"not a candidate".to_vec());
    assert_eq!(discard.symbol, inspect.symbol);
    // candidate.validate and commit reach the owners with the base revision.
    let principal = sley_repo::test_support::fixed(2, sley_id::PrincipalId::from_bytes);
    let validate_body = encode_record(&[
        (1, tx(genesis_id)),
        (2, principal.as_bytes().to_vec()),
        (3, encode_uvar(1_000)),
        (4, b"not a candidate".to_vec()),
    ])
    .unwrap();
    // The S20-360 validator renders every outcome as a canonical result
    // record, so garbage candidate bytes yield a result, not a failure.
    let validated = harness.ok(Method::CandidateValidate, validate_body);
    assert_eq!(&validated.body[..8], b"SLEYCRS1");
    assert_eq!(validated.bounds.returned_entities, 1);
    let commit_body = encode_record(&[
        (1, tx(genesis_id)),
        (2, principal.as_bytes().to_vec()),
        (3, encode_uvar(1_000)),
        (4, b"not a candidate".to_vec()),
    ])
    .unwrap();
    let committed = harness.fail(Method::Commit, commit_body);
    assert!(
        !committed.symbol.starts_with("PROTOCOL_"),
        "{}",
        committed.symbol
    );
    assert_ne!(
        committed.code, 0,
        "commit failures carry the owner's numeric code"
    );
    // merge.commit of three equal roots reaches the merge owner; an empty
    // plan is the owner's decision, not the transport's.
    let merge_body = encode_record(&[
        (1, tx(genesis_id)),
        (2, tx(genesis_id)),
        (3, tx(genesis_id)),
        (4, principal.as_bytes().to_vec()),
        (5, encode_uvar(1_000)),
        (6, encode_uvar(2_000)),
        (7, b"main".to_vec()),
    ])
    .unwrap();
    let (merge_failed, merge_frame) = harness.call(Method::MergeCommit, merge_body);
    if merge_failed {
        let failure = ProtocolFailure::decode(&merge_frame.body).unwrap();
        assert!(
            failure.symbol.starts_with("MERGE_")
                || failure.symbol.starts_with("REF_")
                || failure.symbol.starts_with("TXN_"),
            "{}",
            failure.symbol
        );
    } else {
        assert_eq!(merge_frame.body[0], 1);
    }
    // exchange.import round trip: export from a dependency-free repository,
    // import into a fresh one, and the accepted head is the source genesis.
    let (source_temp, _source_transactions, source_genesis) =
        genesis("smp1-import-source", dependency_free_bodies(), &[]);
    let source_root = source_temp.child("repo");
    let mut source =
        Server::new(&source_root, &harness.client_hello, &harness.server_hello).unwrap();
    let source_handshake = source.handshake_id();
    let open = source
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            source_handshake.as_bytes().to_vec(),
        ))
        .unwrap();
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&open.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    let source_session = SessionId::from_bytes(frame.body.as_slice().try_into().unwrap());
    let created = source
        .answer(&request_frame(
            Some(source_session),
            1,
            Method::BranchCreate,
            encode_record(&[(1, b"main".to_vec()), (2, tx(source_genesis))]).unwrap(),
        ))
        .unwrap();
    assert!(!created.failed);
    let exported = source
        .answer(&request_frame(
            Some(source_session),
            2,
            Method::ExchangeExport,
            Vec::new(),
        ))
        .unwrap();
    let (DecodedFrame::Response(export_frame), _) =
        decode_frame(&exported.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert!(!exported.failed);
    let target_temp = sley_repo::test_support::TempDir::new("smp1-import-target");
    let target_root = target_temp.child("repo");
    let mut target =
        Server::new(&target_root, &harness.client_hello, &harness.server_hello).unwrap();
    let imported = target
        .answer(&request_frame(
            None,
            0,
            Method::ExchangeImport,
            export_frame.body.clone(),
        ))
        .unwrap();
    let (DecodedFrame::Response(import_frame), _) =
        decode_frame(&imported.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert!(
        !imported.failed,
        "{:?}",
        ProtocolFailure::decode(&import_frame.body)
    );
    assert!(
        import_frame
            .body
            .windows(32)
            .any(|window| window == source_genesis.as_bytes())
    );
    let target_handshake = target.handshake_id();
    let open = target
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            target_handshake.as_bytes().to_vec(),
        ))
        .unwrap();
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&open.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert!(!open.failed, "{:?}", ProtocolFailure::decode(&frame.body));
    let target_session = SessionId::from_bytes(frame.body.as_slice().try_into().unwrap());
    let opened = target
        .answer(&request_frame(
            Some(target_session),
            1,
            Method::WorkspaceOpen,
            Vec::new(),
        ))
        .unwrap();
    assert!(!opened.failed);
    // candidate.append composes two candidate records through the owner's
    // codec; malformed parts keep the owner's code.
    let append_body = encode_record(&[(1, b"not a candidate".to_vec()), (2, vec![0])]).unwrap();
    let appended = harness.fail(Method::CandidateAppend, append_body);
    assert!(
        !appended.symbol.starts_with("PROTOCOL_"),
        "{}",
        appended.symbol
    );
    let malformed_append = harness.fail(Method::CandidateAppend, vec![7]);
    assert_eq!(
        malformed_append.code,
        ProtocolErrorCode::PayloadInvalid.numeric()
    );
    // Execute decodes its body before any engine runs.
    let malformed_execute = harness.fail(Method::Execute, Vec::new());
    assert_eq!(
        malformed_execute.code,
        ProtocolErrorCode::PayloadInvalid.numeric()
    );
}

#[test]
fn cancellation_streaming_and_budgets_are_bounded_at_the_server() {
    let mut harness = Harness::new("smp1-440");
    let genesis_id = harness.genesis;
    let session = harness.session;
    // A cancel in the same batch that precedes execution wins; the target
    // never runs and answers PROTOCOL_CANCELLED under its own identifier.
    let target = request_frame(Some(session), 1, Method::RevisionRead, tx(genesis_id));
    let cancel = request_frame(Some(session), 2, Method::Cancel, encode_uvar(1));
    let answers = harness.server.answer_batch(&[&target, &cancel]).unwrap();
    assert_eq!(answers.len(), 2);
    assert!(answers[0].failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&answers[0].frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::Cancelled.numeric()
    );
    assert!(!answers[1].failed, "the cancel itself acknowledges");
    // The cancel flag on the request frame has the same effect.
    let mut flagged = ProtocolFrame {
        protocol_version: PROTOCOL_VERSION,
        session: Some(session),
        request_id: 3,
        kind: FrameKind::Request,
        method: Method::RevisionRead.tag(),
        flags: crate::FLAG_CANCEL,
        bounds: BoundedContext::none(),
        body: tx(genesis_id),
    };
    let flagged_bytes = encode_frame(&flagged).unwrap().bytes;
    let answers = harness.server.answer_batch(&[&flagged_bytes]).unwrap();
    assert!(answers[0].failed);
    flagged.flags = 0;
    // A cancel naming an already answered request acknowledges.
    harness.next_request = 4;
    let ack = harness.ok(Method::Cancel, encode_uvar(1));
    assert!(ack.body.is_empty());
    // Identifiers stay strictly increasing across cancelled requests: the
    // next identifier after the cancelled ones is admitted normally.
    let next = harness.ok(Method::RevisionRead, tx(genesis_id));
    assert_eq!(next.bounds.returned_entities, 1);

    // Streaming: a profile with a small ceiling and the stream feature.
    let mut small_client = hello(all_methods(), 4);
    small_client.limits.max_frame_bytes = 640;
    small_client.features = 1 | 2;
    let mut small_server = hello(all_methods(), 8);
    small_server.features = 1 | 2;
    let profile = negotiate(&small_client, &small_server).unwrap();
    assert_eq!(profile.limits.max_frame_bytes, 640);
    let mut streaming = Server::new(&harness.repository, &small_client, &small_server).unwrap();
    let open = streaming
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            streaming.handshake_id().as_bytes().to_vec(),
        ))
        .unwrap();
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&open.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    let stream_session = SessionId::from_bytes(frame.body.as_slice().try_into().unwrap());
    let checkout = streaming
        .answer(&request_frame(
            Some(stream_session),
            1,
            Method::Checkout,
            tx(genesis_id),
        ))
        .unwrap();
    assert!(!checkout.failed);
    assert!(
        !checkout.events.is_empty(),
        "the checkout body exceeds the ceiling and streams"
    );
    for event in &checkout.events {
        assert!(event.bytes.len() as u64 <= 640);
    }
    let mut frames: Vec<ProtocolFrame> = checkout
        .events
        .iter()
        .chain(core::iter::once(&checkout.frame))
        .map(
            |encoded| match decode_frame(&encoded.bytes, 640).unwrap().0 {
                DecodedFrame::Response(frame) => frame,
                _ => panic!("stream frame"),
            },
        )
        .collect();
    let reassembled = crate::reassemble_stream(&frames).unwrap();
    // The reassembled body equals the unstreamed answer of the large-ceiling server.
    let plain = harness.ok(Method::Checkout, tx(genesis_id));
    assert_eq!(reassembled.body, plain.body);
    assert_eq!(reassembled.bounds.returned_entities, 7);
    frames.pop();
    assert_eq!(
        crate::reassemble_stream(&frames).unwrap_err().code(),
        ProtocolErrorCode::FrameInvalid
    );
    // Without the stream feature the same body fails closed with no partial body.
    let mut no_stream_client = small_client.clone();
    no_stream_client.features = 1;
    let mut refusing = Server::new(&harness.repository, &no_stream_client, &small_server).unwrap();
    let open = refusing
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            refusing.handshake_id().as_bytes().to_vec(),
        ))
        .unwrap();
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&open.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    let refusing_session = SessionId::from_bytes(frame.body.as_slice().try_into().unwrap());
    let refused = refusing
        .answer(&request_frame(
            Some(refusing_session),
            1,
            Method::Checkout,
            tx(genesis_id),
        ))
        .unwrap();
    assert!(refused.failed && refused.events.is_empty());
    let (DecodedFrame::Response(frame), _) = decode_frame(&refused.frame.bytes, 640).unwrap()
    else {
        panic!();
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::LimitExceeded.numeric()
    );

    // Budgets: a session whose work budget is exhausted fails closed.
    let mut tiny_client = hello(all_methods(), 4);
    tiny_client.limits.max_work = 64;
    let tiny_server = hello(all_methods(), 8);
    let mut budgeted = Server::new(&harness.repository, &tiny_client, &tiny_server).unwrap();
    let open = budgeted
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            budgeted.handshake_id().as_bytes().to_vec(),
        ))
        .unwrap();
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&open.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    let budget_session = SessionId::from_bytes(frame.body.as_slice().try_into().unwrap());
    assert_eq!(budgeted.remaining_budget(budget_session), Some(64));
    let first = budgeted
        .answer(&request_frame(
            Some(budget_session),
            1,
            Method::SessionRenew,
            budget_session.as_bytes().to_vec(),
        ))
        .unwrap();
    assert!(!first.failed);
    assert_eq!(budgeted.remaining_budget(budget_session), Some(64 - 33));
    let second = budgeted
        .answer(&request_frame(
            Some(budget_session),
            2,
            Method::SessionRenew,
            budget_session.as_bytes().to_vec(),
        ))
        .unwrap();
    assert!(!second.failed);
    assert_eq!(budgeted.remaining_budget(budget_session), Some(0));
    let exhausted = budgeted
        .answer(&request_frame(
            Some(budget_session),
            3,
            Method::SessionRenew,
            budget_session.as_bytes().to_vec(),
        ))
        .unwrap();
    assert!(exhausted.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&exhausted.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::LimitExceeded.numeric()
    );
}

#[test]
fn sessions_bind_workspace_root_and_epoch_and_handles_name_their_root() {
    use sley_repo::test_support::{TempDir, genesis_in_workspace};
    let mut harness = Harness::new("smp1-330");
    let genesis_id = harness.genesis;
    let session = harness.session;
    // Instance separation: a second server over the same state issues a
    // different identity for the same ordinal (contract section 1).
    let mut twin = Server::new(
        &harness.repository,
        &harness.client_hello,
        &harness.server_hello,
    )
    .unwrap();
    let open = twin
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            twin.handshake_id().as_bytes().to_vec(),
        ))
        .unwrap();
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&open.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert_ne!(
        frame.body,
        session.as_bytes().to_vec(),
        "twin instances never share an identity space"
    );
    // A positional handle expands under the bound root it names.
    let root = harness.head_root();
    let expanded = harness.ok(Method::HandleExpand, Harness::expand_body(1, root));
    assert_eq!(expanded.bounds.returned_entities, 1);
    assert!(
        expanded
            .body
            .windows(32)
            .any(|window| window == session.as_bytes())
    );
    // Naming a root the session does not bind is stale immediately.
    let foreign_root = Harness::expand_body(1, sley_id::StateRoot::from_bytes([0x77; 32]));
    let wrong = harness.fail(Method::HandleExpand, foreign_root);
    assert_eq!(wrong.symbol, "SESSION_STALE_HANDLE");
    let unknown = harness.fail(Method::HandleExpand, Harness::expand_body(7, root));
    assert_eq!(unknown.symbol, "SESSION_HANDLE_UNKNOWN");
    assert_eq!(unknown.code, 33_005);
    // The head advances: another root under the same workspace replaces the
    // repository on disk, which is what a commit would leave behind.
    let (other_temp, _other_transactions, _other_genesis) =
        genesis_in_workspace("smp1-330-advance", dependency_free_bodies(), &[], 1);
    let parked = TempDir::new("smp1-330-parked");
    let parked_repo = parked.child("repo");
    std::fs::rename(&harness.repository, &parked_repo).unwrap();
    std::fs::rename(other_temp.child("repo"), &harness.repository).unwrap();
    let stale = harness.fail(Method::HandleExpand, Harness::expand_body(1, root));
    assert_eq!(stale.symbol, "SESSION_STALE_HANDLE");
    assert_eq!(stale.code, 33_004);
    let advanced = harness.fail(Method::WorkspaceOpen, Vec::new());
    assert_eq!(advanced.symbol, "SESSION_ROOT_ADVANCED");
    // The head-bound set is closed: branch reads answer over current
    // repository state, so they refuse a stale session too.
    let refs_stale = harness.fail(Method::RefsResolve, b"main".to_vec());
    assert_eq!(refs_stale.symbol, "SESSION_ROOT_ADVANCED");
    // Methods naming explicit transactions are not head-bound.
    let explicit = harness.fail(Method::RevisionRead, tx(genesis_id));
    assert!(
        !explicit.symbol.starts_with("SESSION_"),
        "{}",
        explicit.symbol
    );
    // Renewal rebinds to the new head. A body that does not name the
    // frame's session is a frame failure, not a renewal.
    let new_root = harness.head_root();
    assert_ne!(new_root, root);
    let mismatch = harness.fail(Method::SessionRenew, vec![0xFF; 32]);
    assert_eq!(mismatch.code, ProtocolErrorCode::FrameInvalid.numeric());
    let renewed = harness.ok(Method::SessionRenew, session.as_bytes().to_vec());
    assert_eq!(renewed.body, session.as_bytes().to_vec());
    // The pre-renewal handle names the old root: it stays stale after
    // renewal instead of resolving to another entity. Only a handle
    // naming the new root resolves (contract section 4, threat T15).
    let still_stale = harness.fail(Method::HandleExpand, Harness::expand_body(0, root));
    assert_eq!(still_stale.symbol, "SESSION_STALE_HANDLE");
    let fresh = harness.ok(Method::HandleExpand, Harness::expand_body(0, new_root));
    assert_eq!(fresh.bounds.returned_entities, 1);
    let reopened = harness.ok(Method::WorkspaceOpen, Vec::new());
    assert_eq!(reopened.bounds.returned_entities, 1);
    // T47: a repository of another workspace refuses the session before
    // anything else, and the same holds for renewal.
    let (foreign_temp, _foreign_transactions, _foreign_genesis) =
        genesis_in_workspace("smp1-330-foreign", dependency_free_bodies(), &[], 2);
    let parked_two = TempDir::new("smp1-330-parked-two");
    std::fs::rename(&harness.repository, parked_two.child("repo")).unwrap();
    std::fs::rename(foreign_temp.child("repo"), &harness.repository).unwrap();
    let mismatch = harness.fail(
        Method::HandleExpand,
        Harness::expand_body(0, harness.head_root()),
    );
    assert_eq!(mismatch.symbol, "SESSION_WORKSPACE_MISMATCH");
    assert_eq!(mismatch.code, 33_001);
    let refused_renew = harness.fail(Method::SessionRenew, session.as_bytes().to_vec());
    assert_eq!(refused_renew.symbol, "SESSION_WORKSPACE_MISMATCH");
    let refused_read = harness.fail(Method::RevisionRead, tx(genesis_id));
    assert_eq!(refused_read.symbol, "SESSION_WORKSPACE_MISMATCH");
    // A capsule built under the session carries the negotiated arm.
    std::fs::rename(&harness.repository, foreign_temp.child("repo")).unwrap();
    std::fs::rename(parked_two.child("repo"), &harness.repository).unwrap();
    let transactions = sley_txn::TransactionRepository::new(&harness.repository);
    let head = transactions.accepted_head().unwrap();
    let outcome = run_root_query(
        &harness.repository,
        head.verified_revision(),
        &maintenance_guard(&harness.repository),
        RootQuery::GetRootSummary,
        QueryLimits::profile_maximum(),
        false,
        None,
    )
    .unwrap();
    let capsule = harness.ok(Method::Capsule, outcome.request.preimage().to_vec());
    assert!(
        capsule
            .body
            .windows(32)
            .any(|window| window == session.as_bytes())
    );
    // Unknown sessions are refused as sessions with SESSION_UNKNOWN: the
    // liveness check precedes request-identity admission.
    let ghost = harness
        .server
        .answer(&request_frame(
            Some(SessionId::from_bytes([0xAA; 32])),
            1,
            Method::WorkspaceOpen,
            Vec::new(),
        ))
        .unwrap();
    assert!(ghost.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&ghost.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    let failure = ProtocolFailure::decode(&frame.body).unwrap();
    assert_eq!(failure.symbol, "SESSION_UNKNOWN");
    assert_eq!(failure.code, 33_000);
    // The genesis exemption ends at the first head: sessionless creation
    // on a headed repository is refused with SESSION_BINDING_INVALID.
    let headed = harness
        .server
        .answer(&request_frame(None, 0, Method::WorkspaceCreate, Vec::new()))
        .unwrap();
    assert!(headed.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&headed.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    let failure = ProtocolFailure::decode(&frame.body).unwrap();
    assert_eq!(failure.symbol, "SESSION_BINDING_INVALID");
    assert_eq!(failure.code, 33_007);
    // Close, and the name answers SESSION_CLOSED while remembered.
    harness.ok(Method::SessionClose, Vec::new());
    let closed = harness.fail(Method::RevisionRead, tx(genesis_id));
    assert_eq!(closed.symbol, "PROTOCOL_SESSION_CLOSED");
    assert_eq!(closed.code, ProtocolErrorCode::SessionClosed.numeric());
}

#[test]
fn batch_cancel_precedes_execution_in_one_batch() {
    use crate::ProtocolErrorCode;
    let mut harness = Harness::new("smp1-batch-cancel");
    let session = harness.session;
    // One batch: a capabilities request, a cancel naming it, and a
    // budgets request. The cancel is collected before anything
    // executes, so the capabilities request is admitted (keeping the
    // sequence exact) but never executed.
    let frames = [
        request_frame(Some(session), 2, Method::SessionCapabilities, Vec::new()),
        request_frame(Some(session), 3, Method::Cancel, encode_uvar(2)),
        request_frame(Some(session), 4, Method::SessionBudgets, Vec::new()),
    ];
    let borrowed: Vec<&[u8]> = frames.iter().map(Vec::as_slice).collect();
    let answers = harness.server.answer_batch(&borrowed).unwrap();
    assert_eq!(answers.len(), 3);
    assert!(answers[0].failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&answers[0].frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::Cancelled.numeric()
    );
    assert!(!answers[1].failed);
    assert!(!answers[2].failed);
    // The cancelled request consumed no budget: budgets still report a
    // full session.
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&answers[2].frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert!(!frame.body.is_empty());
}

#[test]
fn live_sessions_are_capped_and_restarts_forget() {
    // A negotiated cap of one live session: the second open is refused
    // with SESSION_BINDING_INVALID, and closing frees the slot.
    let mut small_client = hello(all_methods(), 4);
    small_client.limits.max_sessions = 1;
    let mut harness = Harness::with_hellos("smp1-330-cap", small_client, hello(all_methods(), 8));
    let second = harness
        .server
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            harness.server.handshake_id().as_bytes().to_vec(),
        ))
        .unwrap();
    assert!(second.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&second.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    let failure = ProtocolFailure::decode(&frame.body).unwrap();
    assert_eq!(failure.symbol, "SESSION_BINDING_INVALID");
    assert_eq!(failure.code, 33_007);
    harness.ok(Method::SessionClose, Vec::new());
    let reopened = harness
        .server
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            harness.server.handshake_id().as_bytes().to_vec(),
        ))
        .unwrap();
    assert!(!reopened.failed);

    // A restart forgets every pre-restart name: the new instance mints a
    // fresh nonce, so the old name is unknown rather than silently
    // re-minted (contract section 1, threat T56).
    let mut harness = Harness::new("smp1-330-restart");
    let old = harness.session;
    let client_hello = harness.client_hello.clone();
    let server_hello = harness.server_hello.clone();
    harness.server = Server::new(&harness.repository, &client_hello, &server_hello).unwrap();
    let ghost = harness
        .server
        .answer(&request_frame(
            Some(old),
            1,
            Method::WorkspaceOpen,
            Vec::new(),
        ))
        .unwrap();
    assert!(ghost.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&ghost.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!();
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().symbol,
        "SESSION_UNKNOWN"
    );
}

#[test]
fn binding_failures_precede_budget_exhaustion() {
    use sley_repo::test_support::genesis_in_workspace;
    // One unit of work: the first call drains the session exactly.
    let mut tiny_client = hello(all_methods(), 4);
    tiny_client.limits.max_work = 1;
    let mut harness = Harness::with_hellos("smp1-330-budget", tiny_client, hello(all_methods(), 8));
    harness.ok(Method::WorkspaceOpen, Vec::new());
    // Exhausted and workspace-mismatched: the binding failure answers,
    // not the budget failure (contract section 3).
    let (foreign_temp, _foreign_transactions, _foreign_genesis) =
        genesis_in_workspace("smp1-330-budget-foreign", dependency_free_bodies(), &[], 2);
    let parked = sley_repo::test_support::TempDir::new("smp1-330-budget-parked");
    std::fs::rename(&harness.repository, parked.child("repo")).unwrap();
    std::fs::rename(foreign_temp.child("repo"), &harness.repository).unwrap();
    let mismatch = harness.fail(Method::WorkspaceOpen, Vec::new());
    assert_eq!(mismatch.symbol, "SESSION_WORKSPACE_MISMATCH");
    std::fs::rename(&harness.repository, foreign_temp.child("repo")).unwrap();
    // Exhausted and bound to a root the head has left behind: the stale
    // bound root answers on a head-bound method, not the budget (contract
    // section 3, check 5 before check 6).
    let (advanced_temp, _advanced_transactions, _advanced_genesis) =
        genesis_in_workspace("smp1-330-budget-advance", dependency_free_bodies(), &[], 1);
    std::fs::rename(advanced_temp.child("repo"), &harness.repository).unwrap();
    let stale = harness.fail(Method::WorkspaceOpen, Vec::new());
    assert_eq!(stale.symbol, "SESSION_ROOT_ADVANCED");
    std::fs::rename(&harness.repository, advanced_temp.child("repo")).unwrap();
    // Exhausted and validly bound: the budget failure answers.
    std::fs::rename(parked.child("repo"), &harness.repository).unwrap();
    let exhausted = harness.fail(Method::WorkspaceOpen, Vec::new());
    assert_eq!(exhausted.code, ProtocolErrorCode::LimitExceeded.numeric());
}

#[test]
fn session_bound_capsules_are_minted_from_live_authority_state() {
    use sley_repo::test_support::genesis_in_workspace;
    let mut harness = Harness::new("capsule-bind");
    let session = harness.session;
    let transactions = sley_txn::TransactionRepository::new(&harness.repository);
    let head = transactions.accepted_head().unwrap();
    let outcome = run_root_query(
        &harness.repository,
        head.verified_revision(),
        &maintenance_guard(&harness.repository),
        RootQuery::GetRootSummary,
        QueryLimits::profile_maximum(),
        false,
        None,
    )
    .unwrap();
    // The live session mints a capsule carrying the negotiated arm.
    let capsule = harness
        .server
        .authority_mut()
        .bind_context_capsule(session, &outcome.request, &outcome.response)
        .unwrap();
    assert_eq!(capsule.session(), Some(session));
    // A session the authority never issued mints nothing.
    assert_eq!(
        harness
            .server
            .authority_mut()
            .bind_context_capsule(
                SessionId::from_bytes([0xAA; 32]),
                &outcome.request,
                &outcome.response,
            )
            .unwrap_err(),
        CapsuleBindError::UnknownSession
    );
    // A consistent request/response pair from another workspace fails
    // the authority-held binding: the session's workspace, root, and
    // epoch come from authority state, never from caller arguments.
    let (foreign_temp, _foreign_transactions, _foreign_genesis) =
        genesis_in_workspace("capsule-bind-foreign", dependency_free_bodies(), &[], 2);
    let foreign_repo = foreign_temp.child("repo");
    let foreign_head = sley_txn::TransactionRepository::new(&foreign_repo)
        .accepted_head()
        .unwrap();
    let foreign = run_root_query(
        &foreign_repo,
        foreign_head.verified_revision(),
        &maintenance_guard(&foreign_repo),
        RootQuery::GetRootSummary,
        QueryLimits::profile_maximum(),
        false,
        None,
    )
    .unwrap();
    assert_eq!(
        harness
            .server
            .authority_mut()
            .bind_context_capsule(session, &foreign.request, &foreign.response)
            .unwrap_err(),
        CapsuleBindError::Capsule(ContextCapsuleError::new(
            ContextCapsuleErrorCode::SourceInvalid
        ))
    );
    // Closing the session revokes minting: no live session, no capsule.
    harness
        .server
        .authority_mut()
        .close_session(session)
        .unwrap();
    assert_eq!(
        harness
            .server
            .authority_mut()
            .bind_context_capsule(session, &outcome.request, &outcome.response)
            .unwrap_err(),
        CapsuleBindError::UnknownSession
    );
}

#[test]
fn the_offered_hello_names_exactly_the_dispatched_methods() {
    let offered = Server::offered_hello().unwrap();
    assert_eq!(offered.protocol_versions, vec![PROTOCOL_VERSION]);
    assert_eq!(offered.limits, LimitProfile::maximum());
    assert_eq!(
        offered.features,
        crate::FEATURE_CANCEL | crate::FEATURE_STREAM
    );
    assert!(offered.adapters.is_empty() && offered.effects.is_empty());
    assert_eq!(offered.methods.len(), 37);
    for method in Method::ALL {
        let offered_it = offered.methods.contains(&method.tag());
        assert_eq!(offered_it, !method.is_reserved(), "{method:?}");
    }
    // Every offered method answers something other than an unsupported-method
    // failure carrying the deferred or reserved reason.
    let mut harness = Harness::new("offered-hello");
    for tag in &offered.methods {
        let method = Method::from_tag(*tag).unwrap();
        if matches!(method, Method::SessionOpen | Method::SessionClose) {
            continue;
        }
        let (failed, frame) = harness.call(method, Vec::new());
        if failed {
            let failure = ProtocolFailure::decode(&frame.body).unwrap();
            assert!(
                failure.details != RESERVED_SEAM_370_DETAIL
                    && failure.details != RESERVED_SEAM_620_DETAIL,
                "{method:?} is offered but not dispatched"
            );
        }
    }
    let selected = negotiate(&offered, &offered).unwrap();
    assert_eq!(selected.methods, offered.methods);
}

/// A server with an open session over a genesis of the given bodies and no
/// dependency roots; returns the server, the session, and the genesis id.
fn open_server(
    label: &str,
    bodies: Vec<(u8, sley_mutate::value::EntityBodyValue)>,
) -> (
    sley_repo::test_support::TempDir,
    Server,
    SessionId,
    TransactionId,
) {
    open_server_with_features(label, bodies, 1)
}

fn open_server_with_features(
    label: &str,
    bodies: Vec<(u8, sley_mutate::value::EntityBodyValue)>,
    features: u32,
) -> (
    sley_repo::test_support::TempDir,
    Server,
    SessionId,
    TransactionId,
) {
    let (temp, _transactions, genesis_id) = genesis(label, bodies, &[]);
    let repository = temp.child("repo");
    let mut client_hello = hello(all_methods(), 4);
    client_hello.features = features;
    let mut server_hello = hello(all_methods(), 8);
    server_hello.features = features;
    let mut server = Server::new(&repository, &client_hello, &server_hello).unwrap();
    let handshake = server.handshake_id();
    let open = server
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            handshake.as_bytes().to_vec(),
        ))
        .unwrap();
    assert!(!open.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&open.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!("session open response");
    };
    let session = SessionId::from_bytes(frame.body.as_slice().try_into().unwrap());
    (temp, server, session, genesis_id)
}

fn call_frame(
    server: &mut Server,
    session: SessionId,
    id: u64,
    method: Method,
    body: Vec<u8>,
) -> (bool, ProtocolFrame) {
    let answer = server
        .answer(&request_frame(Some(session), id, method, body))
        .unwrap();
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&answer.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!("response frame");
    };
    (answer.failed, frame)
}

fn fields_of(body: &[u8], count: u64) -> Vec<Vec<u8>> {
    // record(count) || (tag || len || bytes)*
    let mut offset = 0;
    let read = |offset: &mut usize| -> u64 {
        let mut value = 0u64;
        let mut shift = 0;
        loop {
            let byte = body[*offset];
            *offset += 1;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return value;
            }
            shift += 7;
        }
    };
    assert_eq!(read(&mut offset), count);
    (0..count)
        .map(|index| {
            assert_eq!(read(&mut offset), index + 1);
            let length = usize::try_from(read(&mut offset)).unwrap();
            let field = body[offset..offset + length].to_vec();
            offset += length;
            field
        })
        .collect()
}

fn list_of(body: &[u8]) -> Vec<Vec<u8>> {
    // list || (len || bytes)*
    let mut offset = 0;
    let read = |offset: &mut usize| -> u64 {
        let mut value = 0u64;
        let mut shift = 0;
        loop {
            let byte = body[*offset];
            *offset += 1;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return value;
            }
            shift += 7;
        }
    };
    let count = usize::try_from(read(&mut offset)).unwrap();
    (0..count)
        .map(|_| {
            let length = usize::try_from(read(&mut offset)).unwrap();
            let field = body[offset..offset + length].to_vec();
            offset += length;
            field
        })
        .collect()
}

#[test]
fn gc_dry_run_and_collect_derive_the_snapshot_from_the_repository() {
    let (_temp, mut server, session, genesis_id) = open_server("smp1-gc", dependency_free_bodies());
    let (failed, _) = call_frame(
        &mut server,
        session,
        2,
        Method::BranchCreate,
        encode_record(&[(1, b"main".to_vec()), (2, tx(genesis_id))]).unwrap(),
    );
    assert!(!failed);
    let no_pins = encode_record(&[(1, sley_scb1::encode_list(&[]).unwrap())]).unwrap();
    let (failed, frame) = call_frame(&mut server, session, 3, Method::GcDryRun, no_pins.clone());
    assert!(!failed, "{:?}", ProtocolFailure::decode(&frame.body));
    let fields = fields_of(&frame.body, 10);
    // decision 1 = dry run; no candidates; every inventory object reachable.
    assert_eq!(fields[7], encode_uvar(1));
    assert_eq!(fields[4], sley_scb1::encode_list(&[]).unwrap());
    assert_eq!(fields[2], fields[3], "reachable equals inventory");
    assert_eq!(fields[8], sley_scb1::encode_list(&[]).unwrap());
    // The examined anchors are the accepted head and the one named branch.
    let anchors_record = &fields[0];
    assert!(
        anchors_record[0] >= 2,
        "at least the head and the branch anchors"
    );
    // A pin naming an unknown object fails closed in the owner.
    let pin = encode_record(&[(
        1,
        sley_scb1::encode_list(&[sley_scb1::encode_union(2, &[0xEE; 32]).unwrap()]).unwrap(),
    )])
    .unwrap();
    let (failed, frame) = call_frame(&mut server, session, 4, Method::GcDryRun, pin);
    assert!(failed);
    assert!(
        ProtocolFailure::decode(&frame.body)
            .unwrap()
            .symbol
            .starts_with("GC_")
    );
    // Collect under the exclusive guard: nothing is unreachable, so nothing is deleted.
    let (failed, frame) = call_frame(&mut server, session, 5, Method::GcCollect, no_pins);
    assert!(!failed, "{:?}", ProtocolFailure::decode(&frame.body));
    let fields = fields_of(&frame.body, 10);
    assert_eq!(fields[7], encode_uvar(2));
    assert_eq!(fields[8], sley_scb1::encode_list(&[]).unwrap());
    assert_eq!(fields[9], sley_scb1::encode_union(0, &[]).unwrap());
    // The repository still answers reads after a collection.
    let (failed, _) = call_frame(
        &mut server,
        session,
        6,
        Method::RevisionRead,
        tx(genesis_id),
    );
    assert!(!failed);
}

#[test]
fn tampered_hello_is_rejected_at_session_open() {
    // Threat T45: the client offers versions 1 and 2, the server offers 1,
    // so the honest selection is version 1. A stripped client hello (2
    // removed on the wire) negotiates the same selection but a different
    // transcript, so the honest identity must fail at open.
    let (temp, _transactions, _genesis) = genesis("smp1-t45", dependency_free_bodies(), &[]);
    let repository = temp.child("repo");
    let mut client = hello(all_methods(), 4);
    client.protocol_versions = vec![1, 2];
    let server_hello = hello(all_methods(), 8);
    let (_, honest_id) = negotiate_identity(&client, &server_hello).unwrap();
    let mut stripped = client.clone();
    stripped.protocol_versions = vec![1];
    let mut server = Server::new(&repository, &stripped, &server_hello).unwrap();
    assert_eq!(server.profile().protocol_version, 1);
    assert_ne!(
        server.handshake_id(),
        honest_id,
        "the transcript binds the hellos, not just the selection"
    );
    let answer = server
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            honest_id.as_bytes().to_vec(),
        ))
        .unwrap();
    assert!(answer.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&answer.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!("downgrade response");
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::Downgrade.numeric()
    );
    // The server's own view is self-consistent and opens.
    let answer = server
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            server.handshake_id().as_bytes().to_vec(),
        ))
        .unwrap();
    assert!(!answer.failed);
}

#[test]
fn collection_retains_a_non_head_session_bound_root() {
    // Threat T15: S1 is renewed onto a real importable root built from
    // this repository's own objects that is not the head, simulating a
    // session bound to an old head after the head advanced without
    // renewal. A dry run from a second session must retain that root
    // with no deletion candidates; before the all-session pins it was a
    // silent deletion candidate.
    use sley_state_root::{StateRootBuilder, conformance_registry as state_registry};

    let (_temp_a, mut server_a, s1, _) = open_server("smp1-gc-stale-a", dependency_free_bodies());
    let head_record = sley_txn::TransactionRepository::new(server_a.repository())
        .accepted_head()
        .unwrap()
        .verified_revision()
        .clone()
        .state_root()
        .clone();
    let (old_entity, old_object) = head_record.record.entity_bindings[0];
    let odd = StateRootBuilder::new(
        head_record.record.workspace_id,
        head_record.record.contract_root,
        head_record.record.test_root,
        head_record.record.policy_root,
    )
    .entity_binding(old_entity, old_object)
    .build(&state_registry().unwrap())
    .unwrap();
    let bound = server_a.authority_mut().record(s1).copied().unwrap();
    assert_ne!(
        odd.root, bound.bound_root,
        "the partial root must diverge from the head for the test to mean anything"
    );
    let binding = HeadBinding {
        workspace_id: bound.workspace_id,
        root: odd.root,
        schema_epoch: bound.schema_epoch,
    };
    server_a
        .authority_mut()
        .renew_session(s1, &binding, &odd)
        .unwrap();
    let open = server_a
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            server_a.handshake_id().as_bytes().to_vec(),
        ))
        .unwrap();
    assert!(!open.failed);
    let (DecodedFrame::Response(open_frame), _) =
        decode_frame(&open.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!("second session opens");
    };
    let s2 = SessionId::from_bytes(open_frame.body.as_slice().try_into().unwrap());
    let no_pins = encode_record(&[(1, sley_scb1::encode_list(&[]).unwrap())]).unwrap();
    let (failed, frame) = call_frame(&mut server_a, s2, 1, Method::GcDryRun, no_pins);
    assert!(!failed, "{:?}", ProtocolFailure::decode(&frame.body));
    let fields = fields_of(&frame.body, 10);
    assert!(
        fields[1]
            .windows(32)
            .any(|window| window == odd.root.as_bytes().as_slice()),
        "the stale session's bound root is retained"
    );
    assert_eq!(
        fields[4],
        sley_scb1::encode_list(&[]).unwrap(),
        "nothing is a deletion candidate"
    );
    // One SessionPin anchor (kind 7) per live session, under its own id.
    let mut pins: Vec<(Vec<u8>, Vec<u8>)> = list_of(&fields[0])
        .iter()
        .map(|anchor| {
            let key = fields_of(anchor, 2);
            (key[0].clone(), key[1].clone())
        })
        .filter(|(kind, _)| *kind == encode_uvar(7))
        .collect();
    pins.sort();
    let mut expected = vec![
        (encode_uvar(7), s1.as_bytes().to_vec()),
        (encode_uvar(7), s2.as_bytes().to_vec()),
    ];
    expected.sort();
    assert_eq!(pins, expected);
}

#[test]
fn execute_under_the_extended_profile_derives_its_own_report_identity() {
    use sley_ssmc::{ConstData, ConstValue, TypeExpr};
    let (_temp, mut server, session, _genesis_id) = open_server_with_features(
        "smp1-execute-extended",
        executable_bodies(),
        1 | FEATURE_EXTENDED_EXECUTE,
    );
    let function = sley_repo::test_support::id(30);
    let value = |bit: bool| {
        sley_mutate::encode_const_value(&ConstValue {
            value_type: TypeExpr::Bool,
            data: ConstData::Bool(bit),
        })
        .unwrap()
    };
    // Appendix C field 6 selects the profile: 1 restricted, 2 extended.
    let body = |profile: u64, inputs: Vec<Vec<u8>>| {
        let limits = encode_record(&[
            (1, encode_uvar(1_000)),
            (2, encode_uvar(1_000)),
            (3, encode_uvar(10_000)),
            (4, encode_uvar(100)),
            (5, sley_scb1::encode_union(0, &[]).unwrap()),
            (6, encode_uvar(profile)),
        ])
        .unwrap();
        encode_record(&[
            (1, function.as_bytes().to_vec()),
            (2, sley_scb1::encode_list(&inputs).unwrap()),
            (3, limits),
        ])
        .unwrap()
    };
    let (failed, restricted) = call_frame(
        &mut server,
        session,
        2,
        Method::Execute,
        body(1, vec![value(true), value(true)]),
    );
    assert!(!failed, "{:?}", ProtocolFailure::decode(&restricted.body));
    let (failed, extended) = call_frame(
        &mut server,
        session,
        3,
        Method::Execute,
        body(2, vec![value(true), value(true)]),
    );
    assert!(!failed, "{:?}", ProtocolFailure::decode(&extended.body));
    // The same function and inputs under two profiles are two reports: the
    // envelope binds the profile it ran under (S20-290 revision 2).
    assert_ne!(
        fields_of(&restricted.body, 2)[0],
        fields_of(&extended.body, 2)[0],
        "two profiles must not share one execution report identity"
    );
    // Each identity still re-derives from its own stored preimage, and the
    // store answers the extended one verbatim.
    let identity = fields_of(&extended.body, 2)[0].clone();
    let (failed, report) = call_frame(&mut server, session, 4, Method::Report, identity);
    assert!(!failed, "{:?}", ProtocolFailure::decode(&report.body));
    assert_eq!(report.body, extended.body);
}

#[test]
fn execute_runs_a_bound_root_function_and_report_answers_the_stored_record() {
    use sley_ssmc::{ConstData, ConstValue, TypeExpr};
    let (_temp, mut server, session, _genesis_id) =
        open_server("smp1-execute", executable_bodies());
    let function = sley_repo::test_support::id(30);
    let value = |bit: bool| {
        sley_mutate::encode_const_value(&ConstValue {
            value_type: TypeExpr::Bool,
            data: ConstData::Bool(bit),
        })
        .unwrap()
    };
    let limits = encode_record(&[
        (1, encode_uvar(1_000)),
        (2, encode_uvar(1_000)),
        (3, encode_uvar(10_000)),
        (4, encode_uvar(100)),
        (5, sley_scb1::encode_union(0, &[]).unwrap()),
        (6, encode_uvar(1)),
    ])
    .unwrap();
    let body = |inputs: Vec<Vec<u8>>| {
        encode_record(&[
            (1, function.as_bytes().to_vec()),
            (2, sley_scb1::encode_list(&inputs).unwrap()),
            (3, limits.clone()),
        ])
        .unwrap()
    };
    let (failed, frame) = call_frame(
        &mut server,
        session,
        2,
        Method::Execute,
        body(vec![value(true), value(true)]),
    );
    assert!(!failed, "{:?}", ProtocolFailure::decode(&frame.body));
    let fields = fields_of(&frame.body, 2);
    assert_eq!(fields[0].len(), 32);
    // field 2 is bytes(len || preimage)
    let mut offset = 0;
    let mut length = 0usize;
    let mut shift = 0;
    loop {
        let byte = fields[1][offset];
        offset += 1;
        length |= usize::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    let stored = &fields[1][offset..offset + length];
    assert!(stored.starts_with(b"SLEYEXR1"));
    assert_eq!(
        sley_id::ExecutionReportId::derive(stored).as_bytes(),
        fields[0].as_slice(),
        "the identity re-derives from the preimage"
    );
    // Equal executions answer the same identity and bytes.
    let (failed, again) = call_frame(
        &mut server,
        session,
        3,
        Method::Execute,
        body(vec![value(true), value(true)]),
    );
    assert!(!failed);
    assert_eq!(again.body, frame.body);
    // Report answers the stored record verbatim.
    let (failed, report) = call_frame(&mut server, session, 4, Method::Report, fields[0].clone());
    assert!(!failed, "{:?}", ProtocolFailure::decode(&report.body));
    assert_eq!(report.body, frame.body);
    // An input count mismatch is a rejected report, not a protocol failure.
    let (failed, rejected) = call_frame(
        &mut server,
        session,
        5,
        Method::Execute,
        body(vec![value(true)]),
    );
    assert!(!failed, "{:?}", ProtocolFailure::decode(&rejected.body));
    assert_ne!(rejected.body, frame.body);
    // An unknown function is a payload failure with the appendix C detail.
    let mut unknown = body(vec![value(true), value(true)]);
    unknown[3..35].copy_from_slice(&[0x77; 32]);
    let (failed, frame) = call_frame(&mut server, session, 6, Method::Execute, unknown);
    assert!(failed);
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().details,
        FUNCTION_UNKNOWN_DETAIL
    );
    // A malformed value is a payload failure.
    let (failed, frame) = call_frame(
        &mut server,
        session,
        7,
        Method::Execute,
        body(vec![vec![0xFF, 0xFF]]),
    );
    assert!(failed);
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::PayloadInvalid.numeric()
    );
}

#[test]
fn execute_selects_the_cache_profile_from_limits_field_six() {
    use sley_ssmc::{ConstData, ConstValue, TypeExpr};
    let (_temp, mut server, session, _genesis_id) = open_server_with_features(
        "smp1-execute-profile",
        executable_bodies(),
        1 | FEATURE_EXTENDED_EXECUTE,
    );
    let function = sley_repo::test_support::id(30);
    let value = |bit: bool| {
        sley_mutate::encode_const_value(&ConstValue {
            value_type: TypeExpr::Bool,
            data: ConstData::Bool(bit),
        })
        .unwrap()
    };
    let limits = |fields: Vec<(u32, Vec<u8>)>| encode_record(&fields).unwrap();
    let base = |profile: Option<u64>| {
        let mut fields = vec![
            (1, encode_uvar(1_000)),
            (2, encode_uvar(1_000)),
            (3, encode_uvar(10_000)),
            (4, encode_uvar(100)),
            (5, sley_scb1::encode_union(0, &[]).unwrap()),
        ];
        if let Some(profile) = profile {
            fields.push((6, encode_uvar(profile)));
        }
        fields
    };
    let body = |limits_bytes: Vec<u8>| {
        encode_record(&[
            (1, function.as_bytes().to_vec()),
            (
                2,
                sley_scb1::encode_list(&[value(true), value(true)]).unwrap(),
            ),
            (3, limits_bytes),
        ])
        .unwrap()
    };
    let (failed, restricted) = call_frame(
        &mut server,
        session,
        2,
        Method::Execute,
        body(limits(base(Some(1)))),
    );
    assert!(!failed, "{:?}", ProtocolFailure::decode(&restricted.body));
    let (failed, extended) = call_frame(
        &mut server,
        session,
        3,
        Method::Execute,
        body(limits(base(Some(2)))),
    );
    assert!(!failed, "{:?}", ProtocolFailure::decode(&extended.body));
    // Both profiles execute the Boolean function; the reports differ because
    // the cache key names the profile.
    assert_ne!(
        fields_of(&restricted.body, 2)[0],
        fields_of(&extended.body, 2)[0]
    );
    for (request, malformed) in [(4, limits(base(Some(3)))), (5, limits(base(None)))] {
        let (failed, frame) = call_frame(
            &mut server,
            session,
            request,
            Method::Execute,
            body(malformed),
        );
        assert!(failed);
        assert_eq!(
            ProtocolFailure::decode(&frame.body).unwrap().code,
            ProtocolErrorCode::PayloadInvalid.numeric()
        );
    }
}

/// Prints the S20-720 demo fixture for `scripts/generate_release_demo_fixtures.py`:
/// the exchange of the executable genesis with branch `main`, a bound
/// `query.root` summary request and its response, an `execute` request and
/// its report identity, and the head transaction identity.
#[test]
#[ignore = "fixture refresh emitter"]
fn emit_release_demo_vectors_for_fixture_refresh() {
    use sley_ssmc::{ConstData, ConstValue, TypeExpr};
    fn hex_of(bytes: &[u8]) -> String {
        use core::fmt::Write as _;
        bytes.iter().fold(String::new(), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
    }
    let (temp, mut server, session, genesis_id) = open_server("release-demo", executable_bodies());
    let repository = temp.child("repo");
    let (failed, _) = call_frame(
        &mut server,
        session,
        2,
        Method::BranchCreate,
        encode_record(&[(1, b"main".to_vec()), (2, tx(genesis_id))]).unwrap(),
    );
    assert!(!failed);
    let transactions = sley_txn::TransactionRepository::new(&repository);
    let revision = transactions.verified_revision(genesis_id).unwrap();
    let outcome = run_root_query(
        &repository,
        &revision,
        &maintenance_guard(&repository),
        RootQuery::GetRootSummary,
        QueryLimits::profile_maximum(),
        false,
        None,
    )
    .unwrap();
    let query_request = outcome.request.preimage().to_vec();
    let (failed, summary) = call_frame(
        &mut server,
        session,
        3,
        Method::QueryRoot,
        query_request.clone(),
    );
    assert!(!failed);
    let function = sley_repo::test_support::id(30);
    let value = |bit: bool| {
        sley_mutate::encode_const_value(&ConstValue {
            value_type: TypeExpr::Bool,
            data: ConstData::Bool(bit),
        })
        .unwrap()
    };
    let execute_request = encode_record(&[
        (1, function.as_bytes().to_vec()),
        (
            2,
            sley_scb1::encode_list(&[value(true), value(true)]).unwrap(),
        ),
        (
            3,
            encode_record(&[
                (1, encode_uvar(1_000)),
                (2, encode_uvar(1_000)),
                (3, encode_uvar(10_000)),
                (4, encode_uvar(100)),
                (5, sley_scb1::encode_union(0, &[]).unwrap()),
                (6, encode_uvar(1)),
            ])
            .unwrap(),
        ),
    ])
    .unwrap();
    let (failed, executed) = call_frame(
        &mut server,
        session,
        4,
        Method::Execute,
        execute_request.clone(),
    );
    assert!(!failed, "{:?}", ProtocolFailure::decode(&executed.body));
    let report_id = fields_of(&executed.body, 2)[0].clone();
    let (failed, exported) =
        call_frame(&mut server, session, 5, Method::ExchangeExport, Vec::new());
    assert!(!failed, "{:?}", ProtocolFailure::decode(&exported.body));
    println!("RELEASE_DEMO|exchange_hex|{}", hex_of(&exported.body));
    println!(
        "RELEASE_DEMO|query_root_request_hex|{}",
        hex_of(&query_request)
    );
    println!(
        "RELEASE_DEMO|query_root_response_hex|{}",
        hex_of(&summary.body)
    );
    println!(
        "RELEASE_DEMO|execute_request_hex|{}",
        hex_of(&execute_request)
    );
    println!(
        "RELEASE_DEMO|execute_response_hex|{}",
        hex_of(&executed.body)
    );
    println!(
        "RELEASE_DEMO|execution_report_id_hex|{}",
        hex_of(&report_id)
    );
    println!(
        "RELEASE_DEMO|head_transaction_id_hex|{}",
        hex_of(genesis_id.as_bytes())
    );
    println!(
        "RELEASE_DEMO|function_id_hex|{}",
        hex_of(function.as_bytes())
    );
    println!("RELEASE_DEMO|branch_name_hex|{}", hex_of(b"main"));
}

#[test]
fn request_id_zero_never_enters_a_session() {
    // Identifier 0 is the pre-session sentinel (contract section 3): the
    // first legal in-session identifier is 1, and a rejection consumes
    // nothing from the sequence.
    let mut harness = Harness::new("smp1-id-zero");
    let answer = harness
        .server
        .answer(&request_frame(
            Some(harness.session),
            0,
            Method::SessionCapabilities,
            Vec::new(),
        ))
        .unwrap();
    assert!(answer.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&answer.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!("response frame");
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::RequestIdConflict.numeric()
    );
    harness.ok(Method::SessionCapabilities, Vec::new());
}

#[test]
fn hello_listing_a_reserved_tag_is_a_payload_failure() {
    // Reserved tags are never negotiable (contract section 2): a hello
    // naming one fails validation rather than intersecting it.
    let mut harness = Harness::new("smp1-hello-reserved");
    let reserved_client = hello(vec![100, 305], 4);
    assert_eq!(
        Server::new(&harness.repository, &reserved_client, &harness.server_hello)
            .unwrap_err()
            .code(),
        ProtocolErrorCode::PayloadInvalid
    );
    // The live session is unaffected.
    harness.ok(Method::SessionCapabilities, Vec::new());
}

#[test]
fn selection_without_session_open_is_no_common_profile() {
    // The session family is the mandatory floor (contract section 2): an
    // intersection without `session.open` is no common profile.
    let client = hello(vec![300, 301], 4);
    let server = hello(all_methods(), 8);
    assert_eq!(
        negotiate(&client, &server).unwrap_err().code(),
        ProtocolErrorCode::NoCommonProfile
    );
}

#[test]
fn frame_codec_pins_protocol_version_one() {
    // Only version 1 exists (contract section 2): the codec neither
    // emits nor admits another version. A frame below a future selection
    // would be a downgrade attempt and above it an unknown version; that
    // server rule lives in `SelectedProfile::check_claim` (unit-tested in
    // `crate::tests`) and the dispatch path applies it.
    let mut harness = Harness::new("smp1-version-pin");
    for (claimed, code) in [
        (0, ProtocolErrorCode::Downgrade),
        (PROTOCOL_VERSION + 1, ProtocolErrorCode::VersionUnsupported),
    ] {
        let frame = ProtocolFrame {
            protocol_version: claimed,
            session: Some(harness.session),
            request_id: harness.next_request,
            kind: FrameKind::Request,
            method: Method::SessionCapabilities.tag(),
            flags: 0,
            bounds: BoundedContext::none(),
            body: Vec::new(),
        };
        assert_eq!(
            encode_frame(&frame).unwrap_err().code(),
            code,
            "claimed version {claimed}"
        );
    }
    harness.ok(Method::SessionCapabilities, Vec::new());
}

#[test]
fn failed_dispatch_costs_one_budget_unit() {
    // Dispatch costs one unit up front (contract section 7): a request
    // that reaches an engine and fails still costs work, so only the
    // bytes ride on success.
    let harness = Harness::new("smp1-budget-failure");
    let mut limited_client = hello(all_methods(), 4);
    limited_client.limits.max_work = 64;
    let mut budgeted = Server::new(
        &harness.repository,
        &limited_client,
        &hello(all_methods(), 8),
    )
    .unwrap();
    let open = budgeted
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            budgeted.handshake_id().as_bytes().to_vec(),
        ))
        .unwrap();
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&open.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!("open response");
    };
    let session = SessionId::from_bytes(frame.body.as_slice().try_into().unwrap());
    assert_eq!(budgeted.remaining_budget(session), Some(64));
    // A malformed body fails before any engine runs but after dispatch.
    let (failed, _) = call_frame(
        &mut budgeted,
        session,
        1,
        Method::RevisionRead,
        vec![1, 2, 3],
    );
    assert!(failed);
    assert_eq!(budgeted.remaining_budget(session), Some(63));
}

#[test]
fn oversize_response_fails_before_any_partial_body() {
    // The negotiated `max_response_bytes` binds the transport outcome
    // (contract section 5): a body that does not fit fails closed on the
    // single-frame path, with no partial body.
    let harness = Harness::new("smp1-response-ceiling");
    let mut small_client = hello(all_methods(), 4);
    small_client.limits.max_response_bytes = 40;
    let mut small =
        Server::new(&harness.repository, &small_client, &hello(all_methods(), 8)).unwrap();
    let open = small
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            small.handshake_id().as_bytes().to_vec(),
        ))
        .unwrap();
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&open.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!("open response");
    };
    let session = SessionId::from_bytes(frame.body.as_slice().try_into().unwrap());
    // The 32-byte session id fits the 40-byte ceiling, but the
    // capabilities preimage does not.
    let (failed, capabilities) = call_frame(
        &mut small,
        session,
        1,
        Method::SessionCapabilities,
        Vec::new(),
    );
    assert!(failed);
    assert_eq!(
        ProtocolFailure::decode(&capabilities.body).unwrap().code,
        ProtocolErrorCode::LimitExceeded.numeric()
    );
    assert!(capabilities.bounds.returned_bytes <= 40 || capabilities.bounds.returned_bytes == 0);
}

#[test]
fn bounded_context_counts_above_negotiated_maximums_fail() {
    // Every copied count binds the transport outcome (contract section
    // 5): bytes, entities, edges, and depth each fail closed past their
    // negotiated maximum, with no partial body.
    let harness = Harness::new("smp1-count-ceilings");
    let limits = harness.server.profile_limits_for_test();
    let ok_bounds = || crate::BoundedContext {
        applied_limits: limits,
        ..crate::BoundedContext::none()
    };
    let refused = |bounds: crate::BoundedContext| {
        harness
            .server
            .respond_for_test(
                Some(harness.session),
                99,
                Method::RefsList.tag(),
                Ok((vec![1, 2, 3], bounds)),
            )
            .unwrap()
    };
    // A fitting body answers normally.
    let fitting = harness
        .server
        .respond_for_test(
            Some(harness.session),
            99,
            Method::RefsList.tag(),
            Ok((vec![1, 2, 3], ok_bounds())),
        )
        .unwrap();
    assert!(!fitting.failed);
    for bounds in [
        crate::BoundedContext {
            returned_bytes: limits.max_response_bytes + 1,
            ..ok_bounds()
        },
        crate::BoundedContext {
            returned_entities: limits.max_entities + 1,
            ..ok_bounds()
        },
        crate::BoundedContext {
            returned_edges: limits.max_edges + 1,
            ..ok_bounds()
        },
        crate::BoundedContext {
            reached_depth: limits.max_depth + 1,
            ..ok_bounds()
        },
    ] {
        let answer = refused(bounds);
        assert!(answer.failed);
        assert!(answer.events.is_empty());
        let (DecodedFrame::Response(frame), _) =
            decode_frame(&answer.frame.bytes, MAX_FRAME_BYTES).unwrap()
        else {
            panic!("response frame");
        };
        assert_eq!(
            ProtocolFailure::decode(&frame.body).unwrap().code,
            ProtocolErrorCode::LimitExceeded.numeric()
        );
    }
}

#[test]
fn reserved_detail_names_the_seam_and_is_retryable_after_capability() {
    use crate::Retryability;
    // A reserved tag names a real seam (contract section 4): the detail
    // says which, and the failure lifts when the capability appears.
    let mut harness = Harness::new("smp1-reserved-seam");
    let diagnostics = harness.fail(Method::Diagnostics, Vec::new());
    assert_eq!(diagnostics.details, RESERVED_SEAM_620_DETAIL);
    assert_eq!(diagnostics.retryability, Retryability::AfterCapability);
    let protected_move = harness.fail(Method::RefMoveProtected, Vec::new());
    assert_eq!(protected_move.details, RESERVED_SEAM_370_DETAIL);
    assert_eq!(protected_move.retryability, Retryability::AfterCapability);
}

#[test]
fn extended_profile_requires_the_negotiated_feature_bit() {
    // Profile selector 2 without the intersected `extended_execute`
    // feature bit is a payload failure (contract appendix C).
    use sley_ssmc::{ConstData, ConstValue, TypeExpr};
    let (_temp, mut server, session, _genesis_id) =
        open_server("smp1-execute-gated", executable_bodies());
    let function = sley_repo::test_support::id(30);
    let value = sley_mutate::encode_const_value(&ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(true),
    })
    .unwrap();
    let limits = encode_record(&[
        (1, encode_uvar(1_000)),
        (2, encode_uvar(1_000)),
        (3, encode_uvar(10_000)),
        (4, encode_uvar(100)),
        (5, sley_scb1::encode_union(0, &[]).unwrap()),
        (6, encode_uvar(2)),
    ])
    .unwrap();
    let body = encode_record(&[
        (1, function.as_bytes().to_vec()),
        (2, sley_scb1::encode_list(&[value]).unwrap()),
        (3, limits),
    ])
    .unwrap();
    let (failed, frame) = call_frame(&mut server, session, 2, Method::Execute, body);
    assert!(failed);
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::PayloadInvalid.numeric()
    );
}

#[test]
fn request_carrying_bounds_is_malformed() {
    // Bounds ride responses only (contract section 1): a request
    // carrying nonzero bounds fails before anything else runs.
    let mut harness = Harness::new("smp1-bounds-malformed");
    let mut frame = request_frame(
        Some(harness.session),
        harness.next_request,
        Method::SessionCapabilities,
        Vec::new(),
    );
    let DecodedFrame::Request(mut decoded) = decode_frame(&frame, MAX_FRAME_BYTES).unwrap().0
    else {
        panic!("request frame");
    };
    decoded.bounds = crate::BoundedContext {
        returned_bytes: 1,
        ..crate::BoundedContext::none()
    };
    frame = encode_frame(&decoded).unwrap().bytes;
    let answer = harness.server.answer(&frame).unwrap();
    assert!(answer.failed);
    let (DecodedFrame::Response(failed), _) =
        decode_frame(&answer.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!("response frame");
    };
    assert_eq!(
        ProtocolFailure::decode(&failed.body).unwrap().code,
        ProtocolErrorCode::FrameInvalid.numeric()
    );
    harness.next_request += 1;
    harness.ok(Method::SessionCapabilities, Vec::new());
}

#[test]
fn session_less_frame_with_nonzero_identifier_is_malformed() {
    // The pre-session space carries identifier 0 only (contract
    // section 3): a session-less frame naming any other identifier is
    // malformed, and the rejection consumes nothing.
    let mut harness = Harness::new("smp1-presession-id");
    let answer = harness
        .server
        .answer(&request_frame(
            None,
            7,
            Method::SessionOpen,
            harness.server.handshake_id().as_bytes().to_vec(),
        ))
        .unwrap();
    assert!(answer.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&answer.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!("response frame");
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::FrameInvalid.numeric()
    );
    // A malformed pre-session identifier is answered, never echoed:
    // session-less answers carry identifier 0 (contract appendix B).
    assert_eq!(answer.request_id, 0);
    assert_eq!(frame.request_id, 0);
    assert_eq!(frame.session, None);
    // Identifier 0 still opens.
    let open = harness
        .server
        .answer(&request_frame(
            None,
            0,
            Method::SessionOpen,
            harness.server.handshake_id().as_bytes().to_vec(),
        ))
        .unwrap();
    assert!(!open.failed);
}

// ---------------------------------------------------------------------------
// Protocol version 2 entity reads (AT-MW-02, ENTITY_READ_PROFILE_V2)
// ---------------------------------------------------------------------------

use crate::{
    ENTITY_SIGNATURE_TAG, ENTITY_VERSION_TAG, PROTOCOL_VERSION_V2, decode_frame_for_version,
    encode_frame_for_version, encode_single_frame_direct, frame_total_len,
    negotiate_identity_versioned, negotiate_versioned,
};
use sley_id::{EntityId, StateRoot};
use sley_query::decode_entity_read_response;

fn vhello(methods: Vec<u32>, inflight: u32) -> Hello {
    vhello_capped(methods, inflight, 100_000_000, 8_388_608, 8_388_608, 1)
}

fn vhello_capped(
    methods: Vec<u32>,
    inflight: u32,
    max_work: u64,
    max_frame_bytes: u64,
    max_response_bytes: u64,
    features: u32,
) -> Hello {
    Hello {
        protocol_versions: vec![PROTOCOL_VERSION, PROTOCOL_VERSION_V2],
        schema_epochs: vec![epoch(0x11)],
        limits: LimitProfile {
            max_frame_bytes,
            max_entities: 65_535,
            max_edges: 400_000,
            max_depth: 65_535,
            max_response_bytes,
            max_work,
            max_inflight: inflight,
            max_sessions: 256,
        },
        methods,
        features,
        adapters: vec![],
        effects: vec![],
    }
}

fn v2_methods() -> Vec<u32> {
    Method::V2_ALL
        .iter()
        .filter(|method| !method.is_reserved())
        .map(|method| method.tag())
        .collect()
}

fn vrequest_frame(session: Option<SessionId>, request_id: u64, tag: u32, body: Vec<u8>) -> Vec<u8> {
    encode_frame_for_version(
        &ProtocolFrame {
            protocol_version: PROTOCOL_VERSION_V2,
            session,
            request_id,
            kind: FrameKind::Request,
            method: tag,
            flags: 0,
            bounds: BoundedContext::none(),
            body,
        },
        PROTOCOL_VERSION_V2,
    )
    .unwrap()
    .bytes
}

fn entity_read_body(root: StateRoot, entity: EntityId) -> Vec<u8> {
    entity_read_body_capped(root, entity, 65_535, 8_388_608, 100_000_000)
}

fn entity_read_body_capped(
    root: StateRoot,
    entity: EntityId,
    max_objects: u64,
    max_response_bytes: u64,
    max_work: u64,
) -> Vec<u8> {
    encode_record(&[
        (1, root.as_bytes().to_vec()),
        (2, entity.as_bytes().to_vec()),
        (3, encode_uvar(max_objects)),
        (4, encode_uvar(max_response_bytes)),
        (5, encode_uvar(max_work)),
    ])
    .unwrap()
}

struct VServer {
    _temp: sley_repo::test_support::TempDir,
    repository: std::path::PathBuf,
    server: Server,
    session: SessionId,
    root: StateRoot,
    next_request: u64,
}

impl VServer {
    fn new(label: &str) -> Self {
        Self::with_bodies(label, executable_bodies(), &[])
    }

    fn with_bodies(
        label: &str,
        bodies: Vec<(u8, sley_mutate::value::EntityBodyValue)>,
        dependency_roots: &[StateRoot],
    ) -> Self {
        Self::with_hellos_bodies(
            label,
            bodies,
            dependency_roots,
            &vhello(v2_methods(), 4),
            &vhello(v2_methods(), 8),
        )
    }

    fn with_hellos(
        label: &str,
        bodies: Vec<(u8, sley_mutate::value::EntityBodyValue)>,
        dependency_roots: &[StateRoot],
        client_hello: &Hello,
        server_hello: &Hello,
    ) -> Self {
        Self::with_hellos_bodies(label, bodies, dependency_roots, client_hello, server_hello)
    }

    fn with_hellos_bodies(
        label: &str,
        bodies: Vec<(u8, sley_mutate::value::EntityBodyValue)>,
        dependency_roots: &[StateRoot],
        client_hello: &Hello,
        server_hello: &Hello,
    ) -> Self {
        let (temp, _transactions, _) = genesis(label, bodies, dependency_roots);
        Self::open_on_repo(temp, client_hello, server_hello)
    }

    /// Opens a version-aware session on an already-built repository path,
    /// so a second server can share one fixture (for exact session-budget
    /// ceilings derived from a first server's observed work).
    fn alias_on_repo(
        alias_label: &str,
        repository: std::path::PathBuf,
        client_hello: &Hello,
        server_hello: &Hello,
    ) -> Self {
        let temp = sley_repo::test_support::TempDir::new(alias_label);
        Self::open(temp, repository, client_hello, server_hello)
    }

    fn open_on_repo(
        temp: sley_repo::test_support::TempDir,
        client_hello: &Hello,
        server_hello: &Hello,
    ) -> Self {
        let repository = temp.child("repo");
        Self::open(temp, repository, client_hello, server_hello)
    }

    fn open(
        temp: sley_repo::test_support::TempDir,
        repository: std::path::PathBuf,
        client_hello: &Hello,
        server_hello: &Hello,
    ) -> Self {
        let mut server = Server::new_versioned(&repository, client_hello, server_hello).unwrap();
        assert_eq!(server.profile().protocol_version, PROTOCOL_VERSION_V2);
        let handshake = server.handshake_id();
        let open = server
            .answer(&vrequest_frame(
                None,
                0,
                Method::SessionOpen.tag(),
                handshake.as_bytes().to_vec(),
            ))
            .unwrap();
        assert!(!open.failed);
        assert!(open.events.is_empty());
        let (DecodedFrame::Response(frame), _) =
            decode_frame_for_version(&open.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2)
                .unwrap()
        else {
            panic!("session open response");
        };
        assert_eq!(frame.protocol_version, PROTOCOL_VERSION_V2);
        let session = SessionId::from_bytes(frame.body.as_slice().try_into().unwrap());
        let root = sley_txn::TransactionRepository::new(&repository)
            .accepted_head()
            .unwrap()
            .verified_revision()
            .state_root()
            .root;
        Self {
            _temp: temp,
            repository,
            server,
            session,
            root,
            next_request: 1,
        }
    }

    fn call(&mut self, tag: u32, body: Vec<u8>) -> (bool, ProtocolFrame) {
        let answer = self.call_raw(tag, body);
        let (DecodedFrame::Response(frame), _) =
            decode_frame_for_version(&answer.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2)
                .unwrap()
        else {
            panic!("response frame");
        };
        assert_eq!(frame.method, tag);
        assert_eq!(frame.session, Some(self.session));
        assert_eq!(frame.protocol_version, PROTOCOL_VERSION_V2);
        assert!(answer.events.is_empty(), "entity reads never stream");
        (answer.failed, frame)
    }

    fn call_raw(&mut self, tag: u32, body: Vec<u8>) -> crate::server::Answer {
        let answer = self
            .server
            .answer(&vrequest_frame(
                Some(self.session),
                self.next_request,
                tag,
                body,
            ))
            .unwrap();
        self.next_request += 1;
        answer
    }

    fn read(&mut self, tag: u32, entity: EntityId) -> ProtocolFrame {
        let (failed, frame) = self.call(tag, entity_read_body(self.root, entity));
        assert!(
            !failed,
            "{tag} failed: {:?}",
            ProtocolFailure::decode(&frame.body)
        );
        frame
    }

    fn refuse(&mut self, tag: u32, body: Vec<u8>) -> ProtocolFailure {
        let (failed, frame) = self.call(tag, body);
        assert!(failed, "{tag} unexpectedly succeeded");
        ProtocolFailure::decode(&frame.body).unwrap()
    }

    fn budget(&self) -> u64 {
        self.server.remaining_budget(self.session).unwrap()
    }
}

/// Corpus request-shape wires refuse with their recorded codes.
///
/// The 16 `request_record`/`request_range` rejection rows carry full on-wire
/// frames. The test decodes each wire with the production frame decoder,
/// re-issues its method tag and body on a live v2 session negotiated down
/// to the corpus ceilings (minima win, so the recorded over-selected
/// ceilings refuse exactly as authored), and compares the refusal symbol.
/// Malformed records refuse before admission; over-selected ceilings
/// refuse at admission. Owner-layer rows stay in sley-query; relation,
/// sequence, and server-path rows stay with their owners.
#[test]
fn corpus_request_shape_wires_refuse_with_recorded_codes() {
    let methods = v2_methods();
    let client_hello = Hello {
        protocol_versions: vec![PROTOCOL_VERSION, PROTOCOL_VERSION_V2],
        schema_epochs: vec![epoch(0x11)],
        limits: LimitProfile {
            max_frame_bytes: 8_388_608,
            max_entities: 1_024,
            max_edges: 10_000,
            max_depth: 64,
            max_response_bytes: 4_194_304,
            max_work: 10_000_000,
            max_inflight: 16,
            max_sessions: 16,
        },
        methods: methods.clone(),
        features: 1,
        adapters: vec![],
        effects: vec![],
    };
    let mut harness = VServer::with_hellos(
        "corpus-shape-wires",
        executable_bodies(),
        &[],
        &client_hello,
        &vhello(methods, 8),
    );
    let rejected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/entity-read/v2/rejected.json"
    ))
    .unwrap();
    let mut covered = 0;
    for row in rejected["cases"].as_array().unwrap() {
        let layer = row.get("failing_layer").and_then(|value| value.as_str());
        if !matches!(layer, Some("request_record" | "request_range")) {
            continue;
        }
        covered += 1;
        let id = row["id"].as_str().unwrap();
        let wire = corpus_hex(row["input_hex"].as_str().unwrap());
        let decoded = decode_frame_for_version(&wire, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2)
            .unwrap_or_else(|error| panic!("{id}: wire decode: {error:?}"));
        let (DecodedFrame::Request(frame), _) = decoded else {
            panic!("{id}: not a request frame")
        };
        assert!(
            frame.method == ENTITY_VERSION_TAG || frame.method == ENTITY_SIGNATURE_TAG,
            "{id}: entity-read method"
        );
        let failure = harness.refuse(frame.method, frame.body);
        assert_eq!(
            failure.symbol,
            row["expected_symbol"].as_str().unwrap(),
            "{id}: refusal symbol"
        );
    }
    assert_eq!(covered, 16, "request_record plus request_range rows");
}

/// The owner test reproduces these bodies from stored objects. This test
/// independently builds the response envelope from semantic input metadata
/// and the frozen body, then compares the production encoder's bytes and
/// identity. It does not claim to issue the corpus's synthetic session.
#[test]
fn accepted_corpus_response_frames_match_protocol_encoder() {
    let accepted: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/entity-read/v2/accepted.json"
    ))
    .unwrap();
    let inputs: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/entity-read/v2/inputs.json"
    ))
    .unwrap();
    let limits = &inputs["selected_limits"];
    let applied_limits = LimitProfile {
        max_frame_bytes: limits["max_frame_bytes"].as_u64().unwrap(),
        max_entities: limits["max_entities"].as_u64().unwrap(),
        max_edges: limits["max_edges"].as_u64().unwrap(),
        max_depth: u32::try_from(limits["max_depth"].as_u64().unwrap()).unwrap(),
        max_response_bytes: limits["max_response_bytes"].as_u64().unwrap(),
        max_work: limits["max_work"].as_u64().unwrap(),
        max_inflight: u32::try_from(limits["max_inflight"].as_u64().unwrap()).unwrap(),
        max_sessions: u32::try_from(limits["max_sessions"].as_u64().unwrap()).unwrap(),
    };
    let session = SessionId::from_bytes(
        corpus_hex(inputs["context"]["session"].as_str().unwrap())
            .try_into()
            .unwrap(),
    );
    let cases = accepted["cases"].as_object().unwrap();
    assert_eq!(cases.len(), 23);
    for (id, case) in cases {
        let authored = &inputs["cases"][id];
        let body = corpus_hex(case["response_body_hex"].as_str().unwrap());
        let response = decode_entity_read_response(&body).unwrap();
        assert_eq!(
            response.work_units,
            case["work"].as_u64().unwrap(),
            "{id}: work"
        );
        let mut frame = ProtocolFrame {
            protocol_version: PROTOCOL_VERSION_V2,
            session: Some(session),
            request_id: authored["request"]["request_id"].as_u64().unwrap(),
            kind: FrameKind::Response,
            method: u32::try_from(authored["method"].as_u64().unwrap()).unwrap(),
            flags: 0,
            bounds: BoundedContext {
                applied_limits,
                returned_bytes: u64::try_from(body.len()).unwrap(),
                returned_entities: u64::try_from(response.objects.len()).unwrap(),
                returned_edges: 0,
                reached_depth: 0,
                omitted: 0,
                truncated: false,
                continuation: false,
            },
            body,
        };
        assert_eq!(
            frame.bounds.returned_entities,
            case["count_k"].as_u64().unwrap(),
            "{id}: count"
        );
        let encoded = encode_single_frame_direct(&frame, applied_limits.max_frame_bytes).unwrap();
        let generic = encode_frame_for_version(&frame, PROTOCOL_VERSION_V2).unwrap();
        assert_eq!(
            encoded.bytes, generic.bytes,
            "{id}: direct/generic encoders"
        );
        assert_eq!(
            encoded.frame_id, generic.frame_id,
            "{id}: direct/generic identity"
        );
        let expected_wire = corpus_hex(case["response_wire_hex"].as_str().unwrap());
        let expected_id = corpus_hex(case["response_frame_id"].as_str().unwrap());
        assert_eq!(encoded.bytes, expected_wire, "{id}: response wire");
        assert_eq!(
            encoded.frame_id.as_bytes().as_slice(),
            expected_id,
            "{id}: frame identity"
        );
        assert_eq!(
            u64::try_from(encoded.bytes.len()).unwrap(),
            case["response_wire_len"].as_u64().unwrap(),
            "{id}: frame length"
        );
        let (decoded, decoded_id) = decode_frame_for_version(
            &encoded.bytes,
            applied_limits.max_frame_bytes,
            PROTOCOL_VERSION_V2,
        )
        .unwrap();
        assert_eq!(
            decoded,
            DecodedFrame::Response(frame.clone()),
            "{id}: round trip"
        );
        assert_eq!(decoded_id, encoded.frame_id, "{id}: decoded identity");
        // Negative control: another lawful request identity must not match
        // the independently frozen response, even though its body is equal.
        frame.request_id += 1;
        let other = encode_frame_for_version(&frame, PROTOCOL_VERSION_V2).unwrap();
        assert_ne!(other.bytes, expected_wire, "{id}: changed request wire");
        assert_ne!(
            other.frame_id.as_bytes().as_slice(),
            expected_id,
            "{id}: changed request identity"
        );
    }
}

/// Corpus helper: raw bytes from even-length lowercase hex.
fn corpus_hex(text: &str) -> Vec<u8> {
    assert!(text.len().is_multiple_of(2), "hex length must be even");
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let value = |character: u8| match character {
                b'0'..=b'9' => u32::from(character - b'0'),
                b'a'..=b'f' => u32::from(character - b'a') + 10,
                other => panic!("non-hex digit {other}"),
            };
            u8::try_from(value(pair[0]) * 16 + value(pair[1])).unwrap()
        })
        .collect()
}

/// A function whose declared parameter order is nonmonotonic in raw
/// identity order, with varied parameter types, effects, and contracts.
fn nonmonotonic_bodies() -> Vec<(u8, sley_mutate::value::EntityBodyValue)> {
    use sley_mutate::value::{EntityBodyValue, FunctionBody, NamespaceBody, ParameterBody};
    use sley_repo::test_support::{id, set};
    use sley_ssmc::{ParameterRole, TypeExpr, Visibility};
    let function = EntityBodyValue::Function(FunctionBody {
        type_parameters: Vec::new(),
        parameters: vec![id(43), id(41), id(42)],
        result_type: TypeExpr::Tuple(vec![TypeExpr::Bool, TypeExpr::Text]),
        effects: set(&[]),
        entry_block: id(44),
        blocks: vec![id(44)],
        contracts: set(&[]),
        visibility: Visibility::Private,
    });
    let types = [
        TypeExpr::Bool,
        TypeExpr::Text,
        TypeExpr::Option(Box::new(TypeExpr::Bool)),
    ];
    let mut bodies = vec![(40, function)];
    for (position, value_type) in types.into_iter().enumerate() {
        let byte = [43_u8, 41, 42][position];
        bodies.push((
            byte,
            EntityBodyValue::Parameter(ParameterBody {
                owner: id(40),
                role: ParameterRole::Function,
                ordinal: u32::try_from(position).unwrap(),
                value_type,
            }),
        ));
    }
    bodies.push((
        44,
        EntityBodyValue::Namespace(NamespaceBody {
            parent: None,
            members: sley_mutate::value::EntityIdSet::from_unsorted(vec![]).unwrap(),
        }),
    ));
    bodies
}

#[test]
fn legacy_server_refuses_v2_tags_but_keeps_opaque_intersection() {
    let mut methods = all_methods();
    methods.push(ENTITY_VERSION_TAG);
    methods.push(ENTITY_SIGNATURE_TAG);
    methods.push(999);
    methods.sort_unstable();
    let client_hello = hello(methods.clone(), 4);
    let server_hello = hello(methods, 8);
    let profile = negotiate(&client_hello, &server_hello).unwrap();
    assert!(profile.methods.contains(&ENTITY_VERSION_TAG));
    assert!(profile.methods.contains(&ENTITY_SIGNATURE_TAG));
    assert!(profile.methods.contains(&999));
    let offered = Server::offered_hello().unwrap();
    assert_eq!(offered.protocol_versions, vec![PROTOCOL_VERSION]);
    assert!(!offered.methods.contains(&ENTITY_VERSION_TAG));
    let mut harness = Harness::with_hellos("legacy-v2-refusal", client_hello, server_hello);
    let control = harness.fail(Method::QueryRoot, b"junk".to_vec());
    assert_eq!(control.code, ProtocolErrorCode::PayloadInvalid.numeric());
    let raw = encode_frame(&ProtocolFrame {
        protocol_version: PROTOCOL_VERSION,
        session: Some(harness.session),
        request_id: harness.next_request,
        kind: FrameKind::Request,
        method: ENTITY_VERSION_TAG,
        flags: 0,
        bounds: BoundedContext::none(),
        body: b"junk".to_vec(),
    })
    .unwrap()
    .bytes;
    harness.next_request += 1;
    let answer = harness.server.answer(&raw).unwrap();
    assert!(answer.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&answer.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!("response frame");
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::MethodUnsupported.numeric()
    );
}

#[test]
fn legacy_server_with_v2_selection_keeps_v1_framing() {
    let client_hello = vhello(v2_methods(), 4);
    let server_hello = vhello(v2_methods(), 8);
    let (temp, _transactions, _) = genesis("legacy-v2-select", executable_bodies(), &[]);
    let repository = temp.child("repo");
    let mut server = Server::new(&repository, &client_hello, &server_hello).unwrap();
    assert_eq!(server.profile().protocol_version, PROTOCOL_VERSION_V2);
    let v2_frame = vrequest_frame(None, 0, Method::SessionOpen.tag(), vec![0; 32]);
    let answer = server.answer(&v2_frame).unwrap();
    assert!(answer.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&answer.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!("response frame");
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::VersionUnsupported.numeric()
    );
    // A version 1 frame decodes on the legacy path but meets the above-1
    // opaque selection at the dispatch claim check: the legacy server with
    // a version 2 selection is unserviceable in both directions (contract
    // SMP1 section 2), v1 answering `PROTOCOL_DOWNGRADE`.
    let v1_frame = encode_frame_for_version(
        &ProtocolFrame {
            protocol_version: PROTOCOL_VERSION,
            session: None,
            request_id: 0,
            kind: FrameKind::Request,
            method: Method::SessionOpen.tag(),
            flags: 0,
            bounds: BoundedContext::none(),
            body: vec![0; 32],
        },
        PROTOCOL_VERSION,
    )
    .unwrap()
    .bytes;
    let answer = server.answer(&v1_frame).unwrap();
    assert!(answer.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame(&answer.frame.bytes, MAX_FRAME_BYTES).unwrap()
    else {
        panic!("response frame");
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::Downgrade.numeric()
    );
}

#[test]
fn versioned_hello_travels_at_frame_one_and_selects_two() {
    let offered = Server::offered_hello_versioned().unwrap();
    assert_eq!(
        offered.protocol_versions,
        vec![PROTOCOL_VERSION, PROTOCOL_VERSION_V2]
    );
    assert!(offered.methods.contains(&ENTITY_VERSION_TAG));
    assert!(offered.methods.contains(&ENTITY_SIGNATURE_TAG));
    assert_eq!(
        offered.methods.len(),
        Method::V2_ALL
            .iter()
            .filter(|method| !method.is_reserved())
            .count()
    );
    let encoded = crate::encode_hello_frame(&offered).unwrap();
    let (decoded, _) = decode_frame(&encoded.bytes, MAX_FRAME_BYTES).unwrap();
    let DecodedFrame::Hello(label) = decoded else {
        panic!("hello frame");
    };
    assert_eq!(
        label.protocol_versions,
        vec![PROTOCOL_VERSION, PROTOCOL_VERSION_V2]
    );
    decode_frame_for_version(&encoded.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION).unwrap();
    assert_eq!(
        decode_frame_for_version(&encoded.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2)
            .unwrap_err()
            .code(),
        ProtocolErrorCode::Downgrade
    );
    let mut vhello_frame = vhello(v2_methods(), 4);
    vhello_frame.schema_epochs = offered.schema_epochs.clone();
    let selected = negotiate_versioned(&vhello_frame, &offered).unwrap();
    assert_eq!(selected.protocol_version, PROTOCOL_VERSION_V2);
    let (reselected, reidentity) = negotiate_identity_versioned(&vhello_frame, &offered).unwrap();
    assert_eq!(reselected.methods, selected.methods);
    let _ = reidentity;
}

// ---------------------------------------------------------------------------
// Protocol version 3 native negotiation (NATIVE_TEST_ADMISSION_V1 App. C)
// ---------------------------------------------------------------------------

use crate::{
    FEATURE_NATIVE_TESTS_V1, PROTOCOL_VERSION_V3, Retryability, TESTS_AFFECTED_TAG,
    TESTS_ATTEMPT_STATUS_TAG, TESTS_REPLAY_TAG, TESTS_REPORT_READ_TAG, TESTS_SELECTED_TAG,
};

fn v3_methods() -> Vec<u32> {
    Method::V3_ALL
        .iter()
        .filter(|method| !method.is_reserved())
        .map(|method| method.tag())
        .collect()
}

/// The native-capable offer: the non-reserved v3 methods plus the live
/// native reads 601/602/605 (still-reserved 606-607 stay unoffered).
fn v3_offered_methods() -> Vec<u32> {
    Method::V3_ALL
        .iter()
        .filter(|method| !method.is_reserved() || method.is_native_test())
        .map(|method| method.tag())
        .collect()
}

fn v3hello(methods: Vec<u32>, features: u32) -> Hello {
    Hello {
        protocol_versions: vec![PROTOCOL_VERSION, PROTOCOL_VERSION_V2, PROTOCOL_VERSION_V3],
        schema_epochs: vec![epoch(0x11)],
        limits: LimitProfile {
            max_frame_bytes: 8_388_608,
            max_entities: 65_535,
            max_edges: 400_000,
            max_depth: 65_535,
            max_response_bytes: 8_388_608,
            max_work: 100_000_000,
            max_inflight: 4,
            max_sessions: 256,
        },
        methods,
        features,
        adapters: vec![],
        effects: vec![],
    }
}

fn v3request_frame(
    session: Option<SessionId>,
    request_id: u64,
    tag: u32,
    body: Vec<u8>,
) -> Vec<u8> {
    encode_frame_for_version(
        &ProtocolFrame {
            protocol_version: PROTOCOL_VERSION_V3,
            session,
            request_id,
            kind: FrameKind::Request,
            method: tag,
            flags: 0,
            bounds: BoundedContext::none(),
            body,
        },
        PROTOCOL_VERSION_V3,
    )
    .unwrap()
    .bytes
}

fn open_v3_session(server: &mut Server) -> SessionId {
    let open = server
        .answer(&v3request_frame(
            None,
            0,
            Method::SessionOpen.tag(),
            server.handshake_id().as_bytes().to_vec(),
        ))
        .unwrap();
    assert!(!open.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame_for_version(&open.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V3).unwrap()
    else {
        panic!("v3 session open response");
    };
    assert_eq!(frame.protocol_version, PROTOCOL_VERSION_V3);
    SessionId::from_bytes(frame.body.as_slice().try_into().unwrap())
}

fn call_v3(server: &mut Server, session: SessionId, request_id: u64, tag: u32) -> ProtocolFailure {
    let answer = server
        .answer(&v3request_frame(Some(session), request_id, tag, Vec::new()))
        .unwrap();
    assert!(answer.failed, "{tag} unexpectedly succeeded");
    let (DecodedFrame::Response(frame), _) =
        decode_frame_for_version(&answer.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V3)
            .unwrap()
    else {
        panic!("v3 response frame");
    };
    assert_eq!(frame.method, tag);
    ProtocolFailure::decode(&frame.body).unwrap()
}

#[test]
fn v3_offered_hello_names_v3_table_and_native_bit() {
    let offered = Server::offered_hello_v3().unwrap();
    assert_eq!(
        offered.protocol_versions,
        vec![PROTOCOL_VERSION, PROTOCOL_VERSION_V2, PROTOCOL_VERSION_V3]
    );
    assert_eq!(offered.methods, v3_offered_methods());
    assert_eq!(offered.methods.len(), v3_methods().len() + 5);
    for tag in [
        TESTS_SELECTED_TAG,
        TESTS_AFFECTED_TAG,
        TESTS_REPORT_READ_TAG,
        TESTS_REPLAY_TAG,
        TESTS_ATTEMPT_STATUS_TAG,
    ] {
        assert!(offered.methods.contains(&tag), "v3 offers live {tag}");
    }
    assert!(offered.features & FEATURE_NATIVE_TESTS_V1 != 0);
    // The v3 hello still travels at frame version 1 so older peers can
    // read the offer and negotiate down.
    let encoded = crate::encode_hello_frame(&offered).unwrap();
    let (decoded, _) = decode_frame(&encoded.bytes, MAX_FRAME_BYTES).unwrap();
    let DecodedFrame::Hello(label) = decoded else {
        panic!("hello frame");
    };
    assert_eq!(
        label.protocol_versions,
        vec![PROTOCOL_VERSION, PROTOCOL_VERSION_V2, PROTOCOL_VERSION_V3]
    );
    decode_frame_for_version(&encoded.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION).unwrap();
    assert_eq!(
        decode_frame_for_version(&encoded.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2)
            .unwrap_err()
            .code(),
        ProtocolErrorCode::Downgrade
    );
    // Offered against offered selects v3 with the bit and no native tags.
    let selected = negotiate_versioned(&offered, &offered).unwrap();
    assert_eq!(selected.protocol_version, PROTOCOL_VERSION_V3);
    assert!(selected.features & FEATURE_NATIVE_TESTS_V1 != 0);
    assert_eq!(selected.methods, offered.methods);
}

#[test]
fn v3_native_calls_route_live_with_bit_and_refuse_reserved_without() {
    // With the bit, every native tag negotiates and reaches dispatch:
    // the live 606/607 tags refuse an empty body as malformed, proving
    // they traveled past reservation into their handlers. Without the
    // bit, negotiation strips every native tag and the same calls refuse
    // as reserved without running native semantics.
    let mut offered_native = v3_methods();
    offered_native.extend_from_slice(&[TESTS_REPLAY_TAG, TESTS_ATTEMPT_STATUS_TAG]);
    offered_native.sort_unstable();
    let bit = FEATURE_CANCEL | FEATURE_STREAM | FEATURE_NATIVE_TESTS_V1;
    let (temp, _, _) = genesis("v3-native-live", executable_bodies(), &[]);
    let repository = temp.child("repo");
    let mut server = Server::new_versioned(
        &repository,
        &v3hello(offered_native.clone(), bit),
        &v3hello(offered_native.clone(), bit),
    )
    .unwrap();
    assert_eq!(server.profile().protocol_version, PROTOCOL_VERSION_V3);
    assert!(server.profile().admits(Method::TestsReplay));
    assert!(server.profile().admits(Method::TestsAttemptStatus));
    let session = open_v3_session(&mut server);
    for (request_id, tag) in [TESTS_REPLAY_TAG, TESTS_ATTEMPT_STATUS_TAG]
        .into_iter()
        .enumerate()
    {
        let failure = call_v3(&mut server, session, request_id as u64 + 1, tag);
        assert_eq!(failure.code, ProtocolErrorCode::PayloadInvalid.numeric());
    }
    // Without the bit the tags never reach admission: the selection
    // drops them, and dispatch still refuses the reserved method (the
    // reserved arm dominates the gate for unimplemented methods, so the
    // bit's effect shows in `admits`, not in the refusal code).
    let plain = FEATURE_CANCEL | FEATURE_STREAM;
    let mut unnegotiated = Server::new_versioned(
        &repository,
        &v3hello(offered_native.clone(), plain),
        &v3hello(offered_native, plain),
    )
    .unwrap();
    assert_eq!(unnegotiated.profile().protocol_version, PROTOCOL_VERSION_V3);
    assert!(!unnegotiated.profile().admits(Method::TestsReplay));
    let session = open_v3_session(&mut unnegotiated);
    let failure = call_v3(&mut unnegotiated, session, 1, TESTS_REPLAY_TAG);
    assert_eq!(failure.code, ProtocolErrorCode::MethodUnsupported.numeric());
    assert_eq!(failure.details, RESERVED_SEAM_620_DETAIL);
    assert_eq!(failure.retryability, Retryability::AfterCapability);
}

#[test]
fn native_selection_refuses_reserved_on_v1_paths() {
    // v1 and v2 refuse the reserved native selections byte-for-byte as
    // before: `tests.selected`/`tests.affected` decode, then refuse as
    // reserved with the seam detail and AfterCapability. `tests.report_read`,
    // `tests.replay` and `tests.attempt_status` never existed in the frozen
    // v1/v2 tables, so they refuse at decode as unsupported with no seam
    // detail.
    let mut harness = Harness::new("smp1-native-v1-refusal");
    for method in [Method::TestsSelected, Method::TestsAffected] {
        let failure = harness.fail(method, Vec::new());
        assert_eq!(failure.details, RESERVED_SEAM_620_DETAIL);
        assert_eq!(failure.retryability, Retryability::AfterCapability);
    }
    for method in [
        Method::TestsReportRead,
        Method::TestsReplay,
        Method::TestsAttemptStatus,
    ] {
        let failure = harness.fail(method, Vec::new());
        assert_eq!(failure.code, ProtocolErrorCode::MethodUnsupported.numeric());
        assert!(failure.details.is_empty());
    }
}

// ---------------------------------------------------------------------------
// 601/602 live selection reads (NATIVE_TEST_ADMISSION_V1 App. C rev3)
// ---------------------------------------------------------------------------

/// One live `TestCase` over the `executable_bodies` Bool function: byte 40
/// targets byte 30 with two Bool inputs, so the explicit-root plan selects
/// exactly it when the caller names it and the policy requires nothing.
fn diagnostic_bodies() -> Vec<(u8, sley_mutate::value::EntityBodyValue)> {
    use sley_mutate::value::{EntityBodyValue, TestCaseBody};
    use sley_repo::test_support::id;
    use sley_ssmc::{ConstData, ConstValue, EffectEnvironment, ExpectedOutcome, TypeExpr};
    fn boolean(value: bool) -> ConstValue {
        ConstValue {
            value_type: TypeExpr::Bool,
            data: ConstData::Bool(value),
        }
    }
    let mut bodies = executable_bodies();
    bodies.push((
        40,
        EntityBodyValue::TestCase(TestCaseBody {
            target: id(30),
            inputs: vec![boolean(true), boolean(false)],
            effect_environment: EffectEnvironment::Replay(vec![]),
            expected: ExpectedOutcome::Value(boolean(true)),
            observations: vec![],
            resource_limits: sley_ssmc::ResourceLimits {
                fuel: 100,
                memory_bytes: 1024,
                output_bytes: 64,
                effect_count: 0,
                call_depth: 2,
                wall_timeout_millis: 1000,
            },
        }),
    ));
    bodies
}

/// Test-only diagnostic executor: coherent no-result evidence per selected
/// test (rejected report, attestation without an execution report) in the
/// session workspace with entry-equal declared limits, so assembly reaches
/// the all-rejected status without claiming a real run. The commit entry
/// point stays refused: this double can never back a commit.
struct DiagnosticRejector {
    workspace: sley_id::WorkspaceId,
    principal: sley_id::PrincipalId,
    supervisor_config: Vec<u8>,
    supervisor_config_id: [u8; 32],
    invocations: std::cell::Cell<usize>,
}

impl DiagnosticRejector {
    fn new(workspace: sley_id::WorkspaceId, principal: sley_id::PrincipalId) -> Self {
        use sley_tests::{Caller, Property, SupervisorConfigParts, SupervisorConfigV1};
        // The exact contracted supervisor property set: `build` rejects
        // unknown properties, so diagnostics reuse the frozen shape.
        let fixed = [
            ("CapabilityBoundingSet", "empty"),
            ("DynamicUser", "yes"),
            ("KillMode", "control-group"),
            ("MemoryAccounting", "yes"),
            ("MemorySwapMax", "0"),
            ("NoNewPrivileges", "yes"),
            ("PrivateNetwork", "yes"),
            ("PrivateTmp", "yes"),
            ("ProtectControlGroups", "yes"),
            ("ProtectHome", "yes"),
            ("ProtectSystem", "strict"),
            ("SendSIGKILL", "yes"),
            ("TasksMax", "1"),
            ("TimeoutStopUSec", "2000000"),
        ];
        let mut properties: Vec<Property> = fixed
            .iter()
            .map(|(name, value)| Property {
                name: (*name).to_owned(),
                value: (*value).to_owned(),
            })
            .collect();
        properties.push(Property {
            name: "MemoryMax".to_owned(),
            value: "1048576".to_owned(),
        });
        properties.push(Property {
            name: "RuntimeMaxUSec".to_owned(),
            value: "1000000".to_owned(),
        });
        properties.sort_by(|left, right| left.name.cmp(&right.name));
        let config = SupervisorConfigV1::build(SupervisorConfigParts {
            worker_digest: [0x21; 32],
            supervisor_digest: [0x22; 32],
            properties,
            callers: vec![Caller {
                uid: 0,
                workspace,
                principal,
            }],
            page_size: 4096,
            cleanup_millis: 2000,
            launch_profile: 1,
        })
        .expect("diagnostic supervisor config builds");
        let supervisor_config_id = *config.id().as_bytes();
        Self {
            workspace,
            principal,
            supervisor_config: config.stored_bytes().to_vec(),
            supervisor_config_id,
            invocations: std::cell::Cell::new(0),
        }
    }
}

impl sley_txn::NativeTestExecutor for DiagnosticRejector {
    fn execute(
        &self,
        _plan: &sley_tests::NativeTestPlanV1,
        _validated: &sley_policy::ValidatedCandidatePlan,
    ) -> Result<Vec<sley_txn::ExecutedNativeTest>, sley_txn::NativeCommitError> {
        Err(sley_txn::NativeCommitError::ExecutorUnavailable)
    }

    fn execute_diagnostic(
        &self,
        plan: &sley_tests::NativeTestPlanV1,
        _objects: &std::collections::BTreeMap<sley_id::ObjectId, &[u8]>,
    ) -> Result<Vec<sley_txn::ExecutedNativeTest>, sley_txn::NativeCommitError> {
        use sley_tests::{
            MeasuredTestAttestationParts, MeasuredTestAttestationV1, MemoryEvents,
            NativeExecutionEvidence, NativeExecutionReportParts, NativeExecutionReportV1,
            REJECT_PHASE_EXECUTION, RejectedEvidence, TERMINATION_PRELAUNCH_REFUSED,
        };
        self.invocations.set(self.invocations.get() + 1);
        let mut out = Vec::with_capacity(plan.selected().len());
        for entry in plan.selected() {
            let rejected = RejectedEvidence::from_parts(
                REJECT_PHASE_EXECUTION,
                29211,
                "NATIVE_TEST_EXECUTION_REJECTED",
            )
            .expect("synthetic rejection builds");
            let report = NativeExecutionReportV1::build(NativeExecutionReportParts {
                plan_id: plan.plan_id(),
                test_entity: entry.test_entity,
                test_object: entry.test_object,
                target_object: entry.target_object,
                evidence: NativeExecutionEvidence::Rejected(rejected),
            })
            .expect("synthetic report builds");
            let attestation = MeasuredTestAttestationV1::build(MeasuredTestAttestationParts {
                key_id: [0xB2; 32],
                trust_policy_id: [0xB3; 32],
                supervisor_config_id: self.supervisor_config_id,
                plan_id: plan.plan_id(),
                test_object: entry.test_object,
                execution_report_id: None,
                attempt_nonce: [0xC3; 32],
                workspace: self.workspace,
                principal: self.principal,
                caller_uid: 0,
                declared_limits: entry.declared_limits,
                installed_memory_cap: entry.declared_limits.memory_bytes,
                elapsed_ns: 0,
                measured_memory_peak: 0,
                memory_events: MemoryEvents {
                    max: entry.declared_limits.memory_bytes,
                    oom: 0,
                    oom_kill: 0,
                },
                termination: TERMINATION_PRELAUNCH_REFUSED,
                complete_output: false,
                empty_cgroup_confirmed: true,
                recorded_unix_millis: 1_000,
                signature: [0xA5; 64],
            })
            .expect("synthetic attestation builds");
            out.push(sley_txn::ExecutedNativeTest {
                test_entity: entry.test_entity,
                execution_stored: report.stored_bytes().to_vec(),
                attestation_stored: attestation.stored_bytes().to_vec(),
                supervisor_config_stored: self.supervisor_config.clone(),
            });
        }
        Ok(out)
    }
}

/// A v3 server over the diagnostic genesis with the rejecting executor
/// installed, plus its session: the genesis workspace is byte 1 and its
/// principal byte 2, matching the test-support genesis convention.
fn diagnostic_server(label: &str) -> (sley_repo::test_support::TempDir, Server, SessionId) {
    use sley_id::{PrincipalId, WorkspaceId};
    let (temp, _transactions, _genesis_id) = genesis(label, diagnostic_bodies(), &[]);
    let repository = temp.child("repo");
    let bit = FEATURE_CANCEL | FEATURE_STREAM | FEATURE_NATIVE_TESTS_V1;
    let methods = v3_offered_methods();
    let mut server = Server::new_versioned(
        &repository,
        &v3hello(methods.clone(), bit),
        &v3hello(methods, bit),
    )
    .unwrap();
    assert_eq!(server.profile().protocol_version, PROTOCOL_VERSION_V3);
    let session = open_v3_session(&mut server);
    server.set_executor(Box::new(DiagnosticRejector::new(
        WorkspaceId::from_bytes([1; 32]),
        PrincipalId::from_bytes([2; 32]),
    )));
    (temp, server, session)
}

/// The accepted head root behind a diagnostic server repository.
fn diagnostic_head_root(temp: &sley_repo::test_support::TempDir) -> sley_id::StateRoot {
    sley_txn::TransactionRepository::new(temp.child("repo"))
        .accepted_head()
        .unwrap()
        .verified_revision()
        .state_root()
        .root
}

/// One 601 `tests.selected` request body: root, selected ids, profile,
/// attempt.
fn selected_body(root: sley_id::StateRoot, selected: &[u8], attempt: [u8; 16]) -> Vec<u8> {
    use sley_repo::test_support::id;
    let profile = sley_tests::native_execution_profile_id();
    encode_record(&[
        (1, root.as_bytes().to_vec()),
        (
            2,
            sley_scb1::encode_list(
                &selected
                    .iter()
                    .map(|byte| id(*byte).as_bytes().to_vec())
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
        ),
        (3, profile.as_bytes().to_vec()),
        (4, attempt.to_vec()),
    ])
    .unwrap()
}

/// Answers one v3 call, asserting success and returning the frame.
fn call_v3_ok(
    server: &mut Server,
    session: SessionId,
    request_id: u64,
    tag: u32,
    body: Vec<u8>,
) -> ProtocolFrame {
    let answer = server
        .answer(&v3request_frame(Some(session), request_id, tag, body))
        .unwrap();
    let (DecodedFrame::Response(frame), _) =
        decode_frame_for_version(&answer.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V3)
            .unwrap()
    else {
        panic!("v3 response frame");
    };
    assert!(
        !answer.failed,
        "{tag} failed: {:?}",
        ProtocolFailure::decode(&frame.body)
    );
    assert_eq!(frame.method, tag);
    frame
}

/// Answers one v3 call, asserting failure and returning the refusal.
fn call_v3_fail(
    server: &mut Server,
    session: SessionId,
    request_id: u64,
    tag: u32,
    body: Vec<u8>,
) -> ProtocolFailure {
    let answer = server
        .answer(&v3request_frame(Some(session), request_id, tag, body))
        .unwrap();
    assert!(answer.failed, "{tag} unexpectedly succeeded");
    let (DecodedFrame::Response(frame), _) =
        decode_frame_for_version(&answer.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V3)
            .unwrap()
    else {
        panic!("v3 response frame");
    };
    assert_eq!(frame.method, tag);
    ProtocolFailure::decode(&frame.body).unwrap()
}

#[test]
fn selected_runs_replays_and_conflicts_on_attempt_binding() {
    let (temp, mut server, session) = diagnostic_server("v3-selected-live");
    let root = diagnostic_head_root(&temp);
    let attempt = [0xA0; 16];
    let first = call_v3_ok(
        &mut server,
        session,
        1,
        TESTS_SELECTED_TAG,
        selected_body(root, &[40], attempt),
    );
    let fields = fields_of(&first.body, 6);
    assert_eq!(fields[0].len(), 32, "plan id");
    assert_eq!(fields[1].len(), 32, "report id");
    // Status 3: every entry execution-rejected with no observation.
    assert_eq!(fields[2], encode_uvar(3));
    assert_eq!(fields[3], encode_uvar(1), "exactly the named test");
    assert_eq!(fields[4].len(), 32, "report token");
    assert_ne!(fields[4], vec![0; 32], "token unpredictable");
    let total = fields[5].clone();
    assert_ne!(total, encode_uvar(0), "report stored bytes");
    // Identical resubmission replays the cached response byte-for-byte
    // without re-executing.
    let replay = call_v3_ok(
        &mut server,
        session,
        2,
        TESTS_SELECTED_TAG,
        selected_body(root, &[40], attempt),
    );
    assert_eq!(replay.body, first.body, "identical bindings replay");
    // Divergent bindings under the same attempt refuse as a conflict:
    // the attempt id keys the cache, so the same id with an emptied
    // selection cannot replay and cannot silently re-execute either.
    let conflict = call_v3_fail(
        &mut server,
        session,
        3,
        TESTS_SELECTED_TAG,
        selected_body(root, &[], attempt),
    );
    assert_eq!(conflict.symbol, "NATIVE_ATTEMPT_CONFLICT");
    // Unknown selections refuse through the preserved plan failure,
    // never as a status.
    let unknown = call_v3_fail(
        &mut server,
        session,
        4,
        TESTS_SELECTED_TAG,
        selected_body(root, &[41], [0xA2; 16]),
    );
    assert_eq!(unknown.symbol, "NATIVE_TEST_SELECTION_INVALID");
}

#[test]
fn selected_without_executor_writes_nothing() {
    let (temp, mut server, session) = diagnostic_server("v3-selected-no-executor");
    let root = diagnostic_head_root(&temp);
    // Drop the executor: with no configured dispatch the call fails
    // having stored no report, minted no token, cached no attempt.
    server.set_executor(Box::new(NoDiagnosticExecutor));
    let failure = call_v3_fail(
        &mut server,
        session,
        1,
        TESTS_SELECTED_TAG,
        selected_body(root, &[40], [0xB0; 16]),
    );
    assert_eq!(failure.symbol, "NATIVE_EXECUTOR_UNAVAILABLE");
}

/// Test-only executor refusing every diagnostic outright, like the
/// commit-only default: the call fails before anything is written.
struct NoDiagnosticExecutor;

impl sley_txn::NativeTestExecutor for NoDiagnosticExecutor {
    fn execute(
        &self,
        _plan: &sley_tests::NativeTestPlanV1,
        _validated: &sley_policy::ValidatedCandidatePlan,
    ) -> Result<Vec<sley_txn::ExecutedNativeTest>, sley_txn::NativeCommitError> {
        Err(sley_txn::NativeCommitError::ExecutorUnavailable)
    }
}

#[test]
fn selected_checks_profile_root_and_renewal_invalidation() {
    let (temp, mut server, session) = diagnostic_server("v3-selected-guards");
    let root = diagnostic_head_root(&temp);
    // A foreign execution profile is a malformed request.
    let profiled = call_v3_fail(
        &mut server,
        session,
        1,
        TESTS_SELECTED_TAG,
        encode_record(&[
            (1, root.as_bytes().to_vec()),
            (
                2,
                sley_scb1::encode_list(&[sley_repo::test_support::id(40).as_bytes().to_vec()])
                    .unwrap(),
            ),
            (3, vec![0x77; 32]),
            (4, [0xC0; 16].to_vec()),
        ])
        .unwrap(),
    );
    assert_eq!(profiled.code, ProtocolErrorCode::PayloadInvalid.numeric());
    // A root that is not the accepted head is stale, never executed.
    let stale = call_v3_fail(
        &mut server,
        session,
        2,
        TESTS_SELECTED_TAG,
        selected_body(
            sley_id::StateRoot::from_bytes([0xFF; 32]),
            &[40],
            [0xC1; 16],
        ),
    );
    assert_eq!(stale.symbol, "SESSION_STALE_HANDLE");
    // One success, then renewal kills the token and its attempt: the
    // same bindings re-execute fresh instead of replaying dead state.
    let attempt = [0xC2; 16];
    let first = call_v3_ok(
        &mut server,
        session,
        3,
        TESTS_SELECTED_TAG,
        selected_body(root, &[40], attempt),
    );
    let renew = server
        .answer(&v3request_frame(
            Some(session),
            4,
            Method::SessionRenew.tag(),
            session.as_bytes().to_vec(),
        ))
        .unwrap();
    assert!(!renew.failed);
    let second = call_v3_ok(
        &mut server,
        session,
        5,
        TESTS_SELECTED_TAG,
        selected_body(root, &[40], attempt),
    );
    assert_ne!(
        second.body, first.body,
        "renewal invalidates the cached attempt"
    );
    let fields = fields_of(&second.body, 6);
    assert_eq!(fields[2], encode_uvar(3));
    assert_eq!(fields[3], encode_uvar(1));
}

#[test]
fn affected_preserves_static_validation_failures() {
    // 602 validates the candidate exactly like `candidate.validate`
    // before deriving anything: undecodable bytes keep their static
    // failure instead of reaching selection.
    let (_temp, mut server, session) = diagnostic_server("v3-affected-static");
    let profile = sley_tests::native_execution_profile_id();
    let failure = call_v3_fail(
        &mut server,
        session,
        1,
        TESTS_AFFECTED_TAG,
        encode_record(&[
            (1, b"not-a-candidate".to_vec()),
            (2, profile.as_bytes().to_vec()),
            (3, [0xD0; 16].to_vec()),
        ])
        .unwrap(),
    );
    assert!(
        !failure.symbol.is_empty(),
        "static failure keeps its symbol"
    );
}

// ---------------------------------------------------------------------------
// 605 live report paging (NATIVE_TEST_ADMISSION_V1 App. C rev4)
// ---------------------------------------------------------------------------

/// One 605 `tests.report_read` request body: `token`, `offset`, `max_bytes`.
fn report_read_body(token: &[u8], offset: u64, max_bytes: u64) -> Vec<u8> {
    encode_record(&[
        (1, token.to_vec()),
        (2, encode_uvar(offset)),
        (3, encode_uvar(max_bytes)),
    ])
    .unwrap()
}

/// Decodes one canonical uvar from exact bytes.
fn uvar_of(bytes: &[u8]) -> u64 {
    let mut value = 0_u64;
    let mut shift = 0_u32;
    for (index, byte) in bytes.iter().enumerate() {
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            assert_eq!(index + 1, bytes.len(), "exact uvar bytes");
            return value;
        }
        shift += 7;
    }
    panic!("unterminated uvar");
}

/// Decodes one sized byte string (uvar length prefix plus bytes).
fn sized_of(bytes: &[u8]) -> Vec<u8> {
    let mut offset = 0_usize;
    let mut length = 0_usize;
    let mut shift = 0_u32;
    loop {
        let byte = bytes[offset];
        offset += 1;
        length |= (usize::from(byte & 0x7f)) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    bytes[offset..offset + length].to_vec()
}

/// Runs 601 once and returns the bound report id, token, and total bytes.
fn selected_report(
    server: &mut Server,
    session: SessionId,
    root: sley_id::StateRoot,
    attempt: [u8; 16],
) -> (Vec<u8>, Vec<u8>, u64) {
    let selected = call_v3_ok(
        server,
        session,
        1,
        TESTS_SELECTED_TAG,
        selected_body(root, &[40], attempt),
    );
    let fields = fields_of(&selected.body, 6);
    let total = uvar_of(&fields[5]);
    assert!(total > 0, "report stored bytes");
    (fields[1].clone(), fields[4].clone(), total)
}

#[test]
fn report_read_pages_full_report_then_refuses_past_end() {
    let (temp, mut server, session) = diagnostic_server("v3-report-paging");
    let root = diagnostic_head_root(&temp);
    let (report_id, token, total) = selected_report(&mut server, session, root, [0xD0; 16]);
    // One full read: identity and root echo the minted capability, the
    // offset echoes the request, and the final page links nowhere.
    let page = call_v3_ok(
        &mut server,
        session,
        2,
        TESTS_REPORT_READ_TAG,
        report_read_body(&token, 0, total),
    );
    let fields = fields_of(&page.body, 6);
    assert_eq!(fields[0], report_id, "bound report id");
    assert_eq!(fields[1], root.as_bytes().to_vec(), "bound root");
    assert_eq!(fields[2], encode_uvar(0), "offset echo");
    assert_eq!(fields[3], encode_uvar(total), "total echo");
    assert_eq!(sized_of(&fields[4]).len(), total as usize, "full page");
    assert_eq!(
        fields[5],
        sley_scb1::encode_option_uvar(None).unwrap(),
        "final page links nowhere"
    );
    // Reading exactly at the end answers an empty final page.
    let end = call_v3_ok(
        &mut server,
        session,
        3,
        TESTS_REPORT_READ_TAG,
        report_read_body(&token, total, 1),
    );
    let end_fields = fields_of(&end.body, 6);
    assert_eq!(end_fields[2], encode_uvar(total));
    assert!(sized_of(&end_fields[4]).is_empty());
    assert_eq!(end_fields[5], sley_scb1::encode_option_uvar(None).unwrap());
    // Reading past the end is malformed, never an empty page.
    let past = call_v3_fail(
        &mut server,
        session,
        4,
        TESTS_REPORT_READ_TAG,
        report_read_body(&token, total + 1, 1),
    );
    assert_eq!(past.code, ProtocolErrorCode::PayloadInvalid.numeric());
}

#[test]
fn report_read_slices_middle_pages_with_next_links() {
    let (temp, mut server, session) = diagnostic_server("v3-report-slices");
    let root = diagnostic_head_root(&temp);
    let (report_id, token, total) = selected_report(&mut server, session, root, [0xD1; 16]);
    // One-byte pages walk the whole report: every non-final page links
    // its successor, and the concatenated slices equal the full page.
    let full = call_v3_ok(
        &mut server,
        session,
        2,
        TESTS_REPORT_READ_TAG,
        report_read_body(&token, 0, total),
    );
    let full_bytes = sized_of(&fields_of(&full.body, 6)[4]);
    let mut offset = 0_u64;
    let mut request_id = 3_u64;
    let mut sliced = Vec::new();
    while offset < total {
        let page = call_v3_ok(
            &mut server,
            session,
            request_id,
            TESTS_REPORT_READ_TAG,
            report_read_body(&token, offset, 1),
        );
        request_id += 1;
        let fields = fields_of(&page.body, 6);
        assert_eq!(fields[0], report_id);
        assert_eq!(fields[2], encode_uvar(offset));
        let bytes = sized_of(&fields[4]);
        assert_eq!(bytes.len(), 1, "one-byte slice");
        sliced.extend_from_slice(&bytes);
        let next = offset + 1;
        let expected = if next == total {
            sley_scb1::encode_option_uvar(None).unwrap()
        } else {
            sley_scb1::encode_option_uvar(Some(next)).unwrap()
        };
        assert_eq!(fields[5], expected, "next link at {offset}");
        offset = next;
    }
    assert_eq!(sliced, full_bytes, "slices reassemble the report");
}

#[test]
fn report_read_refuses_unknown_foreign_and_malformed() {
    let (temp, mut server, session) = diagnostic_server("v3-report-refusals");
    let root = diagnostic_head_root(&temp);
    let (_report_id, token, total) = selected_report(&mut server, session, root, [0xD2; 16]);
    // An unknown token refuses without a lookup.
    let unknown = call_v3_fail(
        &mut server,
        session,
        2,
        TESTS_REPORT_READ_TAG,
        report_read_body(&[0xE0; 32], 0, total),
    );
    assert_eq!(unknown.symbol, "NATIVE_TOKEN_INVALID");
    // A token minted for another session refuses there too.
    let foreign = open_v3_session(&mut server);
    let cross = call_v3_fail(
        &mut server,
        foreign,
        3,
        TESTS_REPORT_READ_TAG,
        report_read_body(&token, 0, total),
    );
    assert_eq!(cross.symbol, "NATIVE_TOKEN_INVALID");
    // An empty page request is malformed.
    let empty = call_v3_fail(
        &mut server,
        session,
        4,
        TESTS_REPORT_READ_TAG,
        report_read_body(&token, 0, 0),
    );
    assert_eq!(empty.code, ProtocolErrorCode::PayloadInvalid.numeric());
    // A page that cannot fit UInt32 is malformed.
    let wide = call_v3_fail(
        &mut server,
        session,
        5,
        TESTS_REPORT_READ_TAG,
        report_read_body(&token, 0, u64::from(u32::MAX) + 1),
    );
    assert_eq!(wide.code, ProtocolErrorCode::PayloadInvalid.numeric());
    // A short record is malformed before any token work.
    let short = call_v3_fail(
        &mut server,
        session,
        6,
        TESTS_REPORT_READ_TAG,
        encode_record(&[(1, token.clone()), (2, encode_uvar(0))]).unwrap(),
    );
    assert_eq!(short.code, ProtocolErrorCode::PayloadInvalid.numeric());
}

fn clock_zero() -> u64 {
    0
}

fn clock_far_future() -> u64 {
    u64::MAX
}

#[test]
fn report_read_refuses_after_token_expiry() {
    let (temp, mut server, session) = diagnostic_server("v3-report-expiry");
    let root = diagnostic_head_root(&temp);
    let (_report_id, token, total) = selected_report(&mut server, session, root, [0xD3; 16]);
    // Past the five-minute TTL the token refuses like an unknown one.
    server.set_clock_millis(clock_far_future);
    let expired = call_v3_fail(
        &mut server,
        session,
        2,
        TESTS_REPORT_READ_TAG,
        report_read_body(&token, 0, total),
    );
    assert_eq!(expired.symbol, "NATIVE_TOKEN_INVALID");
    // With the clock back the same token serves again: expiry refused,
    // it never invalidated.
    server.set_clock_millis(clock_zero);
    let revived = call_v3_ok(
        &mut server,
        session,
        3,
        TESTS_REPORT_READ_TAG,
        report_read_body(&token, 0, total),
    );
    assert_eq!(fields_of(&revived.body, 6)[3], encode_uvar(total));
}

// ---------------------------------------------------------------------------
// 606/607/commit route (NATIVE_TEST_ADMISSION_V1 App. C rev5)
// ---------------------------------------------------------------------------

/// Test-only acceptance signer: structural 64-byte claim signature,
/// mirroring the txn fixture signer (real Ed25519 stays vendored out).
struct CommitTestSigner {
    key: [u8; 32],
}

impl sley_txn::NativeAcceptanceSigner for CommitTestSigner {
    fn key_id(&self) -> [u8; 32] {
        self.key
    }

    fn sign(&self, _preimage: &[u8]) -> [u8; 64] {
        [0x5A; 64]
    }
}

/// Test-only commit-path executor: empty plans execute zero workers, so
/// commit and replay both return no evidence. Replay parity is exact
/// because both sides return the same empty vector.
struct EmptyNativeExecutor;

impl sley_txn::NativeTestExecutor for EmptyNativeExecutor {
    fn execute(
        &self,
        _plan: &sley_tests::NativeTestPlanV1,
        _validated: &sley_policy::ValidatedCandidatePlan,
    ) -> Result<Vec<sley_txn::ExecutedNativeTest>, sley_txn::NativeCommitError> {
        Ok(Vec::new())
    }

    fn execute_replay(
        &self,
        _plan: &sley_tests::NativeTestPlanV1,
        _objects: &std::collections::BTreeMap<sley_id::ObjectId, &[u8]>,
    ) -> Result<Vec<sley_txn::ExecutedNativeTest>, sley_txn::NativeCommitError> {
        Ok(Vec::new())
    }
}

/// Test-only commit-path executor refusing replay: commits execute zero
/// workers while replay refuses, proving the inconclusive verdict.
struct NoReplayExecutor;

impl sley_txn::NativeTestExecutor for NoReplayExecutor {
    fn execute(
        &self,
        _plan: &sley_tests::NativeTestPlanV1,
        _validated: &sley_policy::ValidatedCandidatePlan,
    ) -> Result<Vec<sley_txn::ExecutedNativeTest>, sley_txn::NativeCommitError> {
        Ok(Vec::new())
    }
}

const COMMIT_MEASUREMENT_KEY: [u8; 32] = [0xB2; 32];
const COMMIT_ACCEPTANCE_KEY: [u8; 32] = [0xA1; 32];

/// One receiver trust manifest granting one key one role over one
/// workspace/profile pair, mirroring the txn fixture trust.
fn commit_manifest(
    key: [u8; 32],
    role: u32,
    workspace: sley_id::WorkspaceId,
    profile: [u8; 32],
) -> sley_tests::HistoricalTrustPolicyV1 {
    use sley_tests::{HistoricalTrustPolicyParts, HistoricalTrustPolicyV1, TrustEntry};
    HistoricalTrustPolicyV1::build(HistoricalTrustPolicyParts {
        policy_nonce: [0x11; 32],
        entries: vec![TrustEntry {
            key_id: key,
            role,
            workspaces: vec![*workspace.as_bytes()],
            profiles: vec![profile],
            valid_from_unix_millis: 0,
            valid_until_unix_millis: u64::MAX,
        }],
    })
    .expect("commit trust builds")
}

/// Provisions one commit authority over the genesis workspace: trivial
/// commit-path executor, structural signer, and matching trust manifests.
fn commit_authority(workspace: sley_id::WorkspaceId) -> crate::server::NativeAuthority {
    let admission = sley_policy::fixed_native_admission_profile()
        .expect("fixed descriptor builds")
        .id();
    crate::server::NativeAuthority::provision(
        Box::new(EmptyNativeExecutor),
        Box::new(CommitTestSigner {
            key: COMMIT_ACCEPTANCE_KEY,
        }),
        commit_manifest(
            COMMIT_MEASUREMENT_KEY,
            sley_tests::ROLE_MEASUREMENT,
            workspace,
            *sley_tests::native_execution_profile_id().as_bytes(),
        ),
        commit_manifest(
            COMMIT_ACCEPTANCE_KEY,
            sley_tests::ROLE_ACCEPTANCE,
            workspace,
            *admission.as_bytes(),
        ),
    )
}

/// Test-only diagnostic executor serving empty plans: returns no
/// evidence without touching commit machinery (commit entry points keep
/// refusing honestly), proving diagnostics derive over native heads
/// through the shared state, object, and policy types.
struct EmptyDiagnosticExecutor;

impl sley_txn::NativeTestExecutor for EmptyDiagnosticExecutor {
    fn execute(
        &self,
        _plan: &sley_tests::NativeTestPlanV1,
        _validated: &sley_policy::ValidatedCandidatePlan,
    ) -> Result<Vec<sley_txn::ExecutedNativeTest>, sley_txn::NativeCommitError> {
        Err(sley_txn::NativeCommitError::ExecutorUnavailable)
    }

    fn execute_diagnostic(
        &self,
        _plan: &sley_tests::NativeTestPlanV1,
        _objects: &std::collections::BTreeMap<sley_id::ObjectId, &[u8]>,
    ) -> Result<Vec<sley_txn::ExecutedNativeTest>, sley_txn::NativeCommitError> {
        Ok(Vec::new())
    }
}

/// A v3 server provisioned for native commits with a fixed clock: the
/// genesis workspace is byte 1 like the diagnostic convention.
fn commit_server(label: &str) -> (sley_repo::test_support::TempDir, Server, SessionId) {
    use sley_id::WorkspaceId;
    let (temp, _transactions, _genesis_id) = genesis(label, executable_bodies(), &[]);
    let repository = temp.child("repo");
    let bit = FEATURE_CANCEL | FEATURE_STREAM | FEATURE_NATIVE_TESTS_V1;
    let methods = v3_offered_methods();
    let mut server = Server::new_versioned(
        &repository,
        &v3hello(methods.clone(), bit),
        &v3hello(methods, bit),
    )
    .unwrap();
    assert_eq!(server.profile().protocol_version, PROTOCOL_VERSION_V3);
    let session = open_v3_session(&mut server);
    server.set_clock_millis(clock_zero);
    server.set_native_authority(commit_authority(WorkspaceId::from_bytes([1; 32])));
    (temp, server, session)
}

/// An empty-selection namespace candidate over the given parent: no
/// `TestCases` created, so the native plan selects nothing and the commit
/// exercises the journal/status/replay path without worker evidence.
fn empty_candidate(
    temp: &sley_repo::test_support::TempDir,
    parent: sley_id::TransactionId,
) -> Vec<u8> {
    use sley_id::{CandidateNonce, PrincipalId, WorkspaceId};
    use sley_mutate::value::{EntityBodyValue, EntityIdSet, NamespaceBody};
    use sley_mutate::{
        BoundPrecondition, CandidateExpiry, CandidateRecord, ExpectedIdentityAbsent, MutationClass,
        MutationOperation, MutationPayload, PreconditionPayload, PreimageRequirement,
        build_candidate, full_validation_profile_id,
    };
    use sley_policy::build_capability_summary_projection;
    let head = sley_txn::TransactionRepository::new(temp.child("repo"))
        .accepted_head()
        .unwrap();
    let revision = head.verified_revision();
    let workspace = WorkspaceId::from_bytes([1; 32]);
    let principal = PrincipalId::from_bytes([2; 32]);
    let nonce = CandidateNonce::from_bytes([0x77; 32]);
    let target = sley_id::EntityId::derive(workspace, nonce, 3, 0);
    let summary = build_capability_summary_projection(
        principal,
        workspace,
        revision.policy_root().root(),
        revision.state_root().root,
        &[],
    )
    .unwrap();
    build_candidate(&CandidateRecord {
        format_version: 1,
        workspace_id: workspace,
        base_transaction_id: parent,
        base_root: revision.state_root().root,
        schema_epoch_id: revision.state_root().record.schema_epoch_id,
        policy_root_id: revision.policy_root().root(),
        principal_id: principal,
        capability_summary_digest: summary.digest(),
        operations: vec![MutationOperation {
            ordinal: 0,
            class: MutationClass::CreateEntity,
            target_kind: 3,
            target_entity: target,
            field_tag: None,
            payload: MutationPayload::CreateEntity(EntityBodyValue::Namespace(NamespaceBody {
                parent: None,
                members: EntityIdSet::from_unsorted(vec![]).unwrap(),
            })),
            precondition_ordinal: 0,
        }],
        preconditions: vec![BoundPrecondition {
            operation_ordinal: 0,
            requirement: PreimageRequirement::ExpectedIdentityAbsent,
            payload: PreconditionPayload::ExpectedIdentityAbsent(ExpectedIdentityAbsent {
                entity_id: target,
            }),
        }],
        validation_profile_id: full_validation_profile_id().unwrap(),
        candidate_nonce: nonce,
        expiry: CandidateExpiry::unix_millis(2_000),
    })
    .unwrap()
    .stored_bytes
}

/// One v3 native commit request body: candidate, parent, attempt, profile.
fn commit_native_body(
    candidate: &[u8],
    parent: sley_id::TransactionId,
    attempt: [u8; 16],
) -> Vec<u8> {
    let profile = sley_policy::fixed_native_admission_profile()
        .expect("fixed descriptor builds")
        .id();
    encode_record(&[
        (1, candidate.to_vec()),
        (2, parent.as_bytes().to_vec()),
        (3, attempt.to_vec()),
        (4, profile.as_bytes().to_vec()),
    ])
    .unwrap()
}

/// Drives one empty native commit through the v3 route and returns the
/// committed identities: journal coverage every later 606/607 test builds
/// on. The commit advances the head, so callers open a fresh session for
/// the reads that follow.
fn committed_native(
    temp: &sley_repo::test_support::TempDir,
    server: &mut Server,
    session: SessionId,
    attempt: [u8; 16],
) -> (
    sley_id::TransactionId,
    sley_id::ReceiptId,
    sley_id::StateRoot,
) {
    use sley_id::TransactionId;
    let parent = sley_txn::TransactionRepository::new(temp.child("repo"))
        .accepted_head()
        .unwrap()
        .verified_revision()
        .transaction_id();
    let candidate = empty_candidate(temp, parent);
    let frame = call_v3_ok(
        server,
        session,
        1,
        Method::Commit.tag(),
        commit_native_body(&candidate, parent, attempt),
    );
    let fields = fields_of(&frame.body, 6);
    assert_eq!(fields[5], attempt.to_vec());
    let transaction_id = TransactionId::from_bytes(fields[0].as_slice().try_into().unwrap());
    assert_ne!(transaction_id, parent);
    (
        transaction_id,
        sley_id::ReceiptId::from_bytes(fields[1].as_slice().try_into().unwrap()),
        sley_id::StateRoot::from_bytes(fields[2].as_slice().try_into().unwrap()),
    )
}

/// The stored plan's resource policy behind one committed transaction:
/// the claim a 606 replay must echo exactly.
fn stored_policy_id(
    temp: &sley_repo::test_support::TempDir,
    transaction_id: sley_id::TransactionId,
) -> [u8; 32] {
    let revision = sley_txn::TransactionRepository::new(temp.child("repo"))
        .verified_native_revision(transaction_id)
        .unwrap();
    let plan =
        sley_tests::NativeTestPlanV1::parse(revision.receipt().bundle.plan_stored()).unwrap();
    *plan.resource_policy().policy_id().as_bytes()
}

/// One 606 `tests.replay` request body: transaction, root, profile,
/// policy, attempt.
fn replay_body(
    transaction_id: sley_id::TransactionId,
    root: sley_id::StateRoot,
    policy: [u8; 32],
    attempt: [u8; 16],
) -> Vec<u8> {
    let profile = sley_tests::native_execution_profile_id();
    encode_record(&[
        (1, transaction_id.as_bytes().to_vec()),
        (2, root.as_bytes().to_vec()),
        (3, profile.as_bytes().to_vec()),
        (4, policy.to_vec()),
        (5, attempt.to_vec()),
    ])
    .unwrap()
}

/// One 607 `tests.attempt_status` request body: attempt plus an optional
/// candidate binding (empty bytes mean absent).
fn attempt_status_body(attempt: [u8; 16], candidate: Option<[u8; 32]>) -> Vec<u8> {
    encode_record(&[
        (1, attempt.to_vec()),
        (
            2,
            candidate.map_or_else(Vec::new, |identity| identity.to_vec()),
        ),
    ])
    .unwrap()
}

#[test]
fn commit_route_commits_empty_selection_and_journals_attempt() {
    let (temp, mut server, session) = commit_server("v3-commit-route");
    let attempt = [0xC0; 16];
    let parent = sley_txn::TransactionRepository::new(temp.child("repo"))
        .accepted_head()
        .unwrap()
        .verified_revision()
        .transaction_id();
    let (transaction_id, receipt_id, _root) =
        committed_native(&temp, &mut server, session, attempt);
    // The journal binds workspace, principal, candidate, and parent
    // behind the attempt the response echoed.
    let scope = sley_txn::TransactionRepository::new(temp.child("repo"))
        .native_attempt_scope(sley_txn::NativeAttemptId(attempt))
        .unwrap()
        .expect("commit journals its attempt");
    assert_eq!(scope.workspace, sley_id::WorkspaceId::from_bytes([1; 32]));
    assert_eq!(scope.principal, sley_id::PrincipalId::from_bytes([2; 32]));
    assert_eq!(scope.expected_parent, parent);
    assert_ne!(transaction_id, parent);
    let _ = receipt_id;
}

/// The stored test report bytes behind one committed transaction: the
/// exact bytes a 605 token must serve.
fn stored_report_bytes(
    temp: &sley_repo::test_support::TempDir,
    transaction_id: sley_id::TransactionId,
) -> Vec<u8> {
    sley_txn::TransactionRepository::new(temp.child("repo"))
        .verified_native_revision(transaction_id)
        .unwrap()
        .receipt()
        .bundle
        .test_report_stored()
        .to_vec()
}

#[test]
fn commit_route_without_authority_refuses_before_journal() {
    let (temp, mut server, session) = diagnostic_server("v3-commit-noauth");
    let parent = sley_txn::TransactionRepository::new(temp.child("repo"))
        .accepted_head()
        .unwrap()
        .verified_revision()
        .transaction_id();
    let candidate = empty_candidate(&temp, parent);
    let attempt = [0xC1; 16];
    let failure = call_v3_fail(
        &mut server,
        session,
        1,
        Method::Commit.tag(),
        commit_native_body(&candidate, parent, attempt),
    );
    assert_eq!(failure.symbol, "NATIVE_SIGNER_UNAVAILABLE");
    // Nothing journaled, head unchanged.
    assert!(
        sley_txn::TransactionRepository::new(temp.child("repo"))
            .native_attempt_scope(sley_txn::NativeAttemptId(attempt))
            .unwrap()
            .is_none()
    );
}

#[test]
fn commit_route_legacy_shape_refuses_malformed_under_v3() {
    // A legacy v1 commit body under v3+native never reaches the legacy
    // route: the native payload owns the method there. The first field
    // (a parent identity where candidate bytes belong) fails candidate
    // import before any work, so the head stays put and nothing journals.
    // The exact codec symbol depends on the parent bytes, so this pins
    // the refusal and its side-effect freedom, not the symbol.
    let (temp, mut server, session) = commit_server("v3-commit-legacy");
    let parent = sley_txn::TransactionRepository::new(temp.child("repo"))
        .accepted_head()
        .unwrap()
        .verified_revision()
        .transaction_id();
    let candidate = empty_candidate(&temp, parent);
    let legacy = encode_record(&[
        (1, parent.as_bytes().to_vec()),
        (
            2,
            sley_id::PrincipalId::from_bytes([2; 32])
                .as_bytes()
                .to_vec(),
        ),
        (3, encode_uvar(1_000)),
        (4, candidate),
    ])
    .unwrap();
    let _failure = call_v3_fail(&mut server, session, 1, Method::Commit.tag(), legacy);
    let head = sley_txn::TransactionRepository::new(temp.child("repo"))
        .accepted_head()
        .unwrap()
        .verified_revision()
        .transaction_id();
    assert_eq!(head, parent);
}

#[test]
fn commit_route_native_shape_stays_legacy_on_v1() {
    // Routing is by negotiation, never sniffing: the native shape under
    // v1 reaches the legacy commit, where the candidate bytes fail the
    // parent field and refuse as malformed.
    let mut harness = Harness::new("smp1-commit-v1-shape");
    let parent = sley_id::TransactionId::from_bytes([9; 32]);
    let failure = harness.fail(
        Method::Commit,
        commit_native_body(&[7; 64], parent, [0xC2; 16]),
    );
    assert_eq!(failure.code, ProtocolErrorCode::PayloadInvalid.numeric());
}

#[test]
fn attempt_status_answers_unknown_for_missing_and_diagnostic_attempts() {
    let (temp, mut server, session) = diagnostic_server("v3-status-unknown");
    let root = diagnostic_head_root(&temp);
    // A completed diagnostic attempt is never journaled, so 607 answers
    // unknown for it (selected_report answers request 1 underneath).
    let (_report_id, _token, _total) = selected_report(&mut server, session, root, [0xD1; 16]);
    let frame = call_v3_ok(
        &mut server,
        session,
        2,
        TESTS_ATTEMPT_STATUS_TAG,
        attempt_status_body([0xD1; 16], None),
    );
    let fields = fields_of(&frame.body, 4);
    assert_eq!(fields[0], encode_uvar(0));
    assert!(fields[1].is_empty() && fields[2].is_empty() && fields[3].is_empty());
    // No journal record either: unknown with every optional field absent.
    let frame = call_v3_ok(
        &mut server,
        session,
        3,
        TESTS_ATTEMPT_STATUS_TAG,
        attempt_status_body([0xD0; 16], None),
    );
    let fields = fields_of(&frame.body, 4);
    assert_eq!(fields[0], encode_uvar(0));
    assert!(fields[1].is_empty() && fields[2].is_empty() && fields[3].is_empty());
}
#[test]
fn attempt_status_answers_committed_with_token_after_renewal() {
    let (temp, mut server, session) = commit_server("v3-status-committed");
    let attempt = [0xC3; 16];
    let (transaction_id, receipt_id, _root) =
        committed_native(&temp, &mut server, session, attempt);
    // The commit moved the head past the session: renew into the new
    // head, proving status binds workspace rather than session identity.
    let renewed = call_v3_ok(
        &mut server,
        session,
        2,
        Method::SessionRenew.tag(),
        session.as_bytes().to_vec(),
    );
    assert_eq!(renewed.method, Method::SessionRenew.tag());
    let frame = call_v3_ok(
        &mut server,
        session,
        3,
        TESTS_ATTEMPT_STATUS_TAG,
        attempt_status_body(attempt, None),
    );
    let fields = fields_of(&frame.body, 4);
    assert_eq!(fields[0], encode_uvar(5));
    assert_eq!(fields[1], transaction_id.as_bytes().to_vec());
    assert_eq!(fields[2], receipt_id.as_bytes().to_vec());
    assert_eq!(fields[3].len(), 32);
    // The fresh token pages the verified accepted report to its end.
    let page = call_v3_ok(
        &mut server,
        session,
        4,
        TESTS_REPORT_READ_TAG,
        report_read_body(&fields[3], 0, u64::from(u32::MAX)),
    );
    let page_fields = fields_of(&page.body, 6);
    let stored = stored_report_bytes(&temp, transaction_id);
    assert_eq!(uvar_of(&page_fields[3]), stored.len() as u64);
    assert_eq!(
        page_fields[4],
        sley_scb1::encode_bytes(&stored).expect("page encodes")
    );
    assert_eq!(
        page_fields[5],
        sley_scb1::encode_option_uvar(None).expect("no link encodes"),
        "final page links nowhere"
    );
}

#[test]
fn attempt_status_conflicts_on_divergent_candidate() {
    let (temp, mut server, session) = commit_server("v3-status-conflict");
    let attempt = [0xC4; 16];
    let _ = committed_native(&temp, &mut server, session, attempt);
    let fresh = open_v3_session(&mut server);
    let failure = call_v3_fail(
        &mut server,
        fresh,
        1,
        TESTS_ATTEMPT_STATUS_TAG,
        attempt_status_body(attempt, Some([0xEE; 32])),
    );
    assert_eq!(failure.symbol, "NATIVE_ATTEMPT_CONFLICT");
}

#[test]
fn replay_scope_refuses_unknown_transaction() {
    let (_temp, mut server, session) = commit_server("v3-replay-scope");
    let policy = [0xDD; 32];
    let root = sley_id::StateRoot::from_bytes([0xDB; 32]);
    let failure = call_v3_fail(
        &mut server,
        session,
        1,
        TESTS_REPLAY_TAG,
        replay_body(
            sley_id::TransactionId::from_bytes([0xDA; 32]),
            root,
            policy,
            [0xE0; 16],
        ),
    );
    assert_eq!(failure.symbol, "NATIVE_REPLAY_SCOPE_REFUSED");
}

#[test]
fn replay_matches_empty_commit_and_pages_original_report() {
    let (temp, mut server, session) = commit_server("v3-replay-matched");
    let attempt = [0xC5; 16];
    let (transaction_id, _receipt_id, root) =
        committed_native(&temp, &mut server, session, attempt);
    let policy = stored_policy_id(&temp, transaction_id);
    let fresh = open_v3_session(&mut server);
    let frame = call_v3_ok(
        &mut server,
        fresh,
        1,
        TESTS_REPLAY_TAG,
        replay_body(transaction_id, root, policy, [0xE1; 16]),
    );
    let fields = fields_of(&frame.body, 6);
    assert_eq!(fields[0], transaction_id.as_bytes().to_vec());
    assert_eq!(fields[1], root.as_bytes().to_vec());
    assert_eq!(fields[2], encode_uvar(1));
    assert!(!fields[3].is_empty());
    assert!(fields[4].is_empty());
    assert_eq!(fields[5].len(), 32);
    // The match token pages the verified original report to its end.
    let page = call_v3_ok(
        &mut server,
        fresh,
        2,
        TESTS_REPORT_READ_TAG,
        report_read_body(&fields[5], 0, u64::from(u32::MAX)),
    );
    let page_fields = fields_of(&page.body, 6);
    assert_eq!(page_fields[0], fields[3]);
    let stored = stored_report_bytes(&temp, transaction_id);
    assert_eq!(uvar_of(&page_fields[3]), stored.len() as u64);
    assert_eq!(
        page_fields[4],
        sley_scb1::encode_bytes(&stored).expect("page encodes")
    );
    assert_eq!(
        page_fields[5],
        sley_scb1::encode_option_uvar(None).expect("no link encodes"),
        "final page links nowhere"
    );
    // Identical bindings replay the cached answer byte-for-byte.
    let replayed = call_v3_ok(
        &mut server,
        fresh,
        3,
        TESTS_REPLAY_TAG,
        replay_body(transaction_id, root, policy, [0xE1; 16]),
    );
    assert_eq!(replayed.body, frame.body);
    // Divergent bindings under the same attempt refuse as a conflict.
    let conflict = call_v3_fail(
        &mut server,
        fresh,
        4,
        TESTS_REPLAY_TAG,
        replay_body(
            transaction_id,
            sley_id::StateRoot::from_bytes([0xDC; 32]),
            policy,
            [0xE1; 16],
        ),
    );
    assert_eq!(conflict.symbol, "NATIVE_ATTEMPT_CONFLICT");
}

#[test]
fn replay_root_mismatch_answers_untrusted() {
    let (temp, mut server, session) = commit_server("v3-replay-root");
    let attempt = [0xC6; 16];
    let (transaction_id, _receipt_id, _root) =
        committed_native(&temp, &mut server, session, attempt);
    let policy = stored_policy_id(&temp, transaction_id);
    let fresh = open_v3_session(&mut server);
    let frame = call_v3_ok(
        &mut server,
        fresh,
        1,
        TESTS_REPLAY_TAG,
        replay_body(
            transaction_id,
            sley_id::StateRoot::from_bytes([0xDC; 32]),
            policy,
            [0xE2; 16],
        ),
    );
    let fields = fields_of(&frame.body, 6);
    assert_eq!(fields[2], encode_uvar(4));
    assert!(fields[4].is_empty() && fields[5].is_empty());
}

#[test]
fn replay_wrong_policy_claim_refuses_malformed() {
    let (temp, mut server, session) = commit_server("v3-replay-policy");
    let attempt = [0xC7; 16];
    let (transaction_id, _receipt_id, root) =
        committed_native(&temp, &mut server, session, attempt);
    let fresh = open_v3_session(&mut server);
    let failure = call_v3_fail(
        &mut server,
        fresh,
        1,
        TESTS_REPLAY_TAG,
        replay_body(transaction_id, root, [0xDD; 32], [0xE3; 16]),
    );
    assert_eq!(failure.code, ProtocolErrorCode::PayloadInvalid.numeric());
}

#[test]
fn replay_untrusted_without_matching_manifests() {
    let (temp, mut server, session) = commit_server("v3-replay-untrusted");
    let attempt = [0xC8; 16];
    let (transaction_id, _receipt_id, root) =
        committed_native(&temp, &mut server, session, attempt);
    let policy = stored_policy_id(&temp, transaction_id);
    // Re-provision with unrelated manifests: the receipt's trust
    // references resolve against nothing this server holds.
    let workspace = sley_id::WorkspaceId::from_bytes([1; 32]);
    let admission = sley_policy::fixed_native_admission_profile()
        .expect("fixed descriptor builds")
        .id();
    server.set_native_authority(crate::server::NativeAuthority::provision(
        Box::new(EmptyNativeExecutor),
        Box::new(CommitTestSigner {
            key: COMMIT_ACCEPTANCE_KEY,
        }),
        commit_manifest(
            [0xF1; 32],
            sley_tests::ROLE_MEASUREMENT,
            workspace,
            *sley_tests::native_execution_profile_id().as_bytes(),
        ),
        commit_manifest(
            [0xF2; 32],
            sley_tests::ROLE_ACCEPTANCE,
            workspace,
            *admission.as_bytes(),
        ),
    ));
    let fresh = open_v3_session(&mut server);
    let frame = call_v3_ok(
        &mut server,
        fresh,
        1,
        TESTS_REPLAY_TAG,
        replay_body(transaction_id, root, policy, [0xE4; 16]),
    );
    let fields = fields_of(&frame.body, 6);
    assert_eq!(fields[2], encode_uvar(4));
    assert!(fields[4].is_empty() && fields[5].is_empty());
}

#[test]
fn replay_without_authority_answers_untrusted() {
    // A server with no provisioned authority runs the engine
    // executor-less and trust-less: the verifiable history answers
    // untrusted rather than executing.
    let (temp, mut server, session) = commit_server("v3-replay-noauth");
    let attempt = [0xCC; 16];
    let (transaction_id, _receipt_id, root) =
        committed_native(&temp, &mut server, session, attempt);
    let policy = stored_policy_id(&temp, transaction_id);
    let repository = temp.child("repo");
    let bit = FEATURE_CANCEL | FEATURE_STREAM | FEATURE_NATIVE_TESTS_V1;
    let methods = v3_offered_methods();
    let mut bare = Server::new_versioned(
        &repository,
        &v3hello(methods.clone(), bit),
        &v3hello(methods, bit),
    )
    .unwrap();
    let fresh = open_v3_session(&mut bare);
    let frame = call_v3_ok(
        &mut bare,
        fresh,
        1,
        TESTS_REPLAY_TAG,
        replay_body(transaction_id, root, policy, [0xE7; 16]),
    );
    let fields = fields_of(&frame.body, 6);
    assert_eq!(fields[2], encode_uvar(4));
    assert!(fields[4].is_empty() && fields[5].is_empty());
}

#[test]
fn replay_refusing_executor_answers_inconclusive() {
    let (temp, mut server, session) = commit_server("v3-replay-inconclusive");
    let attempt = [0xC9; 16];
    let (transaction_id, _receipt_id, root) =
        committed_native(&temp, &mut server, session, attempt);
    let policy = stored_policy_id(&temp, transaction_id);
    // The commit-time executor replays nothing new here: swap in a
    // dispatch whose replay refuses while history still verifies.
    let workspace = sley_id::WorkspaceId::from_bytes([1; 32]);
    let admission = sley_policy::fixed_native_admission_profile()
        .expect("fixed descriptor builds")
        .id();
    server.set_native_authority(crate::server::NativeAuthority::provision(
        Box::new(NoReplayExecutor),
        Box::new(CommitTestSigner {
            key: COMMIT_ACCEPTANCE_KEY,
        }),
        commit_manifest(
            COMMIT_MEASUREMENT_KEY,
            sley_tests::ROLE_MEASUREMENT,
            workspace,
            *sley_tests::native_execution_profile_id().as_bytes(),
        ),
        commit_manifest(
            COMMIT_ACCEPTANCE_KEY,
            sley_tests::ROLE_ACCEPTANCE,
            workspace,
            *admission.as_bytes(),
        ),
    ));
    let fresh = open_v3_session(&mut server);
    let frame = call_v3_ok(
        &mut server,
        fresh,
        1,
        TESTS_REPLAY_TAG,
        replay_body(transaction_id, root, policy, [0xE5; 16]),
    );
    let fields = fields_of(&frame.body, 6);
    assert_eq!(fields[2], encode_uvar(3));
    assert!(fields[4].is_empty() && fields[5].is_empty());
}

#[test]
fn replay_refuses_non_native_history() {
    // The v1 genesis sits in scope but carries no native receipt: the
    // engine error refuses with its preserved symbol, never a verdict.
    let (temp, mut server, _session) = commit_server("v3-replay-nonnative");
    let genesis_id = sley_txn::TransactionRepository::new(temp.child("repo"))
        .accepted_head()
        .unwrap()
        .verified_revision()
        .transaction_id();
    let fresh = open_v3_session(&mut server);
    let failure = call_v3_fail(
        &mut server,
        fresh,
        1,
        TESTS_REPLAY_TAG,
        replay_body(
            genesis_id,
            diagnostic_head_root(&temp),
            [0xDD; 32],
            [0xE6; 16],
        ),
    );
    assert_eq!(failure.symbol, "EXCHANGE_RECEIPT_INVALID");
}

#[test]
fn selected_runs_empty_plan_over_native_head() {
    // After a native commit the head is format-2: the selection read
    // derives the empty plan over it through the shared types, proving
    // the native arm of every head accessor serves diagnostics.
    let (temp, mut server, session) = commit_server("v3-selected-native");
    let attempt = [0xCA; 16];
    let (_transaction_id, _receipt_id, root) =
        committed_native(&temp, &mut server, session, attempt);
    server.set_executor(Box::new(EmptyDiagnosticExecutor));
    let fresh = open_v3_session(&mut server);
    let frame = call_v3_ok(
        &mut server,
        fresh,
        1,
        TESTS_SELECTED_TAG,
        selected_body(root, &[], [0xCB; 16]),
    );
    let fields = fields_of(&frame.body, 6);
    assert_eq!(fields[3], encode_uvar(0));
    assert!(!fields[4].is_empty());
}

#[test]
fn replay_and_attempt_state_numbers_cover_every_verdict() {
    use crate::server::{attempt_state_number, replay_status_number};
    use sley_repo::NativeReplayStatus;
    assert_eq!(replay_status_number(NativeReplayStatus::Matched), 1);
    assert_eq!(replay_status_number(NativeReplayStatus::Mismatch), 2);
    assert_eq!(
        replay_status_number(NativeReplayStatus::InconclusiveResource),
        3
    );
    assert_eq!(
        replay_status_number(NativeReplayStatus::UntrustedHistory),
        4
    );
    assert_eq!(attempt_state_number(&sley_txn::AttemptStatus::Unknown), 0);
    assert_eq!(attempt_state_number(&sley_txn::AttemptStatus::Admitted), 1);
    assert_eq!(attempt_state_number(&sley_txn::AttemptStatus::Running), 2);
    assert_eq!(
        attempt_state_number(&sley_txn::AttemptStatus::AbortedBeforePromotion),
        3
    );
    assert_eq!(
        attempt_state_number(&sley_txn::AttemptStatus::PromotionStarted),
        4
    );
    assert_eq!(
        attempt_state_number(&sley_txn::AttemptStatus::Committed {
            transaction_id: sley_id::TransactionId::from_bytes([1; 32]),
            receipt_id: sley_id::ReceiptId::from_bytes([2; 32]),
            at_head: true,
        }),
        5
    );
    assert_eq!(
        attempt_state_number(&sley_txn::AttemptStatus::OutcomeUnknown { head: None }),
        6
    );
}

#[test]
fn versioned_session_open_binds_and_claims_split() {
    let mut harness = VServer::new("v2-open");
    let session = harness.session;
    assert!(harness.server.remaining_budget(session).is_some());
    let v1_open = encode_frame_for_version(
        &ProtocolFrame {
            protocol_version: PROTOCOL_VERSION,
            session: None,
            request_id: 0,
            kind: FrameKind::Request,
            method: Method::SessionOpen.tag(),
            flags: 0,
            bounds: BoundedContext::none(),
            body: harness.server.handshake_id().as_bytes().to_vec(),
        },
        PROTOCOL_VERSION,
    )
    .unwrap()
    .bytes;
    let answer = harness.server.answer(&v1_open).unwrap();
    assert!(answer.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame_for_version(&answer.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2)
            .unwrap()
    else {
        panic!("response frame");
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::Downgrade.numeric()
    );
    // An undefined selection emits nothing, so the v4-claiming wire bytes
    // are minted past validation on purpose: the server must still refuse
    // them `PROTOCOL_VERSION_UNSUPPORTED` when they arrive on the wire.
    let v4_claim = ProtocolFrame {
        protocol_version: 4,
        session: None,
        request_id: 0,
        kind: FrameKind::Request,
        method: Method::SessionOpen.tag(),
        flags: 0,
        bounds: BoundedContext::none(),
        body: harness.server.handshake_id().as_bytes().to_vec(),
    };
    assert_eq!(
        encode_frame_for_version(&v4_claim, 4).unwrap_err().code(),
        ProtocolErrorCode::VersionUnsupported
    );
    let v4_open = crate::encode_envelope(v4_claim.kind, &v4_claim.payload().unwrap())
        .unwrap()
        .bytes;
    let answer = harness.server.answer(&v4_open).unwrap();
    assert!(answer.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame_for_version(&answer.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2)
            .unwrap()
    else {
        panic!("response frame");
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::VersionUnsupported.numeric()
    );
}

#[test]
fn entity_version_returns_exact_bytes_and_bounded_context() {
    let mut harness = VServer::new("v2-version");
    let transactions = sley_txn::TransactionRepository::new(&harness.repository).accepted_head();
    let revision = transactions.unwrap().verified_revision().clone();
    let target = revision
        .objects()
        .iter()
        .find(|object| object.record().entity_id == sley_repo::test_support::id(31))
        .unwrap()
        .clone();
    let frame = harness.read(ENTITY_VERSION_TAG, target.record().entity_id);
    assert_eq!(frame.bounds.applied_limits, harness.server.profile().limits);
    assert_eq!(frame.bounds.returned_bytes, frame.body.len() as u64);
    assert_eq!(frame.bounds.returned_entities, 1);
    assert_eq!(frame.bounds.returned_edges, 0);
    assert_eq!(frame.bounds.reached_depth, 0);
    assert_eq!(frame.bounds.omitted, 0);
    assert!(!frame.bounds.truncated);
    assert!(!frame.bounds.continuation);
    let response = decode_entity_read_response(&frame.body).unwrap();
    assert_eq!(response.objects.len(), 1);
    assert_eq!(response.objects[0].entity, target.record().entity_id);
    assert_eq!(response.objects[0].object_id, target.object_id());
    assert_eq!(
        response.objects[0].kind,
        u64::from(target.record().body.kind_tag())
    );
    assert_eq!(response.objects[0].stored_bytes, target.stored_bytes());
    assert_eq!(response.root, harness.root);
    assert_eq!(response.session, harness.session);
    assert_eq!(response.requested_entity, target.record().entity_id);
    assert_eq!(
        response.workspace,
        revision.state_root().record.workspace_id
    );
    assert_eq!(response.epoch, revision.state_root().record.schema_epoch_id);
    let direct = encode_single_frame_direct(&frame, MAX_FRAME_BYTES).unwrap();
    let (DecodedFrame::Response(reframed), _) =
        decode_frame_for_version(&direct.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2).unwrap()
    else {
        panic!("response frame");
    };
    assert_eq!(reframed.body, frame.body);
}

#[test]
fn entity_signature_returns_declaration_order() {
    let mut harness = VServer::with_bodies("v2-signature", nonmonotonic_bodies(), &[]);
    let frame = harness.read(ENTITY_SIGNATURE_TAG, EntityId::from_bytes([40; 32]));
    let response = decode_entity_read_response(&frame.body).unwrap();
    let order: Vec<EntityId> = response
        .objects
        .iter()
        .map(|object| object.entity)
        .collect();
    assert_eq!(
        order,
        vec![
            EntityId::from_bytes([40; 32]),
            EntityId::from_bytes([43; 32]),
            EntityId::from_bytes([41; 32]),
            EntityId::from_bytes([42; 32]),
        ]
    );
    let kinds: Vec<u64> = response.objects.iter().map(|object| object.kind).collect();
    assert_eq!(kinds, vec![5, 6, 6, 6]);
    assert_eq!(frame.bounds.returned_entities, 4);
}

#[test]
fn entity_read_failure_precedence_is_exact() {
    let mut harness = VServer::new("v2-precedence");
    let junk = harness.refuse(ENTITY_VERSION_TAG, b"junk".to_vec());
    assert_eq!(junk.code, ProtocolErrorCode::PayloadInvalid.numeric());
    let zeros = harness.refuse(
        ENTITY_VERSION_TAG,
        encode_record(&[
            (1, harness.root.as_bytes().to_vec()),
            (2, sley_repo::test_support::id(30).as_bytes().to_vec()),
            (3, encode_uvar(0)),
            (4, encode_uvar(8_388_608)),
            (5, encode_uvar(100_000_000)),
        ])
        .unwrap(),
    );
    assert_eq!(zeros.code, ProtocolErrorCode::PayloadInvalid.numeric());
    let over = harness.refuse(
        ENTITY_VERSION_TAG,
        encode_record(&[
            (1, harness.root.as_bytes().to_vec()),
            (2, sley_repo::test_support::id(30).as_bytes().to_vec()),
            (3, encode_uvar(65_535)),
            (4, encode_uvar(8_388_608)),
            (5, encode_uvar(u64::MAX)),
        ])
        .unwrap(),
    );
    assert_eq!(over.code, ProtocolErrorCode::LimitExceeded.numeric());
    let wrong_root = harness.refuse(
        ENTITY_VERSION_TAG,
        entity_read_body(
            StateRoot::from_bytes([0x77; 32]),
            sley_repo::test_support::id(30),
        ),
    );
    assert_eq!(wrong_root.code, 31_008);
    assert_eq!(wrong_root.symbol, "QUERY_ROOT_MISMATCH");
    let unknown = harness.refuse(
        ENTITY_VERSION_TAG,
        entity_read_body(harness.root, EntityId::from_bytes([0x63; 32])),
    );
    assert_eq!(unknown.code, 31_004);
    let wrong_kind = harness.refuse(
        ENTITY_SIGNATURE_TAG,
        entity_read_body(harness.root, sley_repo::test_support::id(34)),
    );
    assert_eq!(wrong_kind.code, 31_010);
    let unknown_session = encode_frame_for_version(
        &ProtocolFrame {
            protocol_version: PROTOCOL_VERSION_V2,
            session: Some(SessionId::from_bytes([0x99; 32])),
            request_id: 91,
            kind: FrameKind::Request,
            method: ENTITY_VERSION_TAG,
            flags: 0,
            bounds: BoundedContext::none(),
            body: entity_read_body(harness.root, sley_repo::test_support::id(30)),
        },
        PROTOCOL_VERSION_V2,
    )
    .unwrap()
    .bytes;
    let answer = harness.server.answer(&unknown_session).unwrap();
    assert!(answer.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame_for_version(&answer.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2)
            .unwrap()
    else {
        panic!("response frame");
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().symbol,
        "SESSION_UNKNOWN"
    );
}

#[test]
fn version_one_selection_refuses_version_two_methods_at_tag_validity() {
    use crate::Retryability;
    // A version-aware server holding a version 1 selection refuses the two
    // version-2 tags and an opaque unknown tag at method-tag validity,
    // before session routing (contract SMP1 sections 2 and 9): the
    // envelope is `PROTOCOL_METHOD_UNSUPPORTED`, never retried, with empty
    // details and no dispatch charge, even for an unknown session.
    let methods = vec![100, 300, ENTITY_VERSION_TAG, ENTITY_SIGNATURE_TAG, 999];
    let mut one = vhello(methods.clone(), 4);
    one.protocol_versions = vec![PROTOCOL_VERSION];
    let mut server_one = vhello(methods, 8);
    server_one.protocol_versions = vec![PROTOCOL_VERSION];
    let (temp, _transactions, _) = genesis("v1-selection-gate", executable_bodies(), &[]);
    let repository = temp.child("repo");
    let mut server = Server::new_versioned(&repository, &one, &server_one).unwrap();
    assert_eq!(server.profile().protocol_version, PROTOCOL_VERSION);
    assert!(!server.profile().methods.contains(&ENTITY_VERSION_TAG));
    assert!(!server.profile().methods.contains(&ENTITY_SIGNATURE_TAG));
    // The opaque tag survives negotiation untouched (legacy treatment) but
    // still never dispatches on a v1 serving path.
    assert!(server.profile().methods.contains(&999));
    let open = encode_frame_for_version(
        &ProtocolFrame {
            protocol_version: PROTOCOL_VERSION,
            session: None,
            request_id: 0,
            kind: FrameKind::Request,
            method: Method::SessionOpen.tag(),
            flags: 0,
            bounds: BoundedContext::none(),
            body: server.handshake_id().as_bytes().to_vec(),
        },
        PROTOCOL_VERSION,
    )
    .unwrap()
    .bytes;
    let opened = server.answer(&open).unwrap();
    assert!(!opened.failed);
    let (DecodedFrame::Response(open_frame), _) =
        decode_frame_for_version(&opened.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION).unwrap()
    else {
        panic!("session open response");
    };
    let session = SessionId::from_bytes(open_frame.body.as_slice().try_into().unwrap());
    let send = |server: &mut Server, tag: u32, session: Option<SessionId>, request_id: u64| {
        let bytes = encode_frame_for_version(
            &ProtocolFrame {
                protocol_version: PROTOCOL_VERSION,
                session,
                request_id,
                kind: FrameKind::Request,
                method: tag,
                flags: 0,
                bounds: BoundedContext::none(),
                body: Vec::new(),
            },
            PROTOCOL_VERSION,
        )
        .unwrap()
        .bytes;
        let answer = server.answer(&bytes).unwrap();
        assert!(answer.failed, "{tag} unexpectedly succeeded");
        let (DecodedFrame::Response(frame), _) =
            decode_frame_for_version(&answer.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION)
                .unwrap()
        else {
            panic!("response frame");
        };
        assert_eq!(frame.protocol_version, PROTOCOL_VERSION);
        ProtocolFailure::decode(&frame.body).unwrap()
    };
    let mut next_request = 1;
    for tag in [ENTITY_VERSION_TAG, ENTITY_SIGNATURE_TAG, 999] {
        let before = server.remaining_budget(session);
        let failure = send(&mut server, tag, Some(session), next_request);
        next_request += 1;
        assert_eq!(failure.code, ProtocolErrorCode::MethodUnsupported.numeric());
        assert_eq!(
            failure.symbol,
            ProtocolErrorCode::MethodUnsupported.as_str()
        );
        assert_eq!(failure.retryability, Retryability::Never);
        assert!(failure.details.is_empty());
        assert_eq!(server.remaining_budget(session), before);
        // Tag validity precedes session routing: an unknown session meets
        // the same refusal, not `SESSION_UNKNOWN`.
        let unknown = send(
            &mut server,
            tag,
            Some(SessionId::from_bytes([0x99; 32])),
            next_request,
        );
        next_request += 1;
        assert_eq!(unknown.code, ProtocolErrorCode::MethodUnsupported.numeric());
        assert!(unknown.details.is_empty());
    }
}

#[test]
fn entity_read_session_lifecycle_binds_one_snapshot() {
    let mut harness = VServer::new("v2-session");
    let (closed_failed, _) = harness.call(Method::SessionClose.tag(), Vec::new());
    assert!(!closed_failed);
    let closed = harness.refuse(
        ENTITY_VERSION_TAG,
        entity_read_body(harness.root, sley_repo::test_support::id(30)),
    );
    assert_eq!(closed.code, ProtocolErrorCode::SessionClosed.numeric());
}

#[test]
fn entity_read_head_advance_fails_before_body_decode() {
    use sley_id::{CandidateNonce, PrincipalId};
    use sley_mutate::value::{EntityBodyValue, EntityIdSet, NamespaceBody};
    use sley_mutate::{
        BoundPrecondition, CandidateExpiry, CandidateRecord, ExpectedIdentityAbsent, MutationClass,
        MutationOperation, MutationPayload, PreconditionPayload, PreimageRequirement,
        build_candidate, full_validation_profile_id,
    };
    use sley_policy::build_capability_summary_projection;
    let mut harness = VServer::new("v2-advance");
    let transactions = sley_txn::TransactionRepository::new(&harness.repository);
    let head = transactions.accepted_head().unwrap();
    let workspace = head.state_root().record.workspace_id;
    let principal = PrincipalId::from_bytes([2; 32]);
    let nonce = CandidateNonce::from_bytes([0x77; 32]);
    let target = EntityId::derive(workspace, nonce, 3, 0);
    let summary = build_capability_summary_projection(
        principal,
        workspace,
        head.policy_root().root(),
        head.state_root().root,
        &[],
    )
    .unwrap();
    let candidate = build_candidate(&CandidateRecord {
        format_version: 1,
        workspace_id: workspace,
        base_transaction_id: head.transaction_id(),
        base_root: head.state_root().root,
        schema_epoch_id: head.state_root().record.schema_epoch_id,
        policy_root_id: head.policy_root().root(),
        principal_id: principal,
        capability_summary_digest: summary.digest(),
        operations: vec![MutationOperation {
            ordinal: 0,
            class: MutationClass::CreateEntity,
            target_kind: 3,
            target_entity: target,
            field_tag: None,
            payload: MutationPayload::CreateEntity(EntityBodyValue::Namespace(NamespaceBody {
                parent: None,
                members: EntityIdSet::from_unsorted(vec![]).unwrap(),
            })),
            precondition_ordinal: 0,
        }],
        preconditions: vec![BoundPrecondition {
            operation_ordinal: 0,
            requirement: PreimageRequirement::ExpectedIdentityAbsent,
            payload: PreconditionPayload::ExpectedIdentityAbsent(ExpectedIdentityAbsent {
                entity_id: target,
            }),
        }],
        validation_profile_id: full_validation_profile_id().unwrap(),
        candidate_nonce: nonce,
        expiry: CandidateExpiry::unix_millis(2_000),
    })
    .unwrap();
    let (commit_failed, _) = harness.call(
        Method::Commit.tag(),
        encode_record(&[
            (1, head.transaction_id().as_bytes().to_vec()),
            (2, principal.as_bytes().to_vec()),
            (3, encode_uvar(1_000)),
            (4, candidate.stored_bytes.clone()),
        ])
        .unwrap(),
    );
    assert!(!commit_failed);
    let new_root = sley_txn::TransactionRepository::new(&harness.repository)
        .accepted_head()
        .unwrap()
        .verified_revision()
        .state_root()
        .root;
    assert_ne!(new_root, harness.root);
    let stale = harness.refuse(
        ENTITY_VERSION_TAG,
        entity_read_body(harness.root, sley_repo::test_support::id(30)),
    );
    assert_eq!(stale.symbol, "SESSION_ROOT_ADVANCED");
    let malformed_stale = harness.refuse(ENTITY_VERSION_TAG, b"junk".to_vec());
    assert_eq!(malformed_stale.symbol, "SESSION_ROOT_ADVANCED");
    let (renew_failed, _) = harness.call(
        Method::SessionRenew.tag(),
        harness.session.as_bytes().to_vec(),
    );
    assert!(!renew_failed);
    harness.root = new_root;
    let frame = harness.read(ENTITY_VERSION_TAG, target);
    let response = decode_entity_read_response(&frame.body).unwrap();
    assert_eq!(response.root, new_root);
    assert_eq!(response.requested_entity, target);
}

/// A reused request identifier on an entity method refuses with
/// `PROTOCOL_REQUEST_ID_CONFLICT` (discharges `seq_request_id_conflict`).
#[test]
fn entity_request_id_reuse_refuses_second_use() {
    let mut harness = VServer::new("v2-request-id");
    let target = sley_repo::test_support::id(30);
    let first = harness.next_request;
    let frame = harness.read(ENTITY_VERSION_TAG, target);
    let _ = frame;
    let replay = encode_frame_for_version(
        &ProtocolFrame {
            protocol_version: PROTOCOL_VERSION_V2,
            session: Some(harness.session),
            request_id: first,
            kind: FrameKind::Request,
            method: ENTITY_VERSION_TAG,
            flags: 0,
            bounds: BoundedContext::none(),
            body: entity_read_body(harness.root, target),
        },
        PROTOCOL_VERSION_V2,
    )
    .unwrap()
    .bytes;
    let answer = harness.server.answer(&replay).unwrap();
    assert!(answer.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame_for_version(&answer.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2)
            .unwrap()
    else {
        panic!("response frame");
    };
    let failure = ProtocolFailure::decode(&frame.body).unwrap();
    assert_eq!(failure.symbol, "PROTOCOL_REQUEST_ID_CONFLICT");
    assert_eq!(failure.code, ProtocolErrorCode::RequestIdConflict.numeric());
}

/// An exhausted budget combined with a stale root answers
/// `SESSION_ROOT_ADVANCED`: session binding precedes budget exhaustion,
/// and the binding failure keeps no-debit semantics (discharges
/// `seq_work_exhausted_precedence`).
#[test]
fn entity_exhausted_budget_with_stale_root_answers_binding_first() {
    // Probe the M-free work part and body length on a funded server.
    let mut probe = VServer::new("v2-exhaust-probe");
    let target = sley_repo::test_support::id(30);
    let (failed, frame) = probe.call(ENTITY_VERSION_TAG, entity_read_body(probe.root, target));
    assert!(!failed);
    let probed = decode_entity_read_response(&frame.body).unwrap();
    let constant = probed.work_units - 8_388_608;
    let body_len = frame.bounds.returned_bytes;
    let before_commit = probe.budget();
    let _ = advance_head_with_namespace(&mut probe);
    let commit_cost = before_commit - probe.budget();
    // A server whose whole budget equals one exact read plus one commit
    // spends to exactly the commit cost, advances, and lands on zero.
    let methods = v2_methods();
    let mut client_hello = vhello(methods.clone(), 4);
    client_hello.limits.max_work = constant + body_len + commit_cost + 1_000;
    let mut harness = VServer::with_hellos(
        "v2-exhausted-stale",
        executable_bodies(),
        &[],
        &client_hello,
        &vhello(methods, 8),
    );
    let opened = harness.budget();
    assert_eq!(opened, constant + body_len + commit_cost + 1_000);
    let spend = opened - commit_cost;
    let exact_m = spend - constant;
    assert!(exact_m >= body_len && exact_m <= 8_388_608);
    let exact_body = entity_read_body_capped(harness.root, target, 65_535, exact_m, opened);
    let (failed, _) = harness.call(ENTITY_VERSION_TAG, exact_body);
    assert!(!failed);
    assert_eq!(harness.budget(), commit_cost);
    // Advance the head past the session-bound root, then read stale.
    let _ = advance_head_with_namespace(&mut harness);
    assert_eq!(harness.budget(), 0);
    let before = harness.budget();
    let stale = harness.refuse(ENTITY_VERSION_TAG, entity_read_body(harness.root, target));
    assert_eq!(stale.symbol, "SESSION_ROOT_ADVANCED");
    assert_eq!(harness.budget(), before);
}

/// Renewing a session then retrying with the live root succeeds
/// (discharges `seq_renewed_session` step two).
#[test]
fn entity_renew_then_read_with_live_root_succeeds() {
    let mut harness = VServer::new("v2-renew-read");
    let (new_root, _) = advance_head_with_namespace(&mut harness);
    let stale = harness.refuse(
        ENTITY_VERSION_TAG,
        entity_read_body(harness.root, sley_repo::test_support::id(30)),
    );
    assert_eq!(stale.symbol, "SESSION_ROOT_ADVANCED");
    let (renew_failed, _) = harness.call(
        Method::SessionRenew.tag(),
        harness.session.as_bytes().to_vec(),
    );
    assert!(!renew_failed);
    harness.root = new_root;
    let target = sley_repo::test_support::id(30);
    let frame = harness.read(ENTITY_VERSION_TAG, target);
    let response = decode_entity_read_response(&frame.body).unwrap();
    assert_eq!(response.root, new_root);
    assert_eq!(response.requested_entity, target);
}

#[test]
fn entity_read_exact_and_one_below_limits() {
    let mut harness = VServer::new("v2-limits");
    let id = harness.next_request;
    let signature_body = entity_read_body(harness.root, sley_repo::test_support::id(30));
    let answer = harness.call_raw(ENTITY_SIGNATURE_TAG, signature_body);
    assert!(!answer.failed);
    assert!(answer.events.is_empty());
    let (DecodedFrame::Response(frame), _) =
        decode_frame_for_version(&answer.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2)
            .unwrap()
    else {
        panic!("response frame");
    };
    let response = decode_entity_read_response(&frame.body).unwrap();
    let work = response.work_units;
    let body_len = frame.bounds.returned_bytes;
    let narrow_objects = encode_record(&[
        (1, harness.root.as_bytes().to_vec()),
        (2, sley_repo::test_support::id(30).as_bytes().to_vec()),
        (3, encode_uvar(2)),
        (4, encode_uvar(8_388_608)),
        (5, encode_uvar(100_000_000)),
    ])
    .unwrap();
    let refused = harness.refuse(ENTITY_SIGNATURE_TAG, narrow_objects);
    assert_eq!(refused.code, ProtocolErrorCode::LimitExceeded.numeric());
    let exact_work = encode_record(&[
        (1, harness.root.as_bytes().to_vec()),
        (2, sley_repo::test_support::id(30).as_bytes().to_vec()),
        (3, encode_uvar(65_535)),
        (4, encode_uvar(8_388_608)),
        (5, encode_uvar(work)),
    ])
    .unwrap();
    let (failed_exact, exact_frame) = harness.call(ENTITY_SIGNATURE_TAG, exact_work);
    assert!(!failed_exact);
    assert_eq!(
        decode_entity_read_response(&exact_frame.body)
            .unwrap()
            .work_units,
        work
    );
    let below_work = encode_record(&[
        (1, harness.root.as_bytes().to_vec()),
        (2, sley_repo::test_support::id(30).as_bytes().to_vec()),
        (3, encode_uvar(65_535)),
        (4, encode_uvar(8_388_608)),
        (5, encode_uvar(work - 1)),
    ])
    .unwrap();
    let refused_work = harness.refuse(ENTITY_SIGNATURE_TAG, below_work);
    assert_eq!(
        refused_work.code,
        ProtocolErrorCode::LimitExceeded.numeric()
    );
    let exact_bytes = encode_record(&[
        (1, harness.root.as_bytes().to_vec()),
        (2, sley_repo::test_support::id(30).as_bytes().to_vec()),
        (3, encode_uvar(65_535)),
        (4, encode_uvar(body_len)),
        (5, encode_uvar(100_000_000)),
    ])
    .unwrap();
    let (failed_bytes, _) = harness.call(ENTITY_SIGNATURE_TAG, exact_bytes);
    assert!(!failed_bytes);
    let tiny_bytes = encode_record(&[
        (1, harness.root.as_bytes().to_vec()),
        (2, sley_repo::test_support::id(30).as_bytes().to_vec()),
        (3, encode_uvar(65_535)),
        (4, encode_uvar(body_len / 2)),
        (5, encode_uvar(100_000_000)),
    ])
    .unwrap();
    let refused_bytes = harness.refuse(ENTITY_SIGNATURE_TAG, tiny_bytes);
    assert_eq!(
        refused_bytes.code,
        ProtocolErrorCode::LimitExceeded.numeric()
    );
    let full_len = frame_total_len(&crate::FrameSize {
        version: PROTOCOL_VERSION_V2,
        session: Some(harness.session),
        request_id: id,
        kind_tag: 2,
        method: ENTITY_SIGNATURE_TAG,
        flags: 0,
        bounds: frame.bounds,
        body_len,
    })
    .unwrap();
    assert_eq!(
        full_len + 8,
        u64::try_from(answer.frame.bytes.len()).unwrap()
    );
}

#[test]
fn entity_read_tiny_streaming_frame_refuses_without_events() {
    let (temp, _transactions, _) = genesis("v2-tiny", executable_bodies(), &[]);
    let repository = temp.child("repo");
    let mut client_hello = vhello(v2_methods(), 4);
    client_hello.limits.max_frame_bytes = 768;
    client_hello.features |= FEATURE_STREAM;
    let mut server_hello = vhello(v2_methods(), 8);
    server_hello.limits.max_frame_bytes = 768;
    server_hello.features |= FEATURE_STREAM;
    let mut server = Server::new_versioned(&repository, &client_hello, &server_hello).unwrap();
    let handshake = server.handshake_id();
    let open = server
        .answer(&vrequest_frame(
            None,
            0,
            Method::SessionOpen.tag(),
            handshake.as_bytes().to_vec(),
        ))
        .unwrap();
    assert!(!open.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame_for_version(&open.frame.bytes, 768, PROTOCOL_VERSION_V2).unwrap()
    else {
        panic!("open frame");
    };
    let session = SessionId::from_bytes(frame.body.as_slice().try_into().unwrap());
    let root = sley_txn::TransactionRepository::new(&repository)
        .accepted_head()
        .unwrap()
        .verified_revision()
        .state_root()
        .root;
    let answer = server
        .answer(&vrequest_frame(
            Some(session),
            1,
            ENTITY_SIGNATURE_TAG,
            entity_read_body(root, sley_repo::test_support::id(30)),
        ))
        .unwrap();
    assert!(answer.failed);
    assert!(answer.events.is_empty());
    let (DecodedFrame::Response(response), _) =
        decode_frame_for_version(&answer.frame.bytes, 768, PROTOCOL_VERSION_V2).unwrap()
    else {
        panic!("response frame");
    };
    assert_eq!(
        ProtocolFailure::decode(&response.body).unwrap().code,
        ProtocolErrorCode::LimitExceeded.numeric()
    );
}

#[test]
fn entity_read_budget_debit_table_is_exact() {
    let mut harness = VServer::with_hellos(
        "v2-budget",
        executable_bodies(),
        &[],
        &vhello(v2_methods(), 1),
        &vhello(v2_methods(), 1),
    );
    let entity = sley_repo::test_support::id(30);
    let before = harness.budget();
    let bad = harness.refuse(
        ENTITY_VERSION_TAG,
        entity_read_body(StateRoot::from_bytes([0x77; 32]), entity),
    );
    assert_eq!(bad.code, 31_008);
    assert_eq!(harness.budget(), before - 1);
    let (failed, frame) = harness.call(ENTITY_VERSION_TAG, entity_read_body(harness.root, entity));
    assert!(!failed);
    let work = decode_entity_read_response(&frame.body).unwrap().work_units;
    assert_eq!(harness.budget(), before - 1 - work);
    assert_ne!(
        before - 1 - harness.budget(),
        work + frame.body.len() as u64,
        "generic body-byte charge must not apply"
    );
    harness.server.set_entity_encode_fault(true);
    let faulted = harness.refuse(ENTITY_VERSION_TAG, entity_read_body(harness.root, entity));
    assert_eq!(faulted.code, ProtocolErrorCode::InternalInvariant.numeric());
    assert_eq!(harness.budget(), before - 1 - work - work);
    harness.server.set_entity_encode_fault(false);
    // Post-reservation cleanup and viability: after clearing the fault,
    // a real read succeeds at max_inflight 1, proving the fault refusal
    // released its slot, and pays full work again on top of the retained
    // fault debit.
    let (failed_again, again_frame) =
        harness.call(ENTITY_VERSION_TAG, entity_read_body(harness.root, entity));
    assert!(
        !failed_again,
        "session stays viable after the fault refusal"
    );
    let again_work = decode_entity_read_response(&again_frame.body)
        .unwrap()
        .work_units;
    assert_eq!(again_work, work);
    assert_eq!(again_frame.bounds.returned_entities, 1);
    assert_eq!(harness.budget(), before - 1 - work - work - again_work);
}

// ---------------------------------------------------------------------------
// Phase2 runtime correction regressions (AT-MW-02 repair checkpoint)
// ---------------------------------------------------------------------------

/// Methods without `entity.version`: the selection stays v2 but 306 is not
/// negotiated.
fn v2_methods_without_version() -> Vec<u32> {
    v2_methods()
        .into_iter()
        .filter(|tag| *tag != ENTITY_VERSION_TAG)
        .collect()
}

/// Commits a fresh namespace so the accepted head advances, returning the
/// new root and the created entity.
fn advance_head_with_namespace(harness: &mut VServer) -> (StateRoot, EntityId) {
    use sley_id::{CandidateNonce, PrincipalId};
    use sley_mutate::value::{EntityBodyValue, EntityIdSet, NamespaceBody};
    use sley_mutate::{
        BoundPrecondition, CandidateExpiry, CandidateRecord, ExpectedIdentityAbsent, MutationClass,
        MutationOperation, MutationPayload, PreconditionPayload, PreimageRequirement,
        build_candidate, full_validation_profile_id,
    };
    use sley_policy::build_capability_summary_projection;
    let transactions = sley_txn::TransactionRepository::new(&harness.repository);
    let head = transactions.accepted_head().unwrap();
    let workspace = head.state_root().record.workspace_id;
    let principal = PrincipalId::from_bytes([2; 32]);
    let nonce = CandidateNonce::from_bytes([0x77; 32]);
    let target = EntityId::derive(workspace, nonce, 3, 0);
    let summary = build_capability_summary_projection(
        principal,
        workspace,
        head.policy_root().root(),
        head.state_root().root,
        &[],
    )
    .unwrap();
    let candidate = build_candidate(&CandidateRecord {
        format_version: 1,
        workspace_id: workspace,
        base_transaction_id: head.transaction_id(),
        base_root: head.state_root().root,
        schema_epoch_id: head.state_root().record.schema_epoch_id,
        policy_root_id: head.policy_root().root(),
        principal_id: principal,
        capability_summary_digest: summary.digest(),
        operations: vec![MutationOperation {
            ordinal: 0,
            class: MutationClass::CreateEntity,
            target_kind: 3,
            target_entity: target,
            field_tag: None,
            payload: MutationPayload::CreateEntity(EntityBodyValue::Namespace(NamespaceBody {
                parent: None,
                members: EntityIdSet::from_unsorted(vec![]).unwrap(),
            })),
            precondition_ordinal: 0,
        }],
        preconditions: vec![BoundPrecondition {
            operation_ordinal: 0,
            requirement: PreimageRequirement::ExpectedIdentityAbsent,
            payload: PreconditionPayload::ExpectedIdentityAbsent(ExpectedIdentityAbsent {
                entity_id: target,
            }),
        }],
        validation_profile_id: full_validation_profile_id().unwrap(),
        candidate_nonce: nonce,
        expiry: CandidateExpiry::unix_millis(2_000),
    })
    .unwrap();
    let (commit_failed, _) = harness.call(
        Method::Commit.tag(),
        encode_record(&[
            (1, head.transaction_id().as_bytes().to_vec()),
            (2, principal.as_bytes().to_vec()),
            (3, encode_uvar(1_000)),
            (4, candidate.stored_bytes.clone()),
        ])
        .unwrap(),
    );
    assert!(!commit_failed);
    let new_root = sley_txn::TransactionRepository::new(&harness.repository)
        .accepted_head()
        .unwrap()
        .verified_revision()
        .state_root()
        .root;
    assert_ne!(new_root, harness.root);
    (new_root, target)
}

#[test]
fn repair_unnegotiated_refusal_releases_its_slot() {
    // R1 / VUL-P2-01: every admitted terminal result releases exactly one
    // request slot. With max_inflight 1, three not-negotiated refusals must
    // not block the next legitimate request.
    let methods = v2_methods_without_version();
    let mut harness = VServer::with_hellos(
        "repair-r1-unnegotiated",
        executable_bodies(),
        &[],
        &vhello(methods.clone(), 1),
        &vhello(methods, 1),
    );
    let before = harness.budget();
    for _ in 0..3 {
        let (failed, frame) = harness.call(
            ENTITY_VERSION_TAG,
            entity_read_body(harness.root, sley_repo::test_support::id(30)),
        );
        assert!(failed, "unoffered entity.version must refuse");
        assert_eq!(
            ProtocolFailure::decode(&frame.body).unwrap().code,
            ProtocolErrorCode::MethodUnsupported.numeric()
        );
    }
    assert_eq!(
        harness.budget(),
        before - 3,
        "each refused admission still costs the dispatch unit"
    );
    let frame = harness.read(ENTITY_SIGNATURE_TAG, sley_repo::test_support::id(30));
    assert_eq!(frame.bounds.returned_entities, 3);
}

#[test]
fn repair_stale_refusal_then_renew_and_read_at_inflight_one() {
    // R1 stale/session refusal sequence with increasing request IDs: the
    // stale refusal must release its slot so renewal and the next read
    // admit under max_inflight 1.
    let mut harness = VServer::with_hellos(
        "repair-r1-stale",
        executable_bodies(),
        &[],
        &vhello(v2_methods(), 1),
        &vhello(v2_methods(), 1),
    );
    let (new_root, target) = advance_head_with_namespace(&mut harness);
    let stale = harness.refuse(
        ENTITY_VERSION_TAG,
        entity_read_body(harness.root, sley_repo::test_support::id(30)),
    );
    assert_eq!(stale.symbol, "SESSION_ROOT_ADVANCED");
    let (renew_failed, _) = harness.call(
        Method::SessionRenew.tag(),
        harness.session.as_bytes().to_vec(),
    );
    assert!(!renew_failed, "renewal must admit after the stale refusal");
    harness.root = new_root;
    let frame = harness.read(ENTITY_VERSION_TAG, target);
    let response = decode_entity_read_response(&frame.body).unwrap();
    assert_eq!(response.root, new_root);
    assert_eq!(response.requested_entity, target);
}

#[test]
fn repair_session_budget_method_order_and_debit() {
    // R2 / A-P2-1: inherited session-binding, budget-exhaustion, then
    // method-negotiation order. A stale session combined with an unoffered
    // method reports the session failure with no debit; a live funded
    // unoffered method reports not-negotiated with exactly the dispatch
    // debit and leaves the session viable.
    let methods = v2_methods_without_version();
    let mut harness = VServer::with_hellos(
        "repair-r2-order",
        executable_bodies(),
        &[],
        &vhello(methods.clone(), 4),
        &vhello(methods, 4),
    );
    let (new_root, _) = advance_head_with_namespace(&mut harness);
    let before = harness.budget();
    let stale_unoffered = harness.refuse(
        ENTITY_VERSION_TAG,
        entity_read_body(harness.root, sley_repo::test_support::id(30)),
    );
    assert_eq!(
        stale_unoffered.symbol, "SESSION_ROOT_ADVANCED",
        "session binding precedes method negotiation"
    );
    assert_eq!(
        harness.budget(),
        before,
        "session errors keep no-debit semantics"
    );
    let (renew_failed, _) = harness.call(
        Method::SessionRenew.tag(),
        harness.session.as_bytes().to_vec(),
    );
    assert!(!renew_failed);
    harness.root = new_root;
    let funded = harness.budget();
    let unoffered = harness.refuse(
        ENTITY_VERSION_TAG,
        entity_read_body(harness.root, sley_repo::test_support::id(30)),
    );
    assert_eq!(
        unoffered.code,
        ProtocolErrorCode::MethodUnsupported.numeric()
    );
    assert_eq!(
        harness.budget(),
        funded - 1,
        "live unoffered method still costs the dispatch unit"
    );
    let frame = harness.read(ENTITY_SIGNATURE_TAG, sley_repo::test_support::id(30));
    assert_eq!(frame.bounds.returned_entities, 3);
}

#[test]
fn repair_exhausted_budget_precedes_unoffered_method() {
    // R2 exhaustion order: draining a max_work 3 session with live
    // unoffered requests leaves budget 0; the next unoffered request
    // reports budget exhaustion with no further debit.
    let methods = v2_methods_without_version();
    let capped = vhello_capped(methods.clone(), 4, 3, 8_388_608, 8_388_608, 1);
    let capped_server = vhello_capped(methods, 4, 3, 8_388_608, 8_388_608, 1);
    let mut harness = VServer::with_hellos(
        "repair-r2-exhausted",
        executable_bodies(),
        &[],
        &capped,
        &capped_server,
    );
    assert_eq!(harness.budget(), 3);
    for _ in 0..3 {
        let denied = harness.refuse(
            ENTITY_VERSION_TAG,
            entity_read_body_capped(
                harness.root,
                sley_repo::test_support::id(30),
                65_535,
                8_388_608,
                3,
            ),
        );
        assert_eq!(denied.code, ProtocolErrorCode::MethodUnsupported.numeric());
    }
    assert_eq!(harness.budget(), 0);
    for _ in 0..2 {
        let exhausted = harness.refuse(
            ENTITY_VERSION_TAG,
            entity_read_body_capped(
                harness.root,
                sley_repo::test_support::id(30),
                65_535,
                8_388_608,
                3,
            ),
        );
        assert_eq!(
            exhausted.code,
            ProtocolErrorCode::LimitExceeded.numeric(),
            "exhausted budget precedes method negotiation"
        );
        assert_eq!(harness.budget(), 0, "exhaustion itself debits nothing");
    }
}

#[test]
fn repair_hello_wire_version_is_always_one() {
    // R5: Hello transport is wire 1 independently of the caller's expected
    // version. A correctly shaped Hello2 is malformed even under
    // expected 2; the established claim checks are unchanged.
    let hello = Server::offered_hello_versioned().unwrap();
    let body = hello.encode().unwrap();
    let hello_frame = |version: u32| ProtocolFrame {
        protocol_version: version,
        session: None,
        request_id: 0,
        kind: FrameKind::Hello,
        method: 0,
        flags: 0,
        bounds: BoundedContext::none(),
        body: body.clone(),
    };
    assert!(
        hello_frame(PROTOCOL_VERSION)
            .validate_for_version(PROTOCOL_VERSION)
            .is_ok()
    );
    assert_eq!(
        hello_frame(PROTOCOL_VERSION)
            .validate_for_version(PROTOCOL_VERSION_V2)
            .map_err(|error| error.code()),
        Err(ProtocolErrorCode::Downgrade)
    );
    assert_eq!(
        hello_frame(PROTOCOL_VERSION_V2)
            .validate_for_version(PROTOCOL_VERSION_V2)
            .map_err(|error| error.code()),
        Err(ProtocolErrorCode::FrameInvalid),
        "Hello2 is malformed even when expected"
    );
    assert_eq!(
        hello_frame(PROTOCOL_VERSION_V2)
            .validate_for_version(PROTOCOL_VERSION)
            .map_err(|error| error.code()),
        Err(ProtocolErrorCode::VersionUnsupported)
    );
}

#[test]
fn repair_explicit_negotiation_supports_one_two_and_three() {
    // R6 / VUL-P2-04: the operational explicit path implements versions 1,
    // 2, and 3. An unsupported greatest-common selection is refused with
    // the existing VersionUnsupported code; legacy helpers keep arbitrary
    // numeric behavior and opaque/reserved tag rules are unchanged.
    fn with_versions_opaque(versions: Vec<u32>, methods: &[u32]) -> Hello {
        Hello {
            protocol_versions: versions,
            schema_epochs: vec![epoch(0x11)],
            limits: LimitProfile {
                max_frame_bytes: 8_388_608,
                max_entities: 65_535,
                max_edges: 400_000,
                max_depth: 65_535,
                max_response_bytes: 8_388_608,
                max_work: 100_000_000,
                max_inflight: 4,
                max_sessions: 256,
            },
            methods: methods.to_vec(),
            features: 1,
            adapters: vec![],
            effects: vec![],
        }
    }
    let base = vhello(v2_methods(), 4);
    let with_versions = |versions: Vec<u32>| Hello {
        protocol_versions: versions,
        ..base.clone()
    };
    for versions in [
        vec![1],
        vec![2],
        vec![3],
        vec![1, 2],
        vec![1, 2, 3],
        vec![2, 3],
    ] {
        let selected = negotiate_versioned(
            &with_versions(versions.clone()),
            &with_versions(versions.clone()),
        )
        .unwrap();
        assert_eq!(
            selected.protocol_version,
            *versions.last().unwrap(),
            "supported selection {versions:?}"
        );
    }
    for versions in [vec![4], vec![1, 4], vec![2, 4]] {
        let refused = negotiate_versioned(
            &with_versions(versions.clone()),
            &with_versions(versions.clone()),
        )
        .err()
        .unwrap_or_else(|| panic!("unsupported selection {versions:?} must fail"));
        assert_eq!(refused.code(), ProtocolErrorCode::VersionUnsupported);
    }
    // Legacy negotiation still reports the arbitrary greatest-common
    // version; only the explicit operational path gates serving.
    let legacy = negotiate(&with_versions(vec![1, 2, 3]), &with_versions(vec![1, 2, 3])).unwrap();
    assert_eq!(legacy.protocol_version, 3);
    // The explicit constructor refuses the unsupported selection before a
    // serving session can exist.
    let (temp, _, _) = genesis("repair-r6-unsupported", executable_bodies(), &[]);
    let repository = temp.child("repo");
    let bad = with_versions(vec![4]);
    assert_eq!(
        Server::new_versioned(&repository, &bad, &bad)
            .unwrap_err()
            .code(),
        ProtocolErrorCode::VersionUnsupported
    );
    // Version-aware method availability claims v3 methods on version 3
    // while the lower-version rules are unchanged.
    assert!(Method::from_tag_versioned(ENTITY_VERSION_TAG, PROTOCOL_VERSION_V2).is_ok());
    assert_eq!(
        Method::from_tag_versioned(ENTITY_VERSION_TAG, PROTOCOL_VERSION)
            .unwrap_err()
            .code(),
        ProtocolErrorCode::MethodUnsupported
    );
    assert!(Method::from_tag_versioned(ENTITY_VERSION_TAG, 3).is_ok());
    assert_eq!(
        Method::from_tag_versioned(ENTITY_VERSION_TAG, 4)
            .unwrap_err()
            .code(),
        ProtocolErrorCode::VersionUnsupported
    );
    // Opaque unrelated tags keep legacy treatment on both selections;
    // reserved tags stay invalid offers on the explicit path.
    let mut opaque = v2_methods();
    opaque.push(999);
    opaque.sort_unstable();
    let selected_v1 = negotiate_versioned(
        &with_versions_opaque(vec![1], &opaque),
        &with_versions_opaque(vec![1], &opaque),
    )
    .unwrap();
    assert_eq!(selected_v1.protocol_version, PROTOCOL_VERSION);
    assert!(selected_v1.methods.contains(&999));
    assert!(!selected_v1.methods.contains(&ENTITY_VERSION_TAG));
    assert!(!selected_v1.methods.contains(&ENTITY_SIGNATURE_TAG));
    let selected_v2 = negotiate_versioned(
        &with_versions_opaque(vec![1, 2], &opaque),
        &with_versions_opaque(vec![1, 2], &opaque),
    )
    .unwrap();
    assert_eq!(selected_v2.protocol_version, PROTOCOL_VERSION_V2);
    assert!(selected_v2.methods.contains(&999));
    assert!(selected_v2.methods.contains(&ENTITY_VERSION_TAG));
    let mut reserved = v2_methods();
    reserved.push(305);
    reserved.sort_unstable();
    assert_eq!(
        negotiate_versioned(
            &with_versions_opaque(vec![1, 2], &reserved),
            &with_versions_opaque(vec![1, 2], &reserved),
        )
        .unwrap_err()
        .code(),
        ProtocolErrorCode::PayloadInvalid
    );
}

#[test]
fn repair_full_wire_ceiling_exact_and_one_below() {
    // R4 / VUL-P2-03: the outgoing fit compares full encoded wire bytes
    // INCLUDING the 8-byte prefix, while the prefix still stores the
    // envelope length. Negotiating exactly the complete response length
    // succeeds; exactly one below refuses before reservation with debit 1,
    // no partial or event response, with streaming on or off.
    let mut learn = VServer::new("repair-r4-learn");
    let signature_body = entity_read_body(learn.root, sley_repo::test_support::id(30));
    let answer = learn.call_raw(ENTITY_SIGNATURE_TAG, signature_body);
    assert!(!answer.failed);
    let (DecodedFrame::Response(learned), _) =
        decode_frame_for_version(&answer.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2)
            .unwrap()
    else {
        panic!("response frame");
    };
    let body_len = learned.bounds.returned_bytes;
    let object_count = learned.bounds.returned_entities;
    assert_eq!(body_len, learned.body.len() as u64);

    for stream in [false, true] {
        let features = if stream {
            FEATURE_CANCEL | FEATURE_STREAM
        } else {
            FEATURE_CANCEL
        };
        let capped = |max_frame_bytes: u64| {
            vhello_capped(
                v2_methods(),
                4,
                100_000_000,
                max_frame_bytes,
                8_388_608,
                features,
            )
        };
        // The target server's bounds carry its own negotiated limits, so
        // the expected wire length is solved under those exact limits: the
        // limit value feeds the bounds encoding, hence the fixpoint.
        let wire_for = |max_frame_bytes: u64| {
            let limits = LimitProfile {
                max_frame_bytes,
                max_entities: 65_535,
                max_edges: 400_000,
                max_depth: 65_535,
                max_response_bytes: 8_388_608,
                max_work: 100_000_000,
                max_inflight: 4,
                max_sessions: 256,
            };
            let bounds = BoundedContext {
                applied_limits: limits,
                returned_bytes: body_len,
                returned_entities: object_count,
                ..BoundedContext::none()
            };
            frame_total_len(&crate::FrameSize {
                version: PROTOCOL_VERSION_V2,
                session: Some(learn.session),
                request_id: 1,
                kind_tag: 2,
                method: ENTITY_SIGNATURE_TAG,
                flags: 0,
                bounds,
                body_len,
            })
            .unwrap()
                + 8
        };
        let mut wire = wire_for(8_388_608);
        for _ in 0..3 {
            wire = wire_for(wire);
        }
        assert_eq!(
            wire_for(wire - 1),
            wire,
            "one-below ceiling must not shift the uvar widths under test"
        );

        let mut exact = VServer::with_hellos(
            &format!("repair-r4-exact-{stream}"),
            executable_bodies(),
            &[],
            &capped(wire),
            &capped(wire),
        );
        let answer = exact.call_raw(
            ENTITY_SIGNATURE_TAG,
            entity_read_body(exact.root, sley_repo::test_support::id(30)),
        );
        assert!(
            !answer.failed,
            "exact full wire length must fit (stream={stream})"
        );
        assert!(answer.events.is_empty());
        assert_eq!(
            answer.frame.bytes.len(),
            wire as usize,
            "emitted bytes equal the negotiated full length (stream={stream})"
        );
        decode_frame_for_version(&answer.frame.bytes, wire, PROTOCOL_VERSION_V2).unwrap();

        let mut below = VServer::with_hellos(
            &format!("repair-r4-below-{stream}"),
            executable_bodies(),
            &[],
            &capped(wire - 1),
            &capped(wire - 1),
        );
        let before = below.budget();
        let (failed, frame) = below.call(
            ENTITY_SIGNATURE_TAG,
            entity_read_body(below.root, sley_repo::test_support::id(30)),
        );
        assert!(failed, "one below full wire must refuse (stream={stream})");
        assert_eq!(
            ProtocolFailure::decode(&frame.body).unwrap().code,
            ProtocolErrorCode::LimitExceeded.numeric()
        );
        assert_eq!(
            below.budget(),
            before - 1,
            "preflight refusal costs only the dispatch unit (stream={stream})"
        );
        let (failed_again, _) = below.call(
            ENTITY_SIGNATURE_TAG,
            entity_read_body(below.root, sley_repo::test_support::id(30)),
        );
        assert!(failed_again);
        assert_eq!(below.budget(), before - 2);
    }
}

#[test]
fn repair_wire_length_matches_encoding_across_varint_widths() {
    // R4 preflight/writer drift guard: envelope length plus the 8-byte
    // prefix equals the actually emitted bytes across uvar width
    // boundaries for request IDs, methods, and body lengths.
    for request_id in [1, 127, 128, 16_383, 16_384, u64::MAX] {
        for body_len in [0_usize, 1, 127, 128, 300] {
            for method in [ENTITY_VERSION_TAG, ENTITY_SIGNATURE_TAG, 603] {
                let frame = ProtocolFrame {
                    protocol_version: PROTOCOL_VERSION_V2,
                    session: Some(SessionId::from_bytes([0x51; 32])),
                    request_id,
                    kind: FrameKind::Response,
                    method,
                    flags: 0,
                    bounds: BoundedContext::none(),
                    body: vec![0xAB; body_len],
                };
                let envelope = frame_total_len(&crate::FrameSize {
                    version: PROTOCOL_VERSION_V2,
                    session: frame.session,
                    request_id,
                    kind_tag: FrameKind::Response.tag(),
                    method,
                    flags: 0,
                    bounds: BoundedContext::none(),
                    body_len: body_len as u64,
                })
                .unwrap();
                let emitted = encode_single_frame_direct(&frame, MAX_FRAME_BYTES)
                    .unwrap()
                    .bytes
                    .len();
                assert_eq!(
                    emitted,
                    envelope as usize + 8,
                    "request {request_id} body {body_len} method {method}"
                );
            }
        }
    }
}

fn wide_hellos() -> (Hello, Hello) {
    (
        vhello_capped(v2_methods(), 4, 100_000_000, 67_108_864, 33_554_432, 1),
        vhello_capped(v2_methods(), 8, 100_000_000, 67_108_864, 33_554_432, 1),
    )
}

/// One Function plus six Parameters sharing one wide shallow Tuple type,
/// per the R3 aggregate-fixture design: seven stored objects each far
/// below the per-object Bytes cap whose aggregate response body exceeds
/// the outer 16MiB frame-body Bytes ceiling.
fn wide_signature_bodies() -> Vec<(u8, sley_mutate::value::EntityBodyValue)> {
    use sley_mutate::value::{
        BlockBody, EntityBodyValue, EntityIdSet, FunctionBody, ParameterBody, TypeDefBody,
    };
    use sley_repo::test_support::id;
    use sley_ssmc::{
        NamedType, ParameterRole, Reachability, ReturnTerminator, Terminator, TypeDefForm,
        TypeExpr, ValueRef, Visibility,
    };
    const TUPLE_ITEMS: usize = 65_535;
    let wide_type = |definition: EntityId| {
        TypeExpr::Tuple(
            (0..TUPLE_ITEMS)
                .map(|_| {
                    TypeExpr::Named(NamedType {
                        definition,
                        arguments: Vec::new(),
                    })
                })
                .collect(),
        )
    };
    let empty = EntityIdSet::from_unsorted(Vec::new()).unwrap();
    let mut bodies = executable_bodies();
    bodies.push((
        50,
        EntityBodyValue::TypeDef(TypeDefBody {
            type_parameters: Vec::new(),
            form: TypeDefForm::Record(Vec::new()),
            invariants: empty.clone(),
            visibility: Visibility::Private,
        }),
    ));
    bodies.push((
        51,
        EntityBodyValue::Function(FunctionBody {
            type_parameters: Vec::new(),
            parameters: vec![id(60), id(61), id(62), id(63), id(64), id(65)],
            result_type: wide_type(id(50)),
            effects: empty.clone(),
            entry_block: id(52),
            blocks: vec![id(52)],
            contracts: empty.clone(),
            visibility: Visibility::Private,
        }),
    ));
    bodies.push((
        52,
        EntityBodyValue::Block(BlockBody {
            function: id(51),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::Parameter(id(60)),
            }),
            reachability: Reachability::Required,
        }),
    ));
    for (byte, ordinal) in [(60, 0), (61, 1), (62, 2), (63, 3), (64, 4), (65, 5)] {
        bodies.push((
            byte,
            EntityBodyValue::Parameter(ParameterBody {
                owner: id(51),
                role: ParameterRole::Function,
                ordinal,
                value_type: wide_type(id(50)),
            }),
        ));
    }
    bodies
}

fn aggregate_request_body(root: StateRoot, entity: EntityId) -> Vec<u8> {
    entity_read_body_capped(root, entity, 7, 24_000_000, 100_000_000)
}

#[test]
fn repair_aggregate_signature_above_bytes_ceiling_refuses_before_reserve() {
    // R3 / VUL-P2-02: the complete frame body travels as one SCB Bytes
    // value, so the inherited MAX_BYTE_PAYLOAD ceiling binds the aggregate
    // response body, not just each stored object. A real method-307
    // signature over one Function plus six Parameters passes every
    // per-object and owner ceiling while its aggregate body exceeds the
    // outer 16MiB cap: serving must refuse with PROTOCOL_LIMIT_EXCEEDED
    // before reservation (dispatch-only debit) and leave the session
    // viable. Trusted genesis admits the inventory structurally; no full
    // candidate semantic validation is claimed here.
    use sley_query::{EntityReadCeilings, EntityReadMethod};
    let ceiling = u64::try_from(MAX_BYTE_PAYLOAD).unwrap();
    let (client, server) = wide_hellos();
    let mut harness = VServer::with_hellos(
        "repair-r3-aggregate",
        wide_signature_bodies(),
        &[],
        &client,
        &server,
    );
    let function = sley_repo::test_support::id(51);
    let request = aggregate_request_body(harness.root, function);

    // Preparation succeeds on the admitted revision: K/body/W metadata
    // proves the transport bound is reached, not an earlier owner check.
    // Every other ceiling is eliminated below, so the outer Bytes cap is
    // the only intended first failing bound.
    let revision = sley_txn::TransactionRepository::new(&harness.repository)
        .accepted_head()
        .unwrap()
        .verified_revision()
        .clone();
    let epoch = revision.state_root().record.schema_epoch_id;
    for byte in [51, 60, 61, 62, 63, 64, 65] {
        let entity = sley_repo::test_support::id(byte);
        let object = revision
            .objects()
            .iter()
            .find(|object| object.record().entity_id == entity)
            .unwrap_or_else(|| panic!("wide object {byte} missing"));
        assert!(
            (object.stored_bytes().len() as u64) < ceiling,
            "wide object {byte} must pass its own Bytes cap"
        );
        let imported = sley_mutate::import_entity_object(epoch, object.stored_bytes()).unwrap();
        assert_eq!(imported.object_id(), object.object_id());
    }
    let before = harness.budget();
    let ceilings = EntityReadCeilings {
        max_entities: 65_535,
        max_response_bytes: 33_554_432,
        max_work: 100_000_000,
        budget_before_dispatch: before,
    };
    let (_, selection) = sley_repo::prepare_verified_entity_read(
        &revision,
        EntityReadMethod::Signature,
        &request,
        &ceilings,
    )
    .unwrap();
    assert_eq!(selection.object_count(), 7);
    assert!(
        selection.body_len() > ceiling,
        "aggregate body must exceed the outer Bytes cap"
    );
    assert!(
        selection.body_len() <= 24_000_000,
        "aggregate body must fit the request ceiling"
    );
    assert!(
        selection.work_units() <= before,
        "aggregate work must fit the live session budget"
    );

    // The raw answer is asserted as a server failure before any client
    // frame decode: on unfixed production this is the oversized success
    // itself, which must never be decoded or unwrapped here.
    let answer = harness.call_raw(ENTITY_SIGNATURE_TAG, request);
    assert!(
        answer.failed,
        "aggregate above the Bytes ceiling must refuse"
    );
    assert!(
        answer.events.is_empty(),
        "outer-ceiling refusal carries no partial or event response"
    );
    let (DecodedFrame::Response(frame), _) =
        decode_frame_for_version(&answer.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2)
            .unwrap()
    else {
        panic!("response frame");
    };
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::LimitExceeded.numeric()
    );
    assert_eq!(
        harness.budget(),
        before - 1,
        "outer-ceiling refusal costs only the dispatch unit: no work reserved"
    );
    let followup = harness.read(ENTITY_VERSION_TAG, sley_repo::test_support::id(31));
    assert_eq!(followup.bounds.returned_entities, 1);
}

#[test]
fn repair_frame_body_bytes_ceiling_edges() {
    // R3 metadata/writer edges, distinct from the aggregate serving
    // fixture: the canonical Bytes encoder and the direct frame writer
    // agree on the inherited 16MiB body ceiling.
    let ceiling = MAX_BYTE_PAYLOAD;
    assert!(encode_bytes(&vec![0u8; ceiling]).is_ok());
    assert!(encode_bytes(&vec![0u8; ceiling + 1]).is_err());
    let frame_with_body = |len: usize| ProtocolFrame {
        protocol_version: PROTOCOL_VERSION_V2,
        session: Some(SessionId::from_bytes([0x51; 32])),
        request_id: 1,
        kind: FrameKind::Response,
        method: ENTITY_SIGNATURE_TAG,
        flags: 0,
        bounds: BoundedContext::none(),
        body: vec![0xAB; len],
    };
    assert!(
        encode_single_frame_direct(&frame_with_body(ceiling), MAX_FRAME_BYTES).is_ok(),
        "a body exactly at the Bytes ceiling still encodes"
    );
    let over = encode_single_frame_direct(&frame_with_body(ceiling + 1), MAX_FRAME_BYTES);
    assert!(
        over.is_err(),
        "one byte over the Bytes ceiling must not encode"
    );
    assert_eq!(
        over.unwrap_err().code(),
        ProtocolErrorCode::LimitExceeded,
        "the direct writer refuses one byte over the Bytes ceiling"
    );
}

#[test]
fn repair_exact_and_one_below_object_count() {
    // R7 object-count dimension: a four-object signature serves with
    // max_objects 4 and refuses with max_objects 3.
    let mut counts = VServer::with_bodies("repair-r7-counts", nonmonotonic_bodies(), &[]);
    let root = counts.root;
    let function = EntityId::from_bytes([40; 32]);
    let exact_k = entity_read_body_capped(root, function, 4, 8_388_608, 100_000_000);
    let (failed_exact, exact_frame) = counts.call(ENTITY_SIGNATURE_TAG, exact_k);
    assert!(!failed_exact, "exact object count must serve");
    assert_eq!(exact_frame.bounds.returned_entities, 4);
    let below_k = entity_read_body_capped(root, function, 3, 8_388_608, 100_000_000);
    let denied_k = counts.refuse(ENTITY_SIGNATURE_TAG, below_k);
    assert_eq!(denied_k.code, ProtocolErrorCode::LimitExceeded.numeric());
}

#[test]
fn repair_response_body_ceiling_tracks_work_feedback() {
    // R7 response-body dimension: the ceiling that binds is found live,
    // because the charged work rides inside the body and shrinking the
    // ceiling also shrinks the body. The learned length serves exactly;
    // the descended threshold is the first ceiling that refuses.
    let mut bodies = VServer::new("repair-r7-body");
    let signature = entity_read_body(bodies.root, sley_repo::test_support::id(30));
    let answer = bodies.call_raw(ENTITY_SIGNATURE_TAG, signature);
    assert!(!answer.failed);
    let (DecodedFrame::Response(frame), _) =
        decode_frame_for_version(&answer.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2)
            .unwrap()
    else {
        panic!("response frame");
    };
    let _ = decode_entity_read_response(&frame.body).unwrap();
    let body_len = frame.bounds.returned_bytes;
    let exact_bytes = entity_read_body_capped(
        bodies.root,
        sley_repo::test_support::id(30),
        65_535,
        body_len,
        100_000_000,
    );
    let (failed_bytes, _) = bodies.call(ENTITY_SIGNATURE_TAG, exact_bytes);
    assert!(!failed_bytes, "exact response bytes must serve");
    // The charged work rides inside the body, so shrinking the ceiling
    // also shrinks the body: descend to the true work-feedback-stable
    // threshold where the ceiling binds exactly.
    assert!(body_len > 16);
    let mut threshold = body_len;
    while {
        let capped = entity_read_body_capped(
            bodies.root,
            sley_repo::test_support::id(30),
            65_535,
            threshold - 1,
            100_000_000,
        );
        let (failed, _) = bodies.call(ENTITY_SIGNATURE_TAG, capped);
        !failed
    } {
        threshold -= 1;
    }
    assert!(threshold > 16);
    let denied_bytes = bodies.refuse(
        ENTITY_SIGNATURE_TAG,
        entity_read_body_capped(
            bodies.root,
            sley_repo::test_support::id(30),
            65_535,
            threshold - 1,
            100_000_000,
        ),
    );
    assert_eq!(
        denied_bytes.code,
        ProtocolErrorCode::LimitExceeded.numeric()
    );
}

#[test]
fn repair_exact_and_one_below_session_budget() {
    // R7 work dimension: a session budgeted at exactly the observed work
    // serves once to zero; the next request reports exhaustion with no
    // further debit. The work figure comes from a live read of the same
    // repository and request shape on a generously budgeted server.
    let mut learn = VServer::new("repair-r7-work-learn");
    let entity = sley_repo::test_support::id(30);
    let frame = learn.read(ENTITY_VERSION_TAG, entity);
    let work = decode_entity_read_response(&frame.body).unwrap().work_units;
    assert!(work > 1);
    let capped = vhello_capped(v2_methods(), 4, work, 8_388_608, 8_388_608, 1);
    let mut alias = VServer::alias_on_repo(
        "repair-r7-budget",
        learn.repository.clone(),
        &capped,
        &capped,
    );
    assert_eq!(alias.budget(), work);
    let exact_work = entity_read_body_capped(alias.root, entity, 65_535, 8_388_608, work);
    let (failed_work, _) = alias.call(ENTITY_VERSION_TAG, exact_work);
    assert!(!failed_work, "exact session budget must serve");
    assert_eq!(alias.budget(), 0);
    let exhausted = alias.refuse(
        ENTITY_VERSION_TAG,
        entity_read_body_capped(alias.root, entity, 65_535, 8_388_608, work),
    );
    assert_eq!(exhausted.code, ProtocolErrorCode::LimitExceeded.numeric());
    assert_eq!(alias.budget(), 0);
}

#[test]
fn repair_foreign_workspace_swap_refuses_entity_read() {
    // R7 selected wrong-workspace context: swapping in a foreign-workspace
    // repository makes the live session foreign; the entity read refuses
    // with SESSION_WORKSPACE_MISMATCH, no debit, and the session serves
    // again once its own repository is restored (slot released).
    use sley_repo::test_support::{TempDir, genesis_in_workspace};
    let mut harness = VServer::new("repair-r7-swap");
    let before = harness.budget();
    let (foreign_temp, _, _) =
        genesis_in_workspace("repair-r7-foreign", dependency_free_bodies(), &[], 2);
    let parked = TempDir::new("repair-r7-parked");
    std::fs::rename(&harness.repository, parked.child("repo")).unwrap();
    std::fs::rename(foreign_temp.child("repo"), &harness.repository).unwrap();
    let mismatch = harness.refuse(
        ENTITY_VERSION_TAG,
        entity_read_body(harness.root, sley_repo::test_support::id(30)),
    );
    assert_eq!(mismatch.symbol, "SESSION_WORKSPACE_MISMATCH");
    assert_eq!(mismatch.code, 33_001);
    assert_eq!(harness.budget(), before, "session refusals debit nothing");
    std::fs::rename(&harness.repository, foreign_temp.child("repo")).unwrap();
    std::fs::rename(parked.child("repo"), &harness.repository).unwrap();
    let frame = harness.read(ENTITY_VERSION_TAG, sley_repo::test_support::id(30));
    assert_eq!(frame.bounds.returned_entities, 1);
}

#[test]
fn repair_debit_phases_observe_budget_and_followup() {
    // R7 debit table: every pre-reservation failure phase costs exactly
    // the dispatch unit, session-binding failures cost nothing, and a
    // follow-up request stays viable after each.
    let mut harness = VServer::new("repair-r7-debit");
    let entity = sley_repo::test_support::id(30);
    let start = harness.budget();
    let frame = harness.read(ENTITY_VERSION_TAG, entity);
    let first_work = decode_entity_read_response(&frame.body).unwrap().work_units;
    let mut expected = start - first_work;

    let junk = harness.refuse(ENTITY_VERSION_TAG, b"junk".to_vec());
    assert_eq!(junk.code, ProtocolErrorCode::PayloadInvalid.numeric());
    expected -= 1;
    assert_eq!(harness.budget(), expected);

    let unknown = harness.refuse(
        ENTITY_VERSION_TAG,
        entity_read_body(harness.root, EntityId::from_bytes([0x63; 32])),
    );
    assert_eq!(unknown.code, 31_004);
    expected -= 1;
    assert_eq!(harness.budget(), expected);

    let wrong_kind = harness.refuse(
        ENTITY_SIGNATURE_TAG,
        entity_read_body(harness.root, sley_repo::test_support::id(34)),
    );
    assert_eq!(wrong_kind.code, 31_010);
    expected -= 1;
    assert_eq!(harness.budget(), expected);

    let narrow_objects = entity_read_body_capped(harness.root, entity, 2, 8_388_608, 100_000_000);
    let denied_k = harness.refuse(ENTITY_SIGNATURE_TAG, narrow_objects);
    assert_eq!(denied_k.code, ProtocolErrorCode::LimitExceeded.numeric());
    expected -= 1;
    assert_eq!(harness.budget(), expected);

    let over_work = encode_record(&[
        (1, harness.root.as_bytes().to_vec()),
        (2, entity.as_bytes().to_vec()),
        (3, encode_uvar(65_535)),
        (4, encode_uvar(8_388_608)),
        (5, encode_uvar(u64::MAX)),
    ])
    .unwrap();
    let denied_work = harness.refuse(ENTITY_VERSION_TAG, over_work);
    assert_eq!(denied_work.code, ProtocolErrorCode::LimitExceeded.numeric());
    expected -= 1;
    assert_eq!(harness.budget(), expected);

    let tiny_bytes = entity_read_body_capped(harness.root, entity, 65_535, 16, 100_000_000);
    let denied_bytes = harness.refuse(ENTITY_SIGNATURE_TAG, tiny_bytes);
    assert_eq!(
        denied_bytes.code,
        ProtocolErrorCode::LimitExceeded.numeric()
    );
    expected -= 1;
    assert_eq!(harness.budget(), expected);

    let wrong_root = harness.refuse(
        ENTITY_VERSION_TAG,
        entity_read_body(StateRoot::from_bytes([0x77; 32]), entity),
    );
    assert_eq!(wrong_root.code, 31_008);
    expected -= 1;
    assert_eq!(harness.budget(), expected);

    let followup = harness.read(ENTITY_VERSION_TAG, sley_repo::test_support::id(31));
    assert_eq!(followup.bounds.returned_entities, 1);
    expected -= decode_entity_read_response(&followup.body)
        .unwrap()
        .work_units;
    assert_eq!(harness.budget(), expected);

    // Session-binding failures debit nothing and release the slot.
    let (new_root, target) = advance_head_with_namespace(&mut harness);
    let after_commit = harness.budget();
    for _ in 0..2 {
        let stale = harness.refuse(ENTITY_VERSION_TAG, entity_read_body(harness.root, entity));
        assert_eq!(stale.symbol, "SESSION_ROOT_ADVANCED");
        assert_eq!(harness.budget(), after_commit);
    }
    let (renew_failed, _) = harness.call(
        Method::SessionRenew.tag(),
        harness.session.as_bytes().to_vec(),
    );
    assert!(!renew_failed);
    harness.root = new_root;
    let renewed = harness.read(ENTITY_VERSION_TAG, target);
    assert_eq!(
        decode_entity_read_response(&renewed.body).unwrap().root,
        new_root
    );
}

#[test]
fn repair_authenticated_hello2_decode_rejects_wire_violation() {
    // R5 full-decoder companion to the header-only wire-version test: an
    // authenticated Hello2 frame (real epoch/digest/prefix from the
    // production envelope, valid Hello body) must still be rejected under
    // expected 2 with the existing FrameInvalid code. The wire-1 controls
    // prove the fixture pipeline is sound, so the primary assertion can
    // fail only because baseline accepts the malformed Hello2.
    let hello = Server::offered_hello_versioned().unwrap();
    let body = hello.encode().unwrap();
    let hello1_bytes = crate::encode_hello_frame(&hello).unwrap().bytes;
    assert!(
        matches!(
            decode_frame_for_version(&hello1_bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION),
            Ok((DecodedFrame::Hello(_), _))
        ),
        "real Hello1 decodes under expected 1"
    );
    assert_eq!(
        decode_frame_for_version(&hello1_bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2)
            .unwrap_err()
            .code(),
        ProtocolErrorCode::Downgrade,
        "Hello1 under expected 2 stays a downgrade"
    );
    // ProtocolFrame::payload serializes without header validation, so the
    // malformed version rides a real production envelope untouched.
    let hello2 = ProtocolFrame {
        protocol_version: PROTOCOL_VERSION_V2,
        session: None,
        request_id: 0,
        kind: FrameKind::Hello,
        method: 0,
        flags: 0,
        bounds: BoundedContext::none(),
        body,
    };
    let hello2_bytes = crate::encode_envelope(FrameKind::Hello, &hello2.payload().unwrap())
        .unwrap()
        .bytes;
    assert_eq!(
        decode_frame_for_version(&hello2_bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2)
            .unwrap_err()
            .code(),
        ProtocolErrorCode::FrameInvalid,
        "authenticated Hello2 is malformed even when expected"
    );
}

#[test]
fn repair_frame_metadata_body_ceiling_exact_and_one_over() {
    // R3 metadata-only outer Bytes cap, distinct from the direct-writer
    // and aggregate-serving tests: frame_total_len is the shared metadata
    // path feeding preflight and the writer, so it admits exactly
    // MAX_BYTE_PAYLOAD and refuses one byte over with LimitExceeded. No
    // body allocation backs this fixture.
    let ceiling = u64::try_from(MAX_BYTE_PAYLOAD).unwrap();
    let metadata = |body_len: u64| crate::FrameSize {
        version: PROTOCOL_VERSION_V2,
        session: Some(SessionId::from_bytes([0x51; 32])),
        request_id: 1,
        kind_tag: FrameKind::Response.tag(),
        method: ENTITY_SIGNATURE_TAG,
        flags: 0,
        bounds: BoundedContext {
            applied_limits: LimitProfile::maximum(),
            ..BoundedContext::none()
        },
        body_len,
    };
    assert!(
        frame_total_len(&metadata(ceiling)).is_ok(),
        "metadata exactly at the Bytes ceiling stays encodable"
    );
    assert_eq!(
        frame_total_len(&metadata(ceiling + 1)).unwrap_err().code(),
        ProtocolErrorCode::LimitExceeded,
        "metadata one byte over the Bytes ceiling refuses"
    );
}

#[test]
fn repair_entity_read_uses_single_admitted_head_load() {
    // R7 one-snapshot proof: the retained admitted VerifiedRevision
    // supplies the entire answer. The counter sits at the single Server
    // head-loading surface, so exactly one load per answer proves
    // preparation and encoding ran on the retained revision with no
    // second load.
    let mut harness = VServer::new("repair-r7-one-snapshot");
    harness.server.reset_head_load_count();
    let entity = sley_repo::test_support::id(30);
    let frame = harness.read(ENTITY_VERSION_TAG, entity);
    assert_eq!(frame.bounds.returned_entities, 1);
    assert_eq!(
        harness.server.head_load_count(),
        1,
        "one answer loads the accepted head exactly once"
    );
    let response = decode_entity_read_response(&frame.body).unwrap();
    assert_eq!(response.root, harness.root);
    assert_eq!(response.requested_entity, entity);
    // The pre-reservation failure path binds the same single snapshot.
    harness.server.reset_head_load_count();
    let denied = harness.refuse(ENTITY_VERSION_TAG, b"junk".to_vec());
    assert_eq!(denied.code, ProtocolErrorCode::PayloadInvalid.numeric());
    assert_eq!(harness.server.head_load_count(), 1);
}

#[test]
fn repair_wrong_epoch_refuses_entity_read_without_debit() {
    // R7 wrong-epoch context: a live session whose bound epoch no longer
    // matches the head refuses with SESSION_EPOCH_MISMATCH, debits
    // nothing, and releases its slot under max_inflight 1.
    let mut harness = VServer::with_hellos(
        "repair-r7-epoch",
        executable_bodies(),
        &[],
        &vhello(v2_methods(), 1),
        &vhello(v2_methods(), 1),
    );
    let bound = harness
        .server
        .authority_mut()
        .record(harness.session)
        .map(|record| record.schema_epoch)
        .unwrap();
    let mut wrong = *bound.as_bytes();
    wrong[0] ^= 0x01;
    harness
        .server
        .authority_mut()
        .set_session_epoch_for_test(harness.session, sley_id::SchemaEpochId::from_bytes(wrong))
        .unwrap();
    let before = harness.budget();
    for _ in 0..2 {
        let denied = harness.refuse(
            ENTITY_VERSION_TAG,
            entity_read_body(harness.root, sley_repo::test_support::id(30)),
        );
        assert_eq!(denied.symbol, "SESSION_EPOCH_MISMATCH");
        assert_eq!(denied.code, 33_003);
        assert_eq!(harness.budget(), before, "session refusals debit nothing");
    }
}

#[test]
fn repair_one_below_session_budget_refuses_before_reserve() {
    // R7 work dimension, true one-below case: selected max_work and
    // request max_work both stay at the observed work W, so the request
    // range stays admitted. One genuine admitted dispatch on a malformed
    // owner request leaves actual remaining budget exactly W-1; the legal
    // unchanged target/request/M then refuses with LimitExceeded before
    // reservation at only one more dispatch debit, with no events.
    // max_inflight 1 proves each refusal released its slot.
    let mut learn = VServer::new("repair-r7-work-learn-below");
    let entity = sley_repo::test_support::id(30);
    let frame = learn.read(ENTITY_VERSION_TAG, entity);
    let work = decode_entity_read_response(&frame.body).unwrap().work_units;
    assert!(work > 1);
    let capped = vhello_capped(v2_methods(), 1, work, 8_388_608, 8_388_608, 1);
    let mut harness = VServer::alias_on_repo(
        "repair-r7-budget-below",
        learn.repository.clone(),
        &capped,
        &capped,
    );
    assert_eq!(harness.budget(), work);
    let malformed = harness.refuse(ENTITY_VERSION_TAG, b"junk".to_vec());
    assert_eq!(malformed.code, ProtocolErrorCode::PayloadInvalid.numeric());
    assert_eq!(
        harness.budget(),
        work - 1,
        "one genuine dispatch leaves remaining budget exactly W-1"
    );
    let before = harness.budget();
    let answer = harness.call_raw(
        ENTITY_VERSION_TAG,
        entity_read_body_capped(harness.root, entity, 65_535, 8_388_608, work),
    );
    assert!(
        answer.failed,
        "remaining budget one below required work must refuse"
    );
    assert!(
        answer.events.is_empty(),
        "one-below refusal carries no events"
    );
    let (DecodedFrame::Response(response), _) =
        decode_frame_for_version(&answer.frame.bytes, MAX_FRAME_BYTES, PROTOCOL_VERSION_V2)
            .unwrap()
    else {
        panic!("response frame");
    };
    assert_eq!(
        ProtocolFailure::decode(&response.body).unwrap().code,
        ProtocolErrorCode::LimitExceeded.numeric()
    );
    assert_eq!(
        harness.budget(),
        before - 1,
        "one-below refusal costs only the dispatch unit: no work reserved"
    );
}
