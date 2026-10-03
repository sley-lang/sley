//! Bound draft composition handed to the ordinary revision/test writer.

use super::{
    BaseRef, Global, Head, Input, Origin, Prepared, Proposal, Result, Target, Value, Workspace,
    attach_resolution, authoring, draft, json,
};
use crate::residual::binding::DraftSource;
use crate::workspace::Program;

pub(super) struct Context {
    pub source: DraftSource,
    pub graph: Program,
    pub report: Value,
}

#[allow(clippy::too_many_arguments)] // Keep the invocation's binding and budget.
pub(super) fn prepare(
    global: &Global,
    workspace: &Workspace,
    head: &Head,
    request: &authoring::Request,
    input: Vec<u8>,
    attempt: bool,
    resolution: Option<&authoring::choices::Resolution>,
    budget: &mut authoring::frontier::Budget,
    mut prepared: Prepared,
) -> Result<Proposal> {
    let source_validations =
        global.event.borrow().line(0, "residual", 0)["residual"]["source_kernel_validations"]
            .as_u64()
            .unwrap_or(0);
    // On a preparation error the outer response must not assert that kernel
    // validation never ran. A successful preparation supplies exact counts.
    global.note(
        "residual",
        json!({"try":attempt,"plan":!attempt,
        "draft_graph_preparation":true,"request_bytes":input.len(),"source_kernel_validations":source_validations}),
    );
    let effective = serde_json::to_vec(&request.value()).expect("resolved request");
    let source = DraftSource::capture(workspace, &effective, &prepared.runtime, budget)?;
    let graph = source.prepare(workspace, budget)?;
    prepared.recheck(workspace, head, &input)?;
    let retained = graph.authoring.expect("bound retained authoring");
    let BaseRef::Draft(reference) = &request.base else {
        unreachable!("draft route")
    };
    let revision = reference.revision.expect("explicit revision");
    let mut proposal = Proposal::new(
        "residual-try",
        input,
        Input::Frame(retained.frame.clone()),
        Target::Next {
            handle: reference.handle.clone(),
            parent: revision,
            latest: true,
        },
        Origin::Draft,
    );
    proposal.on = Some(draft::spell(&reference.handle, revision));
    proposal.inherit(source.status());
    let entries = provenance(
        &graph.expansion,
        &retained,
        &source,
        proposal.on.as_deref().expect("source revision"),
    );
    proposal.artifacts = vec![
        (
            "residual-request.json".into(),
            authoring::strict_json(&proposal.input)?,
        ),
        ("residual-binding.json".into(), prepared.binding.to_json()),
        (
            "residual-expanded.json".into(),
            graph.expansion.frame.clone(),
        ),
        (
            "residual-provenance.json".into(),
            json!({"frame":"frame.json","entries":entries,"scope":request.scope}),
        ),
        (
            "residual-sparse-provenance.json".into(),
            json!({"frame":"residual-expanded.json","entries":graph.expansion.provenance}),
        ),
        ("residual-retained-frame.json".into(), retained.frame),
        ("residual-authoring-changes.json".into(), retained.changes),
        (
            "residual-inherited-constants.json".into(),
            json!({"revision":proposal.on,"binding":source.binding().digest(),
                "candidate_sha256":source.status()["candidate_sha256"],
                "entries":retained.inherited_constants}),
        ),
        (
            "residual-draft-source.json".into(),
            source.binding().to_json(),
        ),
        ("residual-draft-graph.json".into(), graph.report.clone()),
        ("residual-edit.json".into(), graph.contract.report()),
    ];
    global.note("residual", json!({"try":attempt,"plan":!attempt,"draft_graph_preparation":true,
        "request_bytes":proposal.input.len(),"expanded_bytes":serde_json::to_vec(&graph.expansion.frame).expect("JSON").len(),
        "provenance_entries":entries.len(),
        "inherited_constant_declarations":graph.report["inherited_constant_declarations"],
        "kernel_validations":2,"source_kernel_validations":source_validations}));
    prepared.note_outcome(json!({"construction":"complete","kernel":"valid",
        "public_checks":"not_run","native_admission":"not_attempted"}));
    prepared.edit = Some(graph.contract);
    prepared.draft = Some(Context {
        source,
        graph: graph.source,
        report: graph.report,
    });
    proposal.staged = Some((graph.compiled, graph.candidate, graph.output));
    proposal.residual = Some(prepared);
    if let Some(resolution) = resolution {
        attach_resolution(&mut proposal, resolution)?;
    }
    budget.checkpoint()?;
    Ok(proposal)
}

fn provenance(
    expansion: &authoring::fragments::Expansion,
    retained: &authoring::edit::draft::authoring::Retained,
    source: &DraftSource,
    on: &str,
) -> serde_json::Map<String, Value> {
    let mut entries = serde_json::Map::new();
    entries.insert(
        String::new(),
        json!({"class":"INHERITED","revision":on,
        "binding":source.binding().digest(),"document":"frame.json"}),
    );
    for constant in &retained.inherited_constants {
        let mut origin = constant.clone();
        origin["revision"] = json!(on);
        origin["binding"] = json!(source.binding().digest());
        entries.insert(
            constant["authored"]
                .as_str()
                .expect("inherited declaration")
                .to_owned(),
            origin,
        );
    }
    for (index, change) in retained
        .changes
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let pointer = change["authored"].as_str().expect("authored literal");
        let pointer = if retained
            .frame
            .pointer(pointer)
            .is_some_and(|v| v.get("value").is_some())
        {
            entries.insert(
                pointer.to_owned(),
                expansion
                    .provenance
                    .get(&format!("/edit/{index}"))
                    .expect("inherited literal load")
                    .clone(),
            );
            format!("{pointer}/value")
        } else {
            pointer.to_owned()
        };
        let origin = expansion
            .provenance
            .get(&format!("/edit/{index}/with/1/value"))
            .expect("literal decision origin");
        entries.insert(pointer, origin.clone());
    }
    entries
}
