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
    let deferred = harness.fail(Method::GcDryRun, Vec::new());
    assert_eq!(
        deferred.code,
        ProtocolErrorCode::MethodUnsupported.numeric()
    );
    assert_eq!(deferred.details, DEFERRED_DISPATCH_REASON);
    let reserved = harness.fail(Method::Diagnostics, Vec::new());
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
    let mut other = Server::new(&fresh_root, harness.server.profile().clone()).unwrap();
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
    let mut source = Server::new(&source_root, harness.server.profile().clone()).unwrap();
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
    let mut target = Server::new(&target_root, harness.server.profile().clone()).unwrap();
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
    // Deferred methods still answer with the versioned reason.
    let deferred = harness.fail(Method::Execute, Vec::new());
    assert_eq!(deferred.details, DEFERRED_DISPATCH_REASON);
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
    let mut streaming = Server::new(&harness.repository, profile.clone()).unwrap();
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
    let profile = negotiate(&no_stream_client, &small_server).unwrap();
    let mut refusing = Server::new(&harness.repository, profile).unwrap();
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
    let profile = negotiate(&tiny_client, &hello(all_methods(), 8)).unwrap();
    let mut budgeted = Server::new(&harness.repository, profile).unwrap();
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
fn sessions_bind_workspace_root_and_epoch_and_handles_die_with_the_root() {
    use sley_repo::test_support::{TempDir, genesis_in_workspace};
    let mut harness = Harness::new("smp1-330");
    let genesis_id = harness.genesis;
    let session = harness.session;
    // Deterministic issuance: a second server over the same state issues
    // the same identity, derived under sley2.session.v1.
    let mut twin = Server::new(&harness.repository, harness.server.profile().clone()).unwrap();
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
    assert_eq!(frame.body, session.as_bytes().to_vec());
    // A positional handle expands under the bound root.
    let expanded = harness.ok(Method::HandleExpand, encode_uvar(1));
    assert_eq!(expanded.bounds.returned_entities, 1);
    assert!(
        expanded
            .body
            .windows(32)
            .any(|window| window == session.as_bytes())
    );
    let unknown = harness.fail(Method::HandleExpand, encode_uvar(7));
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
    let stale = harness.fail(Method::HandleExpand, encode_uvar(1));
    assert_eq!(stale.symbol, "SESSION_STALE_HANDLE");
    assert_eq!(stale.code, 33_004);
    let advanced = harness.fail(Method::WorkspaceOpen, Vec::new());
    assert_eq!(advanced.symbol, "SESSION_ROOT_ADVANCED");
    // Methods naming explicit transactions are not head-bound.
    let explicit = harness.fail(Method::RevisionRead, tx(genesis_id));
    assert!(
        !explicit.symbol.starts_with("SESSION_"),
        "{}",
        explicit.symbol
    );
    // Renewal rebinds to the new head and handles resolve again.
    let renewed = harness.ok(Method::SessionRenew, session.as_bytes().to_vec());
    assert_eq!(renewed.body, session.as_bytes().to_vec());
    let fresh = harness.ok(Method::HandleExpand, encode_uvar(0));
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
    let mismatch = harness.fail(Method::HandleExpand, encode_uvar(0));
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
    // Unknown sessions are refused as sessions, not as frames.
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
    assert_eq!(
        failure.code,
        ProtocolErrorCode::RequestIdConflict.numeric(),
        "the registry refuses first"
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
    assert_eq!(offered.methods.len(), 33);
    for method in Method::ALL {
        let offered_it = offered.methods.contains(&method.tag());
        assert_eq!(
            offered_it,
            !method.is_reserved() && !Server::is_deferred(method),
            "{method:?}"
        );
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
                failure.details != DEFERRED_DISPATCH_REASON
                    && failure.details != RESERVED_METHOD_REASON,
                "{method:?} is offered but not dispatched"
            );
        }
    }
    let selected = negotiate(&offered, &offered).unwrap();
    assert_eq!(selected.methods, offered.methods);
}
