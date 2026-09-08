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
            vhello(v2_methods(), 4),
            vhello(v2_methods(), 8),
        )
    }

    fn with_hellos(
        label: &str,
        bodies: Vec<(u8, sley_mutate::value::EntityBodyValue)>,
        dependency_roots: &[StateRoot],
        client_hello: Hello,
        server_hello: Hello,
    ) -> Self {
        Self::with_hellos_bodies(label, bodies, dependency_roots, client_hello, server_hello)
    }

    fn with_hellos_bodies(
        label: &str,
        bodies: Vec<(u8, sley_mutate::value::EntityBodyValue)>,
        dependency_roots: &[StateRoot],
        client_hello: Hello,
        server_hello: Hello,
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
        client_hello: Hello,
        server_hello: Hello,
    ) -> Self {
        let temp = sley_repo::test_support::TempDir::new(alias_label);
        Self::open(temp, repository, client_hello, server_hello)
    }

    fn open_on_repo(
        temp: sley_repo::test_support::TempDir,
        client_hello: Hello,
        server_hello: Hello,
    ) -> Self {
        let repository = temp.child("repo");
        Self::open(temp, repository, client_hello, server_hello)
    }

    fn open(
        temp: sley_repo::test_support::TempDir,
        repository: std::path::PathBuf,
        client_hello: Hello,
        server_hello: Hello,
    ) -> Self {
        let mut server =
            Server::new_versioned(&repository, &client_hello, &server_hello).unwrap();
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
    assert_eq!(label.protocol_versions, vec![PROTOCOL_VERSION, PROTOCOL_VERSION_V2]);
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
    let v3_open = encode_frame_for_version(
        &ProtocolFrame {
            protocol_version: 3,
            session: None,
            request_id: 0,
            kind: FrameKind::Request,
            method: Method::SessionOpen.tag(),
            flags: 0,
            bounds: BoundedContext::none(),
            body: harness.server.handshake_id().as_bytes().to_vec(),
        },
        3,
    )
    .unwrap()
    .bytes;
    let answer = harness.server.answer(&v3_open).unwrap();
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
    let transactions =
        sley_txn::TransactionRepository::new(&harness.repository).accepted_head();
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
    assert_eq!(response.workspace, revision.state_root().record.workspace_id);
    assert_eq!(
        response.epoch,
        revision.state_root().record.schema_epoch_id
    );
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
    let order: Vec<EntityId> = response.objects.iter().map(|object| object.entity).collect();
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
        entity_read_body(StateRoot::from_bytes([0x77; 32]), sley_repo::test_support::id(30)),
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
    use sley_mutate::{
        BoundPrecondition, CandidateExpiry, CandidateRecord, ExpectedIdentityAbsent,
        MutationClass, MutationOperation, MutationPayload, PreconditionPayload,
        PreimageRequirement, build_candidate, full_validation_profile_id,
    };
    use sley_mutate::value::{EntityBodyValue, EntityIdSet, NamespaceBody};
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
    let (renew_failed, _) = harness.call(Method::SessionRenew.tag(), harness.session.as_bytes().to_vec());
    assert!(!renew_failed);
    harness.root = new_root;
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
    let mut harness = VServer::new("v2-budget");
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
    assert_eq!(
        faulted.code,
        ProtocolErrorCode::InternalInvariant.numeric()
    );
    assert_eq!(harness.budget(), before - 1 - work - work);
    harness.server.set_entity_encode_fault(false);
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
        BoundPrecondition, CandidateExpiry, CandidateRecord, ExpectedIdentityAbsent,
        MutationClass, MutationOperation, MutationPayload, PreconditionPayload,
        PreimageRequirement, build_candidate, full_validation_profile_id,
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
        vhello(methods.clone(), 1),
        vhello(methods, 1),
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
    let frame = harness.read(
        ENTITY_SIGNATURE_TAG,
        sley_repo::test_support::id(30),
    );
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
        vhello(v2_methods(), 1),
        vhello(v2_methods(), 1),
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
        vhello(methods.clone(), 4),
        vhello(methods, 4),
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
    assert_eq!(harness.budget(), before, "session errors keep no-debit semantics");
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
    let frame = harness.read(
        ENTITY_SIGNATURE_TAG,
        sley_repo::test_support::id(30),
    );
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
        capped,
        capped_server,
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
        assert_eq!(
            denied.code,
            ProtocolErrorCode::MethodUnsupported.numeric()
        );
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
fn repair_explicit_negotiation_supports_only_one_and_two() {
    // R6 / VUL-P2-04: the operational explicit path implements versions 1
    // and 2 only. An unsupported greatest-common selection is refused with
    // the existing VersionUnsupported code; legacy helpers keep arbitrary
    // numeric behavior and opaque/reserved tag rules are unchanged.
    let base = vhello(v2_methods(), 4);
    let with_versions = |versions: Vec<u32>| Hello {
        protocol_versions: versions,
        ..base.clone()
    };
    for versions in [vec![1], vec![2], vec![1, 2]] {
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
    for versions in [vec![3], vec![1, 2, 3], vec![2, 3]] {
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
    let bad = with_versions(vec![1, 2, 3]);
    assert_eq!(
        Server::new_versioned(&repository, &bad, &bad)
            .unwrap_err()
            .code(),
        ProtocolErrorCode::VersionUnsupported
    );
    // Version-aware method availability claims nothing for version 3 while
    // the supported version rules are unchanged.
    assert!(Method::from_tag_versioned(ENTITY_VERSION_TAG, PROTOCOL_VERSION_V2).is_ok());
    assert_eq!(
        Method::from_tag_versioned(ENTITY_VERSION_TAG, PROTOCOL_VERSION)
            .unwrap_err()
            .code(),
        ProtocolErrorCode::MethodUnsupported
    );
    assert_eq!(
        Method::from_tag_versioned(ENTITY_VERSION_TAG, 3)
            .unwrap_err()
            .code(),
        ProtocolErrorCode::VersionUnsupported
    );
    // Opaque unrelated tags keep legacy treatment on both selections;
    // reserved tags stay invalid offers on the explicit path.
    let mut opaque = v2_methods();
    opaque.push(999);
    opaque.sort_unstable();
    let selected_v1 =
        negotiate_versioned(&with_versions_opaque(vec![1], &opaque), &with_versions_opaque(vec![1], &opaque))
            .unwrap();
    assert_eq!(selected_v1.protocol_version, PROTOCOL_VERSION);
    assert!(selected_v1.methods.contains(&999));
    assert!(!selected_v1.methods.contains(&ENTITY_VERSION_TAG));
    assert!(!selected_v1.methods.contains(&ENTITY_SIGNATURE_TAG));
    let selected_v2 =
        negotiate_versioned(&with_versions_opaque(vec![1, 2], &opaque), &with_versions_opaque(vec![1, 2], &opaque))
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
            vhello_capped(v2_methods(), 4, 100_000_000, max_frame_bytes, 8_388_608, features)
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
            capped(wire),
            capped(wire),
        );
        let answer = exact.call_raw(
            ENTITY_SIGNATURE_TAG,
            entity_read_body(exact.root, sley_repo::test_support::id(30)),
        );
        assert!(!answer.failed, "exact full wire length must fit (stream={stream})");
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
            capped(wire - 1),
            capped(wire - 1),
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
        client,
        server,
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
            object.stored_bytes().len() as u64 < ceiling,
            "wide object {byte} must pass its own Bytes cap"
        );
        let imported =
            sley_mutate::import_entity_object(epoch, object.stored_bytes()).unwrap();
        assert_eq!(imported.object_id(), object.object_id());
    }
    let before = harness.budget();
    let ceilings = EntityReadCeilings {
        max_entities: 65_535,
        max_response_bytes: 33_554_432,
        max_work: 100_000_000,
        budget_before_dispatch: before,
    };
    let (_, plan) = sley_repo::prepare_verified_entity_read(
        &revision,
        EntityReadMethod::Signature,
        &request,
        &ceilings,
    )
    .unwrap();
    assert_eq!(plan.object_count(), 7);
    assert!(
        plan.body_len() > ceiling,
        "aggregate body must exceed the outer Bytes cap"
    );
    assert!(
        plan.body_len() <= 24_000_000,
        "aggregate body must fit the request ceiling"
    );
    assert!(
        plan.work_units() <= before,
        "aggregate work must fit the live session budget"
    );

    let (failed, frame) = harness.call(ENTITY_SIGNATURE_TAG, request);
    assert!(failed, "aggregate above the Bytes ceiling must refuse");
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().code,
        ProtocolErrorCode::LimitExceeded.numeric()
    );
    assert_eq!(
        harness.budget(),
        before - 1,
        "outer-ceiling refusal costs only the dispatch unit: no work reserved"
    );
    let followup = harness.read(
        ENTITY_VERSION_TAG,
        sley_repo::test_support::id(31),
    );
    assert_eq!(followup.bounds.returned_entities, 1);
}

#[test]
fn repair_frame_body_bytes_ceiling_edges() {
    // R3 metadata/writer edges, distinct from the aggregate serving
    // fixture: the canonical Bytes encoder and the direct frame writer
    // agree on the inherited 16MiB body ceiling.
    let ceiling = usize::try_from(MAX_BYTE_PAYLOAD).unwrap();
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
    assert_eq!(
        encode_single_frame_direct(&frame_with_body(ceiling + 1), MAX_FRAME_BYTES)
            .unwrap_err()
            .code(),
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
    let exact_bytes =
        entity_read_body_capped(bodies.root, sley_repo::test_support::id(30), 65_535, body_len, 100_000_000);
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
    assert_eq!(denied_bytes.code, ProtocolErrorCode::LimitExceeded.numeric());
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
        capped.clone(),
        capped,
    );
    assert_eq!(alias.budget(), work);
    let exact_work =
        entity_read_body_capped(alias.root, entity, 65_535, 8_388_608, work);
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
    let frame = harness.read(
        ENTITY_VERSION_TAG,
        sley_repo::test_support::id(30),
    );
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
    assert_eq!(
        denied_work.code,
        ProtocolErrorCode::LimitExceeded.numeric()
    );
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
    expected -= decode_entity_read_response(&followup.body).unwrap().work_units;
    assert_eq!(harness.budget(), expected);

    // Session-binding failures debit nothing and release the slot.
    let (new_root, target) = advance_head_with_namespace(&mut harness);
    let after_commit = harness.budget();
    for _ in 0..2 {
        let stale = harness.refuse(
            ENTITY_VERSION_TAG,
            entity_read_body(harness.root, entity),
        );
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
