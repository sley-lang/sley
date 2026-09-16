//! Native boundary tests sharing only the existing semantic fixture builder.
use super::*;
use crate::native_execution::{
    self as native, NativeDeclaredLimits, NativeExecutionError, NativeExecutionInput,
    NativeExecutionProfileError, NativeExecutionRequestV1,
    NativeExecutionTermination as Termination, NativeImplementationLimits,
    NativeResourceKind as Resource,
};

fn request(inputs: Vec<ConstValue>) -> NativeExecutionRequestV1 {
    NativeExecutionRequestV1 {
        inputs,
        profile_id: native::profile_id(),
        declared_limits: NativeDeclaredLimits {
            fuel: 10_000,
            memory_bytes: 1_000_000,
            output_bytes: 1_000_000,
            effect_count: 0,
            call_depth: 256,
            wall_timeout_millis: 1_000,
        },
        implementation_limits: NativeImplementationLimits::HARD_MAXIMA,
    }
}
fn run(
    fixture: &Fixture,
    request: NativeExecutionRequestV1,
) -> Result<native::NativeExecutionOutcome, NativeExecutionError> {
    native::execute_native_function(
        NativeExecutionInput {
            lowering: fixture.input(CacheProfile::EXTENDED_V1),
            effects: &[],
            requirements: &[],
        },
        request,
    )
}
fn negate() -> Fixture {
    Fixture::new(
        &[TypeExpr::Bool],
        &[step(
            Opcode::BoolNot,
            vec![Arg::P(0)],
            Immediate::None,
            TypeExpr::Bool,
        )],
        Vec::new(),
    )
    .with_self()
}

#[test]
fn native_literal_output_bound_and_repeat_identity() {
    let fixture = negate();
    let bytes = u64::try_from(
        sley_mutate::encode_const_value(&boolean(false))
            .unwrap()
            .len(),
    )
    .unwrap();
    for cap in [0, bytes - 1, bytes, bytes + 1, u64::MAX] {
        let mut req = request(vec![boolean(true)]);
        req.declared_limits.output_bytes = cap;
        let result = run(&fixture, req.clone()).unwrap();
        let repeated = run(&fixture, req).unwrap();
        assert_eq!(result, repeated);
        if cap < bytes {
            assert_eq!(
                result.termination(),
                &Termination::ResourceLimit(Resource::OutputBytes)
            );
            assert_eq!(result.observation().output_bytes_counted(), cap + 1);
        } else {
            assert_eq!(result.termination(), &Termination::Success(boolean(false)));
            assert_eq!(result.observation().output_bytes_counted(), bytes);
        }
        assert_eq!(result.observation().peak_call_depth(), 1);
        assert_eq!(result.observation().effect_count(), 0);
    }
}

#[test]
fn native_entry_depth_zero_precedes_initial_value_units_and_fuel() {
    let fixture = negate();
    let mut req = request(vec![boolean(true)]);
    req.declared_limits.call_depth = 0;
    req.declared_limits.fuel = 0;
    req.implementation_limits.max_value_units = 0;
    let result = run(&fixture, req).unwrap();
    assert_eq!(
        result.termination(),
        &Termination::ResourceLimit(Resource::CallDepth)
    );
    assert_eq!(result.observation().peak_call_depth(), 0);
    assert_eq!(result.observation().peak_value_units(), 0);
    assert_eq!(result.observation().fuel_used(), 0);
    assert_eq!(result.observation().output_bytes_counted(), 0);
}

#[test]
fn native_instruction_fuel_and_output_unit_refusals_remain_distinct() {
    let fixture = negate();
    for (case, expected) in [
        (0, Resource::Instructions),
        (1, Resource::Fuel),
        (2, Resource::OutputUnits),
    ] {
        let mut req = request(vec![boolean(true)]);
        req.declared_limits.output_bytes = 0;
        match case {
            0 => req.implementation_limits.max_instructions = 0,
            1 => req.declared_limits.fuel = 0,
            _ => req.implementation_limits.max_output_units = 0,
        }
        let result = run(&fixture, req).unwrap();
        assert_eq!(result.termination(), &Termination::ResourceLimit(expected));
        assert_eq!(result.observation().output_bytes_counted(), 0);
    }
    let mut req = request(vec![boolean(true)]);
    req.declared_limits.fuel = 2;
    let result = run(&fixture, req).unwrap();
    assert_eq!(result.termination(), &Termination::Success(boolean(false)));
    assert_eq!(result.observation().fuel_used(), 2);
}

#[test]
fn native_actual_nested_depth_survives_returns_and_limit_refusals() {
    let mut fixture = depth_chain_fixture(2).with_self();
    fixture.functions.sort_by_key(|f| f.entity_id);
    for (declared, implementation, expected_peak, passes) in
        [(3, 256, 3, true), (2, 256, 2, false), (256, 2, 2, false)]
    {
        let mut req = request(vec![uint(9)]);
        req.declared_limits.call_depth = declared;
        req.implementation_limits.max_call_depth = implementation;
        let result = run(&fixture, req).unwrap();
        assert_eq!(result.observation().peak_call_depth(), expected_peak);
        if passes {
            assert!(matches!(result.termination(), Termination::Success(_)));
        } else {
            assert_eq!(
                result.termination(),
                &Termination::ResourceLimit(Resource::CallDepth)
            );
        }
    }
}

#[test]
fn native_absent_trap_payload_costs_zero_and_present_payload_is_bounded() {
    let mut fixture = negate();
    for payload in [
        None,
        Some(ValueRef::Parameter(fixture.parameters[0].entity_id)),
    ] {
        fixture.blocks[0].terminator = Terminator::Trap(sley_ssmc::TrapTerminator {
            code: sley_ssmc::TrapCode::Unreachable,
            payload,
        });
        let mut req = request(vec![boolean(true)]);
        req.declared_limits.output_bytes = 0;
        let result = run(&fixture, req).unwrap();
        if payload.is_none() {
            assert_eq!(
                result.termination(),
                &Termination::Trap {
                    trap_tag: 1,
                    payload: None
                }
            );
            assert_eq!(result.observation().output_bytes_counted(), 0);
        } else {
            assert_eq!(
                result.termination(),
                &Termination::ResourceLimit(Resource::OutputBytes)
            );
            assert_eq!(result.observation().output_bytes_counted(), 1);
        }
    }
}

#[test]
fn native_context_profile_and_report_reservation_fail_before_execution() {
    let mut fixture = negate();
    let mut req = request(vec![boolean(true)]);
    req.implementation_limits.max_report_bytes = 0;
    assert_eq!(
        run(&fixture, req).unwrap_err(),
        NativeExecutionError::Preserved(ExecutionError::Status(
            crate::ExecutionStatusCode::ResourceLimit
        ))
    );
    let mut req = request(vec![boolean(true)]);
    req.implementation_limits.max_call_depth = 257;
    assert_eq!(
        run(&fixture, req).unwrap_err(),
        NativeExecutionError::Profile(NativeExecutionProfileError::Unsupported)
    );
    fixture.functions.clear();
    assert_eq!(
        run(&fixture, request(vec![boolean(true)])).unwrap_err(),
        NativeExecutionError::Profile(NativeExecutionProfileError::InvalidContext)
    );
    // The missing graph does not replace the earlier integrated input error.
    assert_eq!(
        run(&fixture, request(Vec::new())).unwrap_err(),
        NativeExecutionError::Preserved(ExecutionError::Exec(
            crate::ExecutionErrorCode::InputCountMismatch
        ))
    );
}
