//! The `sley` binary: standard streams in, exit status out (S20-430).

#![forbid(unsafe_code)]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let status = sley_cli::run(
        &args,
        &mut stdin.lock(),
        &mut stdout.lock(),
        &mut stderr.lock(),
    );
    std::process::exit(status);
}
