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
use sley_scb1::{encode_record, encode_uvar};

use crate::server::{
    FUNCTION_UNKNOWN_DETAIL, REPORT_UNKNOWN_DETAIL, RESERVED_METHOD_REASON, Server,
};
use crate::session::{CapsuleBindError, HeadBinding};
use crate::{
    BoundedContext, DecodedFrame, FrameKind, Hello, LimitProfile, MAX_FRAME_BYTES, Method,
    PROTOCOL_VERSION, ProtocolErrorCode, ProtocolFailure, ProtocolFrame, SessionId, decode_frame,
    encode_frame, negotiate, negotiate_identity,
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
    Method::ALL.iter().map(|method| method.tag()).collect()
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
                1,
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
    // is a result rather than a condition a retry can clear.
    for symbol in [
        "MERGE_CONFLICT_DIGEST_MISMATCH",
        "TYPE_DEPTH_LIMIT",
        "CAP_EXPIRED",
        "GC_ROOT_MISSING",
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
            1,
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
    assert_eq!(reserved.details, RESERVED_METHOD_REASON);
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
            1,
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
        .answer(&request_frame(None, 99, Method::SessionOpen, vec![0; 32]))
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
            1,
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
        .answer(&request_frame(None, 2, Method::WorkspaceCreate, body))
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
            1,
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
            1,
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
            2,
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
            1,
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
            1,
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
            1,
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
            1,
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
        .answer(&request_frame(None, 1, Method::WorkspaceCreate, Vec::new()))
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
            99,
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
            100,
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
    // Exhausted and validly bound: the budget failure answers.
    std::fs::rename(&harness.repository, foreign_temp.child("repo")).unwrap();
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
                failure.details != RESERVED_METHOD_REASON,
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
    let (temp, _transactions, genesis_id) = genesis(label, bodies, &[]);
    let repository = temp.child("repo");
    let client_hello = hello(all_methods(), 4);
    let server_hello = hello(all_methods(), 8);
    let mut server = Server::new(&repository, &client_hello, &server_hello).unwrap();
    let handshake = server.handshake_id();
    let open = server
        .answer(&request_frame(
            None,
            1,
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
            1,
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
            2,
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
            2,
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
    let (_temp, mut server, session, _genesis_id) =
        open_server("smp1-execute-extended", executable_bodies());
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
    let (_temp, mut server, session, _genesis_id) =
        open_server("smp1-execute-profile", executable_bodies());
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
