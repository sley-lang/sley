//! Host-facing handoff to the pure native test owner.
//!
//! The portable frame is checked here. Sley graph validation, `TestCase`
//! admission, and VM execution belong to `sley-tests::source_execution`.
//! Neither layer supplies supervisor measurement or commit authority.

use sley_tests::NativeExecutionReportV1;
use sley_tests::source_execution::{
    NativeTestEvaluation, NativeTestSourceInput, evaluate_native_test_source,
    execute_native_test_source, report_native_test_source,
};
use sley_vm::native_execution::NativeExecutionOutcome;

use crate::{program::PortableTestProgram, worker::WorkerRequest};

pub use sley_tests::source_execution::NativeTestSourceError as PortableExecutionError;

fn source_input<'a>(
    program: &'a PortableTestProgram,
    worker: &'a WorkerRequest,
) -> Result<NativeTestSourceInput<'a>, PortableExecutionError> {
    if worker.program_bytes != program.stored_bytes()
        || worker.declared_limits != program.selected().declared_limits
        || worker.implementation_limits != program.plan().implementation_limits()
    {
        return Err(PortableExecutionError::SourceMismatch);
    }
    Ok(NativeTestSourceInput {
        root: program.root(),
        objects: program.objects(),
        selected: program.selected(),
        implementation_limits: worker.implementation_limits,
        input_hashes: &worker.input_hashes,
    })
}

/// Executes a selected portable test through the pure native test owner.
///
/// # Errors
///
/// Refuses an envelope/program mismatch or any owner-side static or VM error.
pub fn execute_portable_test(
    program: &PortableTestProgram,
    worker: &WorkerRequest,
) -> Result<NativeExecutionOutcome, PortableExecutionError> {
    execute_native_test_source(source_input(program, worker)?)
}

/// Produces one unmeasured canonical execution report and comparison entry.
///
/// # Errors
///
/// Refuses invalid portable source, native VM failure, or report defect.
pub fn evaluate_portable_test(
    program: &PortableTestProgram,
    worker: &WorkerRequest,
) -> Result<NativeTestEvaluation, PortableExecutionError> {
    evaluate_native_test_source(source_input(program, worker)?, program.plan().plan_id())
}

/// Records a pure portable test as an observed or VM-rejected report.
///
/// This report is diagnostic until the supervisor measures the worker and
/// the owner verifies admission against its protected plan.
///
/// # Errors
///
/// Refuses invalid portable source or a report-building defect.
pub fn report_portable_test(
    program: &PortableTestProgram,
    worker: &WorkerRequest,
) -> Result<NativeExecutionReportV1, PortableExecutionError> {
    report_native_test_source(source_input(program, worker)?, program.plan().plan_id())
}
