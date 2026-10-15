//! `sley-agent` binary entry.

mod allocator;

/// The binary's allocator (ADR-0052): a bounded size-class cache over the
/// system allocator, so batches do not pay musl's page faults for every input.
#[global_allocator]
static ALLOCATOR: allocator::SizeClassCache = allocator::SizeClassCache;

fn main() {
    sley_agent::residual::frontier::heap::enable_current_thread();
    sley_agent::residual::frontier::working_memory::enable_current_thread();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let status = sley_agent::cli::run_then_exit(&args, &mut std::io::stdout().lock());
    std::process::exit(status);
}
