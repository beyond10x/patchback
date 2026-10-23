//! The `patchback` binary's dispatch: usage, version, the not-yet-implemented subcommands and
//! the refusals, each with its exit status and output line.

use std::process::{Command, Output};

/// The subcommands DESIGN.md § 11 names that the scaffold recognises, as typed.
const STUBS: &[&[&str]] = &[
    &["patch", "validate"],
    &["render"],
    &["analyze"],
    &["compare"],
    &["fit"],
];

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

fn assert_usage(usage: &str) {
    assert!(
        usage.starts_with("Usage: patchback <command>"),
        "usage starts with its synopsis: {usage}"
    );
    for stub in STUBS {
        let name = stub.join(" ");
        assert!(
            usage
                .lines()
                .any(|line| line.trim_start().starts_with(&name)),
            "usage lists {name}: {usage}"
        );
    }
}

#[test]
fn no_arguments_prints_usage_to_stderr_and_exits_2() {
    let out = run(&[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert_usage(&text(&out.stderr));
}

#[test]
fn help_prints_usage_to_stdout_and_exits_0() {
    for flag in ["--help", "-h"] {
        let out = run(&[flag]);
        assert_eq!(out.status.code(), Some(0), "{flag}");
        assert!(out.stderr.is_empty(), "{flag}");
        assert_usage(&text(&out.stdout));
    }
}

#[test]
fn version_prints_the_package_version_and_exits_0() {
    let out = run(&["--version"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stderr.is_empty());
    assert_eq!(
        lines(&out.stdout),
        [format!("patchback {}", env!("CARGO_PKG_VERSION"))]
    );
}

#[test]
fn each_stub_says_it_is_not_implemented_and_exits_3() {
    for stub in STUBS {
        let name = stub.join(" ");
        let mut args = stub.to_vec();
        args.push("--anything");
        for argv in [stub.to_vec(), args] {
            let out = run(&argv);
            assert_eq!(out.status.code(), Some(3), "{argv:?}");
            assert!(out.stdout.is_empty(), "{argv:?}");
            assert_eq!(
                lines(&out.stderr),
                [format!("patchback: {name} is not implemented yet")],
                "{argv:?}"
            );
        }
    }
}

#[test]
fn an_unknown_subcommand_is_named_and_exits_2() {
    for unknown in ["generate", "--verbose", "Render"] {
        let out = run(&[unknown]);
        assert_eq!(out.status.code(), Some(2), "{unknown}");
        assert!(out.stdout.is_empty(), "{unknown}");
        assert_eq!(
            lines(&out.stderr),
            [format!(
                "patchback: unknown command '{unknown}'; see patchback --help"
            )],
            "{unknown}"
        );
    }
}

/// `patchback <command> --help` and `-h` print the usage to stdout and exit 0.
fn assert_help_after(command: &[&str]) {
    for flag in ["--help", "-h"] {
        let mut argv = command.to_vec();
        argv.push(flag);
        let out = run(&argv);
        assert_eq!(out.status.code(), Some(0), "{argv:?}");
        assert!(out.stderr.is_empty(), "{argv:?}: {}", text(&out.stderr));
        assert_usage(&text(&out.stdout));
    }
}

#[test]
fn help_after_patch_prints_usage_and_exits_0() {
    assert_help_after(&["patch"]);
}

#[test]
fn help_after_patch_validate_prints_usage_and_exits_0() {
    assert_help_after(&["patch", "validate"]);
}

#[test]
fn help_after_render_prints_usage_and_exits_0() {
    assert_help_after(&["render"]);
}

#[test]
fn help_after_analyze_prints_usage_and_exits_0() {
    assert_help_after(&["analyze"]);
}

#[test]
fn help_after_compare_prints_usage_and_exits_0() {
    assert_help_after(&["compare"]);
}

#[test]
fn help_after_fit_prints_usage_and_exits_0() {
    assert_help_after(&["fit"]);
}

#[test]
fn help_after_an_argument_of_a_recognised_command_prints_usage_and_exits_0() {
    assert_help_after(&["render", "song.patch"]);
    assert_help_after(&["patch", "validate", "song.patch"]);
}

#[test]
fn help_after_an_unknown_command_still_names_it_and_exits_2() {
    for argv in [&["generate", "--help"][..], &["patch", "render", "-h"][..]] {
        let out = run(argv);
        assert_eq!(out.status.code(), Some(2), "{argv:?}");
        assert!(out.stdout.is_empty(), "{argv:?}");
    }
}

#[test]
fn patch_without_validate_is_refused_and_exits_2() {
    let out = run(&["patch"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert_eq!(
        lines(&out.stderr),
        ["patchback: patch needs a subcommand: patch validate"]
    );

    let out = run(&["patch", "render"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert_eq!(
        lines(&out.stderr),
        ["patchback: unknown command 'patch render'; see patchback --help"]
    );
}
