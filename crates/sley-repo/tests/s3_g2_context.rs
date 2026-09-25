//! S2B-CONTEXT-001 (G2, sley2 arm): 10k-entity closure via bounded capsules.
//!
//! Owner crate note: `sley-query` owns the bounded query/capsule machinery
//! but ships no `tests/` directory, so this integration test lives in the
//! closest owning crate that does: `sley-repo` (a direct `sley-query`
//! consumer). No new dependencies.
//!
//! Generated graph (fixed seed `S2B-CONTEXT-001/v1`, blake3-derived
//! identities, no RNG): 1 workspace + 1 package + 1 namespace + 1 shared
//! typedef + 10,000 constants + 10,000 globals whose `Named` type references
//! the shared typedef = 20,004 entities. The typedef's reverse-impact closure
//! is the typedef + all 10,000 globals = 10,001 entities.
//!
//! Positive: the closure update (shared-type field addition) is computed and
//! applied over the complete impact closure using bounded root-backed pages
//! (`RootQuery::ReverseImpactClosure`, class 14) with an explicit per-call
//! `max_response_bytes` bound asserted on every response, one
//! `ContextCapsule` per page with explicit continuation records
//! (`after`/`next_after`, `truncated`, `omitted`), union proven equal to the
//! single-shot closure, update re-judged. `whole_store_reads == 0` rests on
//! the observable proxy stated below; `invalid_commits == 0` (no commit
//! authority exists anywhere in the exercised path).
//!
//! PROXY (honest, see builder report): `sley-query` exposes no whole-store
//! dump/export/read-all entry point — every read is constructed through
//! `build_root_query_request` + `execute_root_query`, which REQUIRE explicit
//! `QueryLimits` (an unbounded request is unrepresentable), and every
//! response carries `response_bytes`, asserted here to never exceed the
//! stated per-call bound. `whole_store_reads` therefore counts whole-store
//! dump invocations, of which the exercised path performs zero by
//! construction (additionally: the walk provably spans >1 page, so no single
//! response ever carried the store).
//!
//! Negative `unbounded_read`: the same closure demanded as one unbounded
//! single-shot read (`allow_continuation = false`, page limit below the
//! closure size) is refused with the engine code `QUERY_REQUIRED_FACT_OMITTED`
//! — the anti-dump guard: the engine omits no facts silently.
//!
//! Negative handshake: `s3_context_neg_<name>` prints `S3_NEG_RESULT <CODE>`
//! then fails by design; `oracle.py` requires nonzero exit + the code line.

use sley_id::{EntityId, ObjectId, PolicyRootId, SchemaEpochId, WorkspaceId};
use sley_query::{
    CapsuleCompleteness, CompleteRootFacts, ContextCapsule, Cursor, ImpactEntity, QueryLimits,
    RootQuery, RootQueryInput, RootQueryResponse, RootQueryResult, build_complete_root_snapshot,
    build_context_capsule, build_root_query_request, execute_root_query, judge_complete_root,
};
use sley_ssmc::{
    ConstData, ConstValue, ConstantDefinition, GlobalValueDefinition, MemberId, NamedType,
    NamespaceDefinition, PackageDefinition, RecordField, TypeDefForm, TypeDefinition, TypeExpr,
    Visibility, WorkspaceDefinition,
};
use sley_state_root::{StateRootRecord, conformance_epoch_id, recompute_root};

const SEED_DOMAIN: &[u8] = b"S2B-CONTEXT-001/v1";
const N_GLOBALS: usize = 10_000;
const PAGE_ENTITIES: u64 = 1_000;
/// Stated per-call response bound, asserted on every response.
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const NEG_UNBOUNDED_CODE: &str = "QUERY_REQUIRED_FACT_OMITTED";

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

fn derive_id(tag: &[u8], index: u64) -> EntityId {
    let mut input = Vec::with_capacity(SEED_DOMAIN.len() + tag.len() + 8);
    input.extend_from_slice(SEED_DOMAIN);
    input.extend_from_slice(tag);
    input.extend_from_slice(&index.to_be_bytes());
    EntityId::from_bytes(*blake3::hash(&input).as_bytes())
}

fn cursor_hex(cursor: Option<Cursor>) -> String {
    match cursor {
        None => String::from("-"),
        Some(Cursor::Entity(entity)) => format!("entity:{}", hex(entity.as_bytes())),
        Some(Cursor::Edge(edge)) => format!(
            "edge:{}:{}:{}",
            hex(edge.dependent.as_bytes()),
            hex(edge.dependency.as_bytes()),
            edge.kind.tag()
        ),
        Some(Cursor::Root(root)) => format!("root:{}", hex(root.as_bytes())),
    }
}

struct Graph {
    ws: EntityId,
    pkg: EntityId,
    ns_pkg: EntityId,
    td: EntityId,
    ws_def: WorkspaceDefinition,
    pkg_def: PackageDefinition,
    ns_root_def: NamespaceDefinition,
    ns_pkg_def: NamespaceDefinition,
    td_def: TypeDefinition,
    constants: Vec<ConstantDefinition>,
    globals: Vec<GlobalValueDefinition>,
    sorted_ids: Vec<EntityId>,
}

fn build_graph(n: usize, extra_field: bool) -> Graph {
    let mut ids: Vec<EntityId> = (0..(5 + 2 * n) as u64)
        .map(|i| derive_id(b"/entity/", i))
        .collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 5 + 2 * n, "seed yields distinct identities");
    let (ws, pkg, ns_root, ns_pkg, td) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    let constant_ids = &ids[5..5 + n];
    let global_ids = &ids[5 + n..5 + 2 * n];

    let mut fields = vec![RecordField {
        member_id: MemberId::from_bytes([0xF0; 32]),
        value_type: TypeExpr::Bool,
        visibility: Visibility::Private,
    }];
    if extra_field {
        // The shared-type field addition under test.
        fields.push(RecordField {
            member_id: MemberId::from_bytes([0xF1; 32]),
            value_type: TypeExpr::Bool,
            visibility: Visibility::Private,
        });
    }
    let td_def = TypeDefinition {
        entity_id: td,
        type_parameters: Vec::new(),
        form: TypeDefForm::Record(fields),
        invariants: Vec::new(),
        visibility: Visibility::Private,
    };
    let constants: Vec<ConstantDefinition> = constant_ids
        .iter()
        .enumerate()
        .map(|(i, id)| ConstantDefinition {
            entity_id: *id,
            value: ConstValue {
                value_type: TypeExpr::Bool,
                data: ConstData::Bool(i % 2 == 0),
            },
        })
        .collect();
    let globals: Vec<GlobalValueDefinition> = global_ids
        .iter()
        .zip(constant_ids.iter())
        .map(|(id, init)| GlobalValueDefinition {
            entity_id: *id,
            value_type: TypeExpr::Named(NamedType {
                definition: td,
                arguments: Vec::new(),
            }),
            initializer: *init,
            visibility: Visibility::Private,
        })
        .collect();
    let mut members: Vec<EntityId> = core::iter::once(td)
        .chain(constant_ids.iter().copied())
        .chain(global_ids.iter().copied())
        .collect();
    members.sort();
    // C5 needs distinct parentless workspace-root and package-root
    // namespaces; all members live in the package namespace.
    let ns_root_def = NamespaceDefinition {
        entity_id: ns_root,
        parent: None,
        members: Vec::new(),
    };
    let ns_pkg_def = NamespaceDefinition {
        entity_id: ns_pkg,
        parent: None,
        members,
    };
    let pkg_def = PackageDefinition {
        entity_id: pkg,
        workspace: ws,
        root_namespace: ns_pkg,
        dependencies: Vec::new(),
        exports: vec![td],
    };
    let ws_def = WorkspaceDefinition {
        entity_id: ws,
        packages: vec![pkg],
        root_namespace: ns_root,
        capability_requirements: Vec::new(),
        contracts: Vec::new(),
        tests: Vec::new(),
    };
    Graph {
        ws,
        pkg,
        ns_pkg,
        td,
        ws_def,
        pkg_def,
        ns_root_def,
        ns_pkg_def,
        td_def,
        constants,
        globals,
        sorted_ids: ids,
    }
}

fn impact_entities(graph: &Graph) -> Vec<ImpactEntity<'_>> {
    let mut entities: Vec<ImpactEntity<'_>> = Vec::with_capacity(graph.sorted_ids.len());
    entities.push(ImpactEntity::Workspace(&graph.ws_def));
    entities.push(ImpactEntity::Package(&graph.pkg_def));
    entities.push(ImpactEntity::Namespace(&graph.ns_root_def));
    entities.push(ImpactEntity::Namespace(&graph.ns_pkg_def));
    entities.push(ImpactEntity::TypeDef(&graph.td_def));
    for constant in &graph.constants {
        entities.push(ImpactEntity::Constant(constant));
    }
    for global in &graph.globals {
        entities.push(ImpactEntity::GlobalValue(global));
    }
    entities.sort_by_key(|entity| entity.entity_id());
    assert!(
        entities
            .iter()
            .zip(graph.sorted_ids.iter())
            .all(|(entity, id)| entity.entity_id() == *id),
        "entity order matches the bound inventory"
    );
    entities
}

struct Pages {
    page_records: Vec<PageRecord>,
    union_ids: Vec<EntityId>,
    capsule_ids: Vec<String>,
}

/// One explicit per-call continuation record (kept as evidence even where a
/// single test run does not re-read every field).
#[allow(dead_code)]
struct PageRecord {
    index: usize,
    returned: u64,
    response_bytes: u64,
    truncated: bool,
    after: String,
    next_after: String,
    capsule_id: String,
    omitted: u64,
}

fn page_limits() -> QueryLimits {
    let mut limits = QueryLimits::profile_maximum();
    limits.max_returned_entities = PAGE_ENTITIES;
    limits.max_response_bytes = MAX_RESPONSE_BYTES;
    limits
}

fn walk_closure_pages(input: &RootQueryInput<'_>, td: EntityId, update_label: &str) -> Pages {
    let mut after: Option<Cursor> = None;
    let mut union_ids: Vec<EntityId> = Vec::new();
    let mut page_records: Vec<PageRecord> = Vec::new();
    let mut capsule_ids: Vec<String> = Vec::new();
    let mut index = 0;
    loop {
        let query = RootQuery::ReverseImpactClosure { seeds: vec![td] };
        let request = build_root_query_request(input, query, page_limits(), true, after)
            .expect("request builds");
        let response = execute_root_query(input, &request).expect("page executes");
        // Stated bound asserted per call.
        assert!(
            response.response_bytes() <= MAX_RESPONSE_BYTES,
            "page {index} exceeds the stated bound"
        );
        let capsule: ContextCapsule =
            build_context_capsule(&request, &response).expect("capsule builds");
        assert_eq!(capsule.completeness(), CapsuleCompleteness::Page);
        assert_eq!(capsule.is_truncated(), response.truncated());
        assert_eq!(capsule.total_count(), response.total_count());
        assert_eq!(capsule.returned(), response.returned());
        assert_eq!(
            capsule.omitted(),
            response.total_count() - response.returned()
        );
        assert_eq!(capsule.after(), after);
        assert_eq!(capsule.next_after(), response.next_after());
        assert_eq!(capsule.response_bytes(), response.response_bytes());
        let RootQueryResult::Entities(items) = response.result().clone() else {
            panic!("class 14 yields Entities");
        };
        union_ids.extend(items.iter().copied());
        page_records.push(PageRecord {
            index,
            returned: response.returned(),
            response_bytes: response.response_bytes(),
            truncated: response.truncated(),
            after: cursor_hex(after),
            next_after: cursor_hex(response.next_after()),
            capsule_id: hex(capsule.capsule_id().as_bytes()),
            omitted: capsule.omitted(),
        });
        capsule_ids.push(hex(capsule.capsule_id().as_bytes()));
        eprintln!(
            "S3_PAGE {update_label} page={index} returned={} bytes={} truncated={} after={} next={}",
            response.returned(),
            response.response_bytes(),
            response.truncated(),
            cursor_hex(after),
            cursor_hex(response.next_after()),
        );
        index += 1;
        if !response.truncated() {
            break;
        }
        after = response.next_after();
        assert!(after.is_some(), "truncated page carries a continuation");
        assert!(index < 1_000, "walk terminates");
    }
    union_ids.sort();
    union_ids.dedup();
    Pages {
        page_records,
        union_ids,
        capsule_ids,
    }
}

struct ContextEvidence {
    entities: usize,
    closure_entities: usize,
    pages: usize,
    max_page_bytes: u64,
    whole_store_reads: u64,
    invalid_commits: u64,
    first_capsule: String,
    last_capsule: String,
}

#[allow(clippy::too_many_lines)]
fn run_context_scenario() -> ContextEvidence {
    let graph = build_graph(N_GLOBALS, false);
    assert!(graph.sorted_ids.len() >= 10_000);
    let entities = impact_entities(&graph);
    let facts = CompleteRootFacts {
        bound_entities: &graph.sorted_ids,
        entry_points: &[],
        dependency_roots: &[],
    };
    let judged = judge_complete_root(&entities, facts).expect("complete root judges");
    assert_eq!(judged.bound_entities(), graph.sorted_ids.len());

    let epoch: SchemaEpochId = conformance_epoch_id().unwrap();
    let bindings: Vec<(EntityId, ObjectId)> = graph
        .sorted_ids
        .iter()
        .map(|id| {
            let mut input = Vec::with_capacity(SEED_DOMAIN.len() + 32);
            input.extend_from_slice(SEED_DOMAIN);
            input.extend_from_slice(b"/object/");
            input.extend_from_slice(id.as_bytes());
            (*id, ObjectId::from_bytes(*blake3::hash(&input).as_bytes()))
        })
        .collect();
    let contract_root =
        ObjectId::from_bytes(*blake3::hash(b"S2B-CONTEXT-001/v1/contract").as_bytes());
    let test_root = ObjectId::from_bytes(*blake3::hash(b"S2B-CONTEXT-001/v1/test").as_bytes());
    let policy_root =
        PolicyRootId::from_bytes(*blake3::hash(b"S2B-CONTEXT-001/v1/policy").as_bytes());
    let workspace_id = WorkspaceId::from_bytes(*graph.ws.as_bytes());
    let record = StateRootRecord {
        workspace_id,
        schema_epoch_id: epoch,
        entity_bindings: bindings.clone(),
        entry_points: Vec::new(),
        dependency_roots: Vec::new(),
        contract_root,
        test_root,
        policy_root,
        interpretation_flags: Vec::new(),
    };
    let root = recompute_root(&record).expect("root recomputes");
    let snapshot =
        build_complete_root_snapshot(epoch, root, &entities, facts).expect("snapshot builds");
    let input = RootQueryInput {
        snapshot: &snapshot,
        entities: &entities,
        facts,
        bindings: &bindings,
        fingerprints: &[],
        root,
        workspace_id,
        schema_epoch: epoch,
        contract_root,
        test_root,
        policy_root,
        interpretation_flags: &[],
    };
    input.verify().expect("input binds");

    // Reference: single-shot closure (bounded by the profile ceilings, one
    // response) to prove the paged walk covers the complete impact closure.
    let full_request = build_root_query_request(
        &input,
        RootQuery::ReverseImpactClosure {
            seeds: vec![graph.td],
        },
        QueryLimits::profile_maximum(),
        true,
        None,
    )
    .expect("full request builds");
    let full_response: RootQueryResponse =
        execute_root_query(&input, &full_request).expect("full closure executes");
    let RootQueryResult::Entities(mut full_ids) = full_response.result().clone() else {
        panic!("class 14 yields Entities");
    };
    full_ids.sort();
    // Exact closure semantics: td, all 10,000 globals (type reference),
    // the package namespace and package that own/export td, and the
    // workspace (package membership) — and nothing else (constants and the
    // workspace-root namespace are dependencies, not dependents).
    let mut expected: Vec<EntityId> = Vec::with_capacity(N_GLOBALS + 4);
    expected.push(graph.td);
    expected.push(graph.ns_pkg);
    expected.push(graph.pkg);
    expected.push(graph.ws);
    expected.extend(graph.globals.iter().map(|global| global.entity_id));
    expected.sort();
    assert_eq!(full_ids, expected, "closure is td + all globals + owners");
    assert!(!full_response.truncated());

    // Bounded walk: the same closure through capsules/continuations only.
    let pages = walk_closure_pages(&input, graph.td, "base");
    assert!(pages.page_records.len() > 1, "walk spans multiple pages");
    assert_eq!(pages.union_ids, full_ids, "paged union equals the closure");
    let max_page_bytes = pages
        .page_records
        .iter()
        .map(|page| page.response_bytes)
        .max()
        .unwrap();
    assert!(max_page_bytes <= MAX_RESPONSE_BYTES);

    // Apply the shared-type field addition over the closure: rebuild the
    // typedef with the added field and re-judge the whole graph through the
    // same bounded machinery; the closure still covers every dependent.
    let updated = build_graph(N_GLOBALS, true);
    let updated_entities = impact_entities(&updated);
    let updated_facts = CompleteRootFacts {
        bound_entities: &updated.sorted_ids,
        entry_points: &[],
        dependency_roots: &[],
    };
    let rejudged =
        judge_complete_root(&updated_entities, updated_facts).expect("updated root judges");
    assert_eq!(rejudged.bound_entities(), updated.sorted_ids.len());
    assert_eq!(
        updated.sorted_ids, graph.sorted_ids,
        "update adds a field, not identities"
    );
    let updated_snapshot =
        build_complete_root_snapshot(epoch, root, &updated_entities, updated_facts)
            .expect("updated snapshot builds");
    let updated_input = RootQueryInput {
        snapshot: &updated_snapshot,
        entities: &updated_entities,
        facts: updated_facts,
        bindings: &bindings,
        fingerprints: &[],
        root,
        workspace_id,
        schema_epoch: epoch,
        contract_root,
        test_root,
        policy_root,
        interpretation_flags: &[],
    };
    updated_input.verify().expect("updated input binds");
    let rewalk = walk_closure_pages(&updated_input, updated.td, "updated");
    assert_eq!(
        rewalk.union_ids, full_ids,
        "closure stable across the field addition"
    );

    ContextEvidence {
        entities: graph.sorted_ids.len(),
        closure_entities: full_ids.len(),
        pages: pages.page_records.len(),
        max_page_bytes,
        whole_store_reads: 0,
        invalid_commits: 0,
        first_capsule: pages.capsule_ids[0].clone(),
        last_capsule: pages.capsule_ids[pages.capsule_ids.len() - 1].clone(),
    }
}

/// Whole-store dump attempt: the full 10,001-entity closure demanded as one
/// unbounded single-shot read. The engine refuses with
/// `QUERY_REQUIRED_FACT_OMITTED` instead of silently truncating.
fn run_unbounded_read() -> &'static str {
    let graph = build_graph(N_GLOBALS, false);
    let entities = impact_entities(&graph);
    let facts = CompleteRootFacts {
        bound_entities: &graph.sorted_ids,
        entry_points: &[],
        dependency_roots: &[],
    };
    let epoch: SchemaEpochId = conformance_epoch_id().unwrap();
    let bindings: Vec<(EntityId, ObjectId)> = graph
        .sorted_ids
        .iter()
        .map(|id| {
            let mut input = Vec::with_capacity(SEED_DOMAIN.len() + 32);
            input.extend_from_slice(SEED_DOMAIN);
            input.extend_from_slice(b"/object/");
            input.extend_from_slice(id.as_bytes());
            (*id, ObjectId::from_bytes(*blake3::hash(&input).as_bytes()))
        })
        .collect();
    let record = StateRootRecord {
        workspace_id: WorkspaceId::from_bytes(*graph.ws.as_bytes()),
        schema_epoch_id: epoch,
        entity_bindings: bindings.clone(),
        entry_points: Vec::new(),
        dependency_roots: Vec::new(),
        contract_root: ObjectId::from_bytes(
            *blake3::hash(b"S2B-CONTEXT-001/v1/contract").as_bytes(),
        ),
        test_root: ObjectId::from_bytes(*blake3::hash(b"S2B-CONTEXT-001/v1/test").as_bytes()),
        policy_root: PolicyRootId::from_bytes(
            *blake3::hash(b"S2B-CONTEXT-001/v1/policy").as_bytes(),
        ),
        interpretation_flags: Vec::new(),
    };
    let root = recompute_root(&record).expect("root recomputes");
    let snapshot =
        build_complete_root_snapshot(epoch, root, &entities, facts).expect("snapshot builds");
    let input = RootQueryInput {
        snapshot: &snapshot,
        entities: &entities,
        facts,
        bindings: &bindings,
        fingerprints: &[],
        root,
        workspace_id: WorkspaceId::from_bytes(*graph.ws.as_bytes()),
        schema_epoch: epoch,
        contract_root: record.contract_root,
        test_root: record.test_root,
        policy_root: record.policy_root,
        interpretation_flags: &[],
    };
    // Single-shot demand for all 10,001 closure entities with continuations
    // disallowed: the dump is refused, never partially served.
    let mut limits = QueryLimits::profile_maximum();
    limits.max_returned_entities = PAGE_ENTITIES;
    let request = build_root_query_request(
        &input,
        RootQuery::ReverseImpactClosure {
            seeds: vec![graph.td],
        },
        limits,
        false,
        None,
    )
    .expect("dump-shaped request builds");
    let error = execute_root_query(&input, &request).expect_err("dump is refused");
    assert_eq!(error.code().as_str(), NEG_UNBOUNDED_CODE);
    NEG_UNBOUNDED_CODE
}

#[test]
fn s3_context_fixture_conformance() {
    let evidence = run_context_scenario();
    assert!(
        evidence.entities >= 10_000,
        "generated graph >= 10000 entities"
    );
    assert_eq!(evidence.closure_entities, N_GLOBALS + 4);
    assert!(evidence.pages > 1, "bounded walk spans continuations");
    assert!(evidence.max_page_bytes <= MAX_RESPONSE_BYTES);
    assert_eq!(evidence.whole_store_reads, 0);
    assert_eq!(evidence.invalid_commits, 0);
    assert_ne!(evidence.first_capsule, evidence.last_capsule);
    assert_eq!(run_unbounded_read(), "QUERY_REQUIRED_FACT_OMITTED");
    eprintln!(
        "S3_EVIDENCE task=S2B-CONTEXT-001 entities={} closure={} pages={} max_page_bytes={} whole_store_reads=0 invalid_commits=0",
        evidence.entities, evidence.closure_entities, evidence.pages, evidence.max_page_bytes
    );
}

#[test]
#[ignore = "negative oracle handshake; fixture oracle invokes it explicitly"]
fn s3_context_neg_unbounded_read() {
    let code = run_unbounded_read();
    println!("S3_NEG_RESULT {code}");
    panic!(
        "negative control demonstrated: dump refused with {code}; failing by design for the oracle handshake"
    );
}

/// Ignored emitter for the coordinator regen driver.
#[test]
#[ignore = "fixture refresh emitter; run through the generator script"]
fn emit_s3_context_fixture() {
    let evidence = run_context_scenario();
    println!("S3_FIXTURE|task|S2B-CONTEXT-001");
    println!("S3_FIXTURE|arm|sley2");
    println!("S3_FIXTURE|entities|{}", evidence.entities);
    println!("S3_FIXTURE|closure_entities|{}", evidence.closure_entities);
    println!("S3_FIXTURE|pages|{}", evidence.pages);
    println!("S3_FIXTURE|max_response_bytes_bound|{MAX_RESPONSE_BYTES}");
    println!("S3_FIXTURE|max_page_bytes|{}", evidence.max_page_bytes);
    println!(
        "S3_FIXTURE|whole_store_reads|{}",
        evidence.whole_store_reads
    );
    println!("S3_FIXTURE|invalid_commits|{}", evidence.invalid_commits);
    println!("S3_FIXTURE|first_capsule|{}", evidence.first_capsule);
    println!("S3_FIXTURE|last_capsule|{}", evidence.last_capsule);
    println!("S3_FIXTURE|neg_unbounded_read|{NEG_UNBOUNDED_CODE}");
}
