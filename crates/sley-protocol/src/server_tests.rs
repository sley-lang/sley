//! Deterministic server tests (S20-410 slice B) over a trusted genesis
//! repository built through the repository test-support fixtures.

#![allow(clippy::too_many_lines, clippy::cast_possible_truncation)]

use sley_id::{SchemaEpochId, TransactionId};
use sley_query::{
    ModeledEntityKind, QueryLimits, RestrictedQuery, RootQuery, SnapshotContext,
    build_index_snapshot, build_restricted_query_request,
};
use sley_repo::test_support::{complete_bodies, complete_dependency_root, genesis};
use sley_repo::{CompleteRootRequest, run_root_query};
use sley_scb1::{encode_record, encode_uvar};

use crate::server::{DEFERRED_DISPATCH_REASON, RESERVED_METHOD_REASON, Server};
use crate::{
    BoundedContext, DecodedFrame, FrameKind, Hello, LimitProfile, MAX_FRAME_BYTES, Method,
    PROTOCOL_VERSION, ProtocolErrorCode, ProtocolFailure, ProtocolFrame, SessionId, decode_frame,
    encode_frame, negotiate,
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
    server: Server,
    session: SessionId,
    next_request: u64,
}

impl Harness {
    fn new(label: &str) -> Self {
        let (temp, _transactions, genesis) =
            genesis(label, complete_bodies(), &[complete_dependency_root()]);
        let repository = temp.child("repo");
        let profile = negotiate(&hello(all_methods(), 4), &hello(all_methods(), 8)).unwrap();
        let mut server = Server::new(&repository, profile).unwrap();
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
fn exchange_export_transports_the_pack_of_a_dependency_free_root() {
    let (temp, _transactions, genesis_id) = genesis("smp1-export", dependency_free_bodies(), &[]);
    let repository = temp.child("repo");
    let profile = negotiate(&hello(all_methods(), 4), &hello(all_methods(), 8)).unwrap();
    let mut server = Server::new(&repository, profile).unwrap();
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
    // Deferred and reserved methods fail with versioned reasons.
    let deferred = harness.fail(Method::Commit, Vec::new());
    assert_eq!(
        deferred.code,
        ProtocolErrorCode::MethodUnsupported.numeric()
    );
    assert_eq!(deferred.details, DEFERRED_DISPATCH_REASON);
    let reserved = harness.fail(Method::HandleExpand, Vec::new());
    assert_eq!(reserved.details, RESERVED_METHOD_REASON);
    // Malformed bodies fail before any engine runs.
    let malformed = harness.fail(Method::RevisionRead, vec![1, 2, 3]);
    assert_eq!(malformed.code, ProtocolErrorCode::PayloadInvalid.numeric());

    // A second server over the same repository answers byte for byte.
    let mut again = Server::new(&harness.repository, harness.server.profile().clone()).unwrap();
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
    assert_eq!(
        frame.body,
        harness.session.as_bytes().to_vec(),
        "session issuance is deterministic"
    );
    for index in 0..128_u64 {
        let request_id = 2 + index;
        let first = again
            .answer(&request_frame(
                Some(harness.session),
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
    // Unknown session.
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
    // Unknown method tag.
    let mut unknown_method = request_frame(Some(harness.session), 50, Method::Report, Vec::new());
    let _ = &mut unknown_method;
    let (failed, frame) = harness.call(Method::Report, Vec::new());
    assert!(failed);
    assert_eq!(
        ProtocolFailure::decode(&frame.body).unwrap().details,
        DEFERRED_DISPATCH_REASON
    );
    // Close, then everything fails closed.
    harness.ok(Method::SessionClose, Vec::new());
    let closed = harness.fail(Method::RevisionRead, tx(genesis));
    assert_eq!(closed.code, ProtocolErrorCode::SessionClosed.numeric());
}
