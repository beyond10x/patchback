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
//! | 0 | `--help`, `-h` or `--version` printed to stdout; also `--help` or `-h` anywhere after a recognised command (`patch`, `patch validate`, `render`, `analyze`, `compare`, `fit`), which prints the usage |
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
    if arguments_of_recognised(&args).is_some_and(|rest| rest.iter().copied().any(is_help)) {
        return Outcome::Stdout(USAGE_TEXT.to_owned());
    }
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

fn is_help(arg: &str) -> bool {
    matches!(arg, "--help" | "-h")
}

/// The arguments after a recognised command (`patch`, `patch validate`, `render`, `analyze`,
/// `compare`, `fit`), or `None` when the command line does not start with one. `patch` followed
/// by anything but a help flag is not recognised here, so `patch render -h` stays unknown.
fn arguments_of_recognised<'a>(args: &'a [&'a str]) -> Option<&'a [&'a str]> {
    match args {
        ["patch", "validate", rest @ ..] => Some(rest),
        ["patch", rest @ ..] if rest.first().is_none_or(|arg| is_help(arg)) => Some(rest),
        ["render" | "analyze" | "compare" | "fit", rest @ ..] => Some(rest),
        _ => None,
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
