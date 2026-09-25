//! Reconstructs effect-owner units from the same inventory lowering consumes.
//!
//! Derived [`FunctionUnit`] views follow the supplied `functions` order, matching
//! the existing policy/contracts owners which also forward program order to
//! `validate_effect_program`. That checker enforces raw-ID-sorted units with its
//! exact error; this module does not silently re-sort derived views, so an
//! unsorted inventory surfaces the owner's error rather than a normalized pass.
use super::{
    NativeExecutionError, NativeExecutionInput, NativeExecutionProfileError, profile_error,
};
use crate::LoweringInput;
use sley_check::effects::{self, FunctionUnit};
use sley_id::EntityId;
use sley_ssmc::{Block, Operation, Parameter, ParameterRole};
use std::collections::BTreeMap;

#[derive(Default)]
struct OwnedUnit {
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
}
fn invalid() -> NativeExecutionError {
    profile_error(NativeExecutionProfileError::InvalidContext)
}

pub(super) fn validate(input: &NativeExecutionInput<'_>) -> Result<(), NativeExecutionError> {
    let source = &input.lowering;
    check_budgets(source)?;
    let owned = group_owned(source)?;
    let units: Vec<_> = source
        .functions
        .iter()
        .map(|function| {
            let members = &owned[&function.entity_id];
            FunctionUnit {
                function,
                parameters: &members.parameters,
                blocks: &members.blocks,
                operations: &members.operations,
            }
        })
        .collect();
    let mut contracts: Vec<_> = source.contracts.iter().map(|c| c.entity_id).collect();
    contracts.sort_unstable();
    let report = effects::validate_effect_program(
        source.types,
        &units,
        input.effects,
        input.requirements,
        source.adapters,
        &contracts,
    )
    .map_err(NativeExecutionError::Effect)?;
    let pure = |function| -> Result<(), NativeExecutionError> {
        let closure = report
            .functions
            .iter()
            .find(|f| f.function == function)
            .ok_or_else(invalid)?;
        if closure.effects.is_empty() {
            Ok(())
        } else {
            Err(profile_error(NativeExecutionProfileError::Unsupported))
        }
    };
    pure(source.function.entity_id)?;
    for contract in source.contracts {
        pure(contract.predicate)?;
    }
    Ok(())
}

fn check_budgets(source: &LoweringInput<'_>) -> Result<(), NativeExecutionError> {
    // Bound grouping work and allocation before cloning inventory members.
    let entities = source
        .parameters
        .len()
        .checked_add(source.blocks.len())
        .and_then(|n| n.checked_add(source.operations.len()));
    if source.functions.len() > effects::MAX_EFFECT_FUNCTIONS
        || entities.is_none_or(|n| n > effects::MAX_EFFECT_GRAPH_ENTITIES)
        || source.operations.len() > effects::MAX_EFFECT_OPERATIONS
    {
        return Err(NativeExecutionError::Effect(
            effects::EffectValidationError::Effect(effects::EffectError::new(
                effects::EffectErrorCode::ResourceLimit,
            )),
        ));
    }
    Ok(())
}

type OwnedGroups = BTreeMap<EntityId, OwnedUnit>;

fn group_owned(source: &LoweringInput<'_>) -> Result<OwnedGroups, NativeExecutionError> {
    let mut owned = BTreeMap::<EntityId, OwnedUnit>::new();
    for function in source.functions {
        if owned
            .insert(function.entity_id, OwnedUnit::default())
            .is_some()
        {
            return Err(invalid());
        }
    }
    if source
        .functions
        .iter()
        .find(|f| f.entity_id == source.function.entity_id)
        != Some(source.function)
    {
        return Err(invalid());
    }
    let mut block_owners = BTreeMap::new();
    for block in source.blocks {
        if block_owners
            .insert(block.entity_id, block.function)
            .is_some()
        {
            return Err(invalid());
        }
        owned
            .get_mut(&block.function)
            .ok_or_else(invalid)?
            .blocks
            .push(block.clone());
    }
    for parameter in source.parameters {
        let function = match parameter.role {
            ParameterRole::Function => parameter.owner,
            ParameterRole::Block => *block_owners.get(&parameter.owner).ok_or_else(invalid)?,
        };
        owned
            .get_mut(&function)
            .ok_or_else(invalid)?
            .parameters
            .push(parameter.clone());
    }
    for operation in source.operations {
        let function = block_owners.get(&operation.block).ok_or_else(invalid)?;
        owned
            .get_mut(function)
            .ok_or_else(invalid)?
            .operations
            .push(operation.clone());
    }
    Ok(owned)
}
