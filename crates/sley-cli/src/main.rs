//! The `sley` binary: standard streams in, exit status out (S20-430).

#![forbid(unsafe_code)]

fn main() {
    // Argument words are raw operating-system bytes: a word that is not
    // valid Unicode is a usage failure under the contract's exit table,
    // never a panic, so the words are taken as `OsString` and judged by
    // the library.
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let status = sley_cli::run_os(
        &args,
        &mut stdin.lock(),
        &mut stdout.lock(),
        &mut stderr.lock(),
    );
    std::process::exit(status);
}
