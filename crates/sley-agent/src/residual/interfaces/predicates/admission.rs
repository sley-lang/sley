//! Mirror the current candidate analysis profile at the authoring boundary.

use crate::{
    error::{AgentError, AgentErrorCode, Result},
    opcodes::OpcodeRow,
};

// sley-policy::candidate_program::CandidateProgram::EXCLUDED_OPERATION_OPCODES.
// This list is not a claim about VM execution support: some of these opcodes
// have a VM implementation but still cannot pass candidate admission. The
// integration tests pin this list to the policy source and exercise an actual
// well-typed contract assertion's kernel refusal through ordinary authoring.
pub(super) const EXCLUDED: [u32; 5] = [144, 145, 160, 161, 162];

pub(in super::super) fn check(row: &OpcodeRow, at: &str) -> Result<()> {
    if EXCLUDED.contains(&row.tag) {
        return Err(AgentError::new(
            AgentErrorCode::ResidualFragmentShape,
            format!(
                "{at}: {} ({}) is excluded by the current candidate operation-analysis profile (CANDIDATE_OPERATION_ANALYSIS_UNSUPPORTED); fragment expansion cannot make it admissible",
                row.name, row.tag
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn exclusion_list_tracks_the_preserved_candidate_policy_profile() {
        // Deliberate drift alarm for the private policy constant. No kernel
        // API or phase changes are required for this workbench diagnostic.
        let policy = include_str!("../../../../../sley-policy/src/candidate_program.rs");
        let declaration = policy
            .split("const EXCLUDED_OPERATION_OPCODES:")
            .nth(1)
            .unwrap()
            .split('=')
            .nth(1)
            .unwrap()
            .split(';')
            .next()
            .unwrap();
        let tags: Vec<u32> = declaration
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .filter(|part| !part.trim().is_empty())
            .map(|part| part.trim().parse().unwrap())
            .collect();
        assert_eq!(tags, super::EXCLUDED);
    }
}
