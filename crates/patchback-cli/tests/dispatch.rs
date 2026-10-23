//! The `patchback` binary's dispatch on clap derive: help, version, the not-yet-implemented
//! subcommands, `analyze`, `compare` and clap's usage errors, each with its exit status and
//! output; what `analyze` computes and refuses is in `tests/analyze.rs`, and `compare` in
//! `tests/compare.rs`.

use std::process::{Command, Output};

/// The subcommands DESIGN.md § 11 names that are recognised and not implemented yet, as typed
/// (`patch validate` is implemented: `tests/patch_validate.rs`; `analyze`: `tests/analyze.rs`;
/// `compare`: `tests/compare.rs`).
const STUBS: &[&[&str]] = &[&["render"], &["fit"]];

/// The top-level commands the root help lists.
const TOP_LEVEL: &[&str] = &["patch", "render", "analyze", "compare", "fit"];

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(args)
        .output()
        .expect("the patchback binary runs")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("output is UTF-8")
}

fn lines(bytes: &[u8]) -> Vec<String> {
    text(bytes).lines().map(str::to_owned).collect()
}

/// The root help: clap's synopsis line and one line per top-level command.
fn assert_root_help(help: &str) {
    assert!(
        help.lines()
            .any(|line| line == "Usage: patchback <COMMAND>"),
        "help carries clap's synopsis: {help}"
    );
    for name in TOP_LEVEL {
        assert!(
            help.lines()
                .any(|line| line.trim_start().starts_with(&format!("{name} "))),
            "help lists {name}: {help}"
        );
    }
}

/// A clap usage error: exit 2, nothing on stdout, `error: ` first on stderr, naming `what`.
fn assert_usage_error(argv: &[&str], what: &str) {
    let out = run(argv);
    assert_eq!(out.status.code(), Some(2), "{argv:?}");
    assert!(out.stdout.is_empty(), "{argv:?}");
    let stderr = text(&out.stderr);
    assert!(stderr.starts_with("error: "), "{argv:?}: {stderr}");
    assert!(stderr.contains(&format!("'{what}'")), "{argv:?}: {stderr}");
}

#[test]
fn no_arguments_prints_help_to_stderr_and_exits_2() {
    let out = run(&[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert_root_help(&text(&out.stderr));
}

#[test]
fn help_prints_root_help_to_stdout_and_exits_0() {
    for flag in ["--help", "-h"] {
        let out = run(&[flag]);
        assert_eq!(out.status.code(), Some(0), "{flag}");
        assert!(out.stderr.is_empty(), "{flag}");
        assert_root_help(&text(&out.stdout));
    }
}

#[test]
fn help_subcommand_prints_root_help_and_exits_0() {
    let out = run(&["help"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stderr.is_empty());
    assert_root_help(&text(&out.stdout));
}

#[test]
fn version_prints_the_package_version_and_exits_0() {
    for flag in ["--version", "-V"] {
        let out = run(&[flag]);
        assert_eq!(out.status.code(), Some(0), "{flag}");
        assert!(out.stderr.is_empty(), "{flag}");
        assert_eq!(
            lines(&out.stdout),
            [format!("patchback {}", env!("CARGO_PKG_VERSION"))],
            "{flag}"
        );
    }
}

/// `patchback <stub>` writes one stderr line and exits 3.
fn assert_not_implemented(stub: &[&str]) {
    let name = stub.join(" ");
    let out = run(stub);
    assert_eq!(out.status.code(), Some(3), "{stub:?}");
    assert!(out.stdout.is_empty(), "{stub:?}");
    assert_eq!(
        lines(&out.stderr),
        [format!("patchback: {name} is not implemented yet")],
        "{stub:?}"
    );
}

#[test]
fn render_is_not_implemented_and_exits_3() {
    assert_not_implemented(&["render"]);
}

#[test]
fn fit_is_not_implemented_and_exits_3() {
    assert_not_implemented(&["fit"]);
}

#[test]
fn an_argument_a_stub_does_not_take_is_a_usage_error_and_exits_2() {
    for stub in STUBS {
        for extra in ["--anything", "song.patch"] {
            let mut argv = stub.to_vec();
            argv.push(extra);
            assert_usage_error(&argv, extra);
        }
    }
}

#[test]
fn analyze_with_an_argument_it_does_not_take_is_a_usage_error_and_exits_2() {
    let base = ["analyze", "--input", "a.wav", "--out", "analysis"];
    for extra in ["--anything", "song.patch"] {
        let mut argv = base.to_vec();
        argv.push(extra);
        assert_usage_error(&argv, extra);
    }
}

#[test]
fn compare_with_an_argument_it_does_not_take_is_a_usage_error_and_exits_2() {
    let base = ["compare", "--reference", "a.wav", "--candidate", "b.wav"];
    for extra in ["--anything", "song.patch"] {
        let mut argv = base.to_vec();
        argv.push(extra);
        assert_usage_error(&argv, extra);
    }
}

#[test]
fn compare_without_a_candidate_is_a_usage_error_and_exits_2() {
    let out = run(&["compare", "--reference", "a.wav"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let stderr = text(&out.stderr);
    assert!(stderr.starts_with("error: "), "{stderr}");
    assert!(stderr.contains("--candidate <CANDIDATE>"), "{stderr}");
}

#[test]
fn analyze_without_its_arguments_is_a_usage_error_and_exits_2() {
    let out = run(&["analyze"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let stderr = text(&out.stderr);
    assert!(stderr.starts_with("error: "), "{stderr}");
    assert!(stderr.contains("--input <INPUT>"), "{stderr}");
    assert!(stderr.contains("--out <OUT>"), "{stderr}");
}

#[test]
fn an_unknown_subcommand_is_a_usage_error_and_exits_2() {
    for unknown in ["generate", "Render"] {
        assert_usage_error(&[unknown], unknown);
    }
    assert_usage_error(&["patch", "render"], "render");
}

#[test]
fn an_unknown_flag_is_a_usage_error_and_exits_2() {
    assert_usage_error(&["--verbose"], "--verbose");
}

/// `patchback <command> --help` and `-h` print that command's help to stdout and exit 0.
fn assert_help_after(command: &[&str]) {
    let synopsis = format!("Usage: patchback {}", command.join(" "));
    for flag in ["--help", "-h"] {
        let mut argv = command.to_vec();
        argv.push(flag);
        let out = run(&argv);
        assert_eq!(out.status.code(), Some(0), "{argv:?}");
        assert!(out.stderr.is_empty(), "{argv:?}: {}", text(&out.stderr));
        let help = text(&out.stdout);
        assert!(
            help.lines().any(|line| line.starts_with(&synopsis)),
            "{argv:?}: {help}"
        );
    }
}

#[test]
fn help_after_patch_prints_its_help_and_exits_0() {
    assert_help_after(&["patch"]);
}

#[test]
fn help_after_patch_validate_prints_its_help_and_exits_0() {
    assert_help_after(&["patch", "validate"]);
}

#[test]
fn help_after_render_prints_its_help_and_exits_0() {
    assert_help_after(&["render"]);
}

#[test]
fn help_after_analyze_prints_its_help_and_exits_0() {
    assert_help_after(&["analyze"]);
}

#[test]
fn help_after_compare_prints_its_help_and_exits_0() {
    assert_help_after(&["compare"]);
}

#[test]
fn help_after_fit_prints_its_help_and_exits_0() {
    assert_help_after(&["fit"]);
}

#[test]
fn help_after_an_unknown_command_is_still_a_usage_error_and_exits_2() {
    assert_usage_error(&["generate", "--help"], "generate");
    assert_usage_error(&["patch", "render", "-h"], "render");
}

#[test]
fn patch_without_validate_prints_its_help_to_stderr_and_exits_2() {
    let out = run(&["patch"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let help = text(&out.stderr);
    assert!(
        help.lines()
            .any(|line| line == "Usage: patchback patch <COMMAND>"),
        "{help}"
    );
    assert!(
        help.lines()
            .any(|line| line.trim_start().starts_with("validate ")),
        "{help}"
    );
}
