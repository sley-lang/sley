//! `sley-agent` binary entry.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let status = sley_agent::cli::run(&args, &mut std::io::stdout().lock());
    std::process::exit(status);
}
