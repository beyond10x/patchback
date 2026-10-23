//! `patchback`, the application entry point (DESIGN.md § 4.1, § 11).
//!
//! The scaffold recognises the Milestone A subcommands, `patch validate`, `render`, `analyze`,
//! `compare` and `fit`; none is implemented yet. Arguments are read with `std::env` (a
//! non-UTF-8 argument is read lossily rather than panicking), so the binary takes no
//! third-party crate (DESIGN.md § 3).
//!
//! Exit statuses:
//!
//! | Status | Meaning |
//! |---|---|
//! | 0 | `--help`, `-h` or `--version` printed to stdout |
//! | 2 | usage error: no arguments (usage to stderr), an unknown command, or `patch` without `validate` |
//! | 3 | a recognised command that is not implemented yet |
//!
//! Status 1 is left for a recognised command that runs and fails, once commands exist.

use std::env;
use std::process::ExitCode;

/// A recognised command that ran and printed what was asked.
const OK: u8 = 0;
/// The command line names no command, or one Patchback does not have.
const USAGE: u8 = 2;
/// The command is recognised but not implemented yet.
const NOT_IMPLEMENTED: u8 = 3;

const USAGE_TEXT: &str = "\
Usage: patchback <command> [arguments]

Commands:
  patch validate   Check a .patch file without rendering it
  render           Render a patch with a score into a WAV
  analyze          Measure a reference WAV
  compare          Compare a candidate WAV with a reference
  fit              Recover a patch that reproduces a reference WAV

Options:
  -h, --help       Print this usage
      --version    Print the version
";

/// The result of reading the command line: what to print, where, and the exit status.
enum Outcome {
    Stdout(String),
    Stderr(String, u8),
}

fn dispatch(args: &[String]) -> Outcome {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] => Outcome::Stderr(USAGE_TEXT.to_owned(), USAGE),
        ["--help" | "-h", ..] => Outcome::Stdout(USAGE_TEXT.to_owned()),
        ["--version", ..] => Outcome::Stdout(format!("patchback {}\n", env!("CARGO_PKG_VERSION"))),
        ["patch", "validate", ..] => not_implemented("patch validate"),
        ["patch"] => Outcome::Stderr(
            "patchback: patch needs a subcommand: patch validate\n".to_owned(),
            USAGE,
        ),
        ["patch", other, ..] => unknown(&format!("patch {other}")),
        [command @ ("render" | "analyze" | "compare" | "fit"), ..] => not_implemented(command),
        [other, ..] => unknown(other),
    }
}

fn not_implemented(command: &str) -> Outcome {
    Outcome::Stderr(
        format!("patchback: {command} is not implemented yet\n"),
        NOT_IMPLEMENTED,
    )
}

fn unknown(command: &str) -> Outcome {
    Outcome::Stderr(
        format!("patchback: unknown command '{command}'; see patchback --help\n"),
        USAGE,
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    match dispatch(&args) {
        Outcome::Stdout(text) => {
            print!("{text}");
            ExitCode::from(OK)
        }
        Outcome::Stderr(text, status) => {
            eprint!("{text}");
            ExitCode::from(status)
        }
    }
}
