//! `patchback patch validate <file>` run as the built binary on files written to the test target's
//! scratch directory (`CARGO_TARGET_TMPDIR`): status 0 with one stdout line, status 1 with one
//! stderr line for each class of refusal (unreadable, refused by the parser, refused by
//! validation), status 2 for usage errors.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The § 5.2 voice as a `.patch` file, every parameter inside its contract.
const VOICE: &str = "\
patchback-patch 1
sample_rate_hz 48000
topology_id subtractive-mono/1
node 1 NoteInput
node 2 SawOscillator
node 3 Drive
param 3 drive 6
node 4 LowpassFilter
param 4 filter_cutoff 1200
param 4 resonance_q 0.7
node 5 AdsrEnvelope
param 5 attack 0.01
param 5 decay 0.2
param 5 sustain 0.6
param 5 release 0.3
node 6 Vca
node 7 Output
param 7 output_gain -6
edge 1 pitch Control 2 pitch Control
edge 1 gate Event 5 gate Event
edge 2 out Audio 3 in Audio
edge 3 out Audio 4 in Audio
edge 4 out Audio 6 in Audio
edge 5 env Control 6 gain Control
edge 6 out Audio 7 in Audio
";

const REFUSED: &str = "patchback: patch validate refused: ";

/// A fresh directory for one test.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patch-validate")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("the old scratch directory is removed");
    }
    fs::create_dir_all(&dir).expect("the scratch directory is created");
    dir
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, text).expect("the patch file is written");
    path
}

fn run(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(args)
        .output()
        .expect("the patchback binary runs")
}

fn validate(path: &Path) -> Output {
    run(&["patch".as_ref(), "validate".as_ref(), path.as_os_str()])
}

fn lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8(bytes.to_vec())
        .expect("output is UTF-8")
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Status 1, nothing on stdout, one stderr line; returns it.
fn refusal(path: &Path) -> String {
    let out = validate(path);
    assert_eq!(out.status.code(), Some(1), "{path:?}");
    assert!(out.stdout.is_empty(), "{path:?}");
    let mut stderr = lines(&out.stderr);
    assert_eq!(stderr.len(), 1, "{stderr:?}");
    stderr.remove(0)
}

/// `VOICE` with `from` replaced by `to`.
fn voice_with(from: &str, to: &str) -> String {
    assert!(VOICE.contains(from), "{from}");
    VOICE.replacen(from, to, 1)
}

#[test]
fn a_valid_patch_prints_one_line_naming_it_and_its_counts_and_exits_0() {
    let dir = scratch("valid");
    let path = write(&dir, "voice.patch", VOICE);
    let out = validate(&path);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stderr.is_empty(), "{:?}", lines(&out.stderr));
    assert_eq!(
        lines(&out.stdout),
        [format!(
            "patchback: {path:?} is valid: topology subtractive-mono/1, 7 nodes, 7 edges"
        )]
    );
}

#[test]
fn an_unreadable_file_is_refused_with_its_path_and_exits_1() {
    let dir = scratch("unreadable");
    let path = dir.join("absent.patch");
    let line = refusal(&path);
    assert!(
        line.starts_with(&format!("{REFUSED}cannot read {path:?}: ")),
        "{line}"
    );
}

#[test]
fn a_file_the_parser_refuses_names_its_error_and_line_and_exits_1() {
    let dir = scratch("load");
    let path = write(
        &dir,
        "voice.patch",
        &voice_with("node 2 SawOscillator\n", "node 2 SawOscillator extra\n"),
    );
    let line = refusal(&path);
    assert!(
        line.starts_with(&format!(
            "{REFUSED}patchback.patch.MalformedRecord: {path:?}: line 5: "
        )),
        "{line}"
    );
    let path = write(
        &dir,
        "number.patch",
        &voice_with("param 3 drive 6\n", "param 3 drive 6..0\n"),
    );
    let line = refusal(&path);
    assert!(
        line.starts_with(&format!(
            "{REFUSED}patchback.patch.MalformedNumber: {path:?}: line 7: "
        )),
        "{line}"
    );
}

#[test]
fn a_graph_validation_refuses_names_its_error_and_node_and_exits_1() {
    let dir = scratch("node");
    let path = write(
        &dir,
        "voice.patch",
        &voice_with("param 7 output_gain -6\n", "param 7 output_gain 3\n"),
    );
    let line = refusal(&path);
    assert!(
        line.starts_with(&format!(
            "{REFUSED}patchback.patch.ParameterContractViolation: {path:?}: node 7: "
        )),
        "{line}"
    );
}

#[test]
fn a_graph_validation_refuses_names_its_error_and_edge_and_exits_1() {
    let dir = scratch("edge");
    let cases = [
        (
            "unknown-node.patch",
            "edge 6 out Audio 7 in Audio\n",
            "edge 6 out Audio 9 in Audio\n",
            "patchback.patch.UnknownNode: {path}: edge 6 out Audio 9 in Audio: node 9 is not declared",
        ),
        (
            "unknown-port.patch",
            "edge 6 out Audio 7 in Audio\n",
            "edge 6 out Audio 7 sidechain Audio\n",
            "patchback.patch.UnknownPort: {path}: edge 6 out Audio 7 sidechain Audio: ",
        ),
        (
            "rates.patch",
            "edge 5 env Control 6 gain Control\n",
            "edge 4 out Audio 6 gain Control\n",
            "patchback.patch.IncompatiblePortRates: {path}: edge 4 out Audio 6 gain Control: ",
        ),
    ];
    for (name, from, to, expected) in cases {
        let path = write(&dir, name, &voice_with(from, to));
        let line = refusal(&path);
        let expected = format!(
            "{REFUSED}{}",
            expected.replace("{path}", &format!("{path:?}"))
        );
        assert!(line.starts_with(&expected), "{line}\n{expected}");
    }
}

#[test]
fn a_cycle_is_refused_naming_its_nodes_and_exits_1() {
    let dir = scratch("cycle");
    let path = write(
        &dir,
        "cycle.patch",
        "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n\
         node 3 Drive\nparam 3 drive 6\nnode 4 LowpassFilter\nparam 4 filter_cutoff 1200\n\
         param 4 resonance_q 0.7\nedge 3 out Audio 4 in Audio\nedge 4 out Audio 3 in Audio\n",
    );
    let line = refusal(&path);
    assert!(
        line.starts_with(&format!(
            "{REFUSED}patchback.patch.ZeroDelayCycle: {path:?}: "
        )),
        "{line}"
    );
    assert!(line.contains("3 -> 4 -> 3"), "{line}");
}

#[test]
fn a_refusal_of_the_whole_patch_names_its_error_and_exits_1() {
    let dir = scratch("whole");
    let path = write(
        &dir,
        "topology.patch",
        &voice_with(
            "topology_id subtractive-mono/1\n",
            "topology_id fm-poly/1\n",
        ),
    );
    let line = refusal(&path);
    assert!(
        line.starts_with(&format!(
            "{REFUSED}patchback.patch.UnknownTopology: {path:?}: topology fm-poly/1 is not known"
        )),
        "{line}"
    );
    let path = write(
        &dir,
        "voices.patch",
        &voice_with("node 1 NoteInput\n", "node 1 NoteInput\nnode 8 NoteInput\n"),
    );
    let line = refusal(&path);
    assert!(
        line.starts_with(&format!(
            "{REFUSED}patchback.patch.ResourceCapExceeded: {path:?}: 2 voices"
        )),
        "{line}"
    );
}

#[test]
fn a_newline_in_the_path_cannot_split_the_line() {
    let dir = scratch("newline");
    let valid = write(&dir, "two\nlines.patch", VOICE);
    let out = validate(&valid);
    assert_eq!(out.status.code(), Some(0));
    let stdout = lines(&out.stdout);
    assert_eq!(stdout.len(), 1, "{stdout:?}");
    assert!(stdout[0].contains("two\\nlines.patch"), "{stdout:?}");
    let refused = write(
        &dir,
        "three\nlines.patch",
        &voice_with("param 3 drive 6\n", "param 3 drive 99\n"),
    );
    let line = refusal(&refused);
    assert!(line.contains("three\\nlines.patch"), "{line}");
}

/// A clap usage error: status 2, nothing on stdout, `error: ` first on stderr.
fn assert_usage_error(args: &[&str], what: &str) {
    let args: Vec<&std::ffi::OsStr> = args.iter().map(|a| a.as_ref()).collect();
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{args:?}");
    assert!(out.stdout.is_empty(), "{args:?}");
    let stderr = String::from_utf8(out.stderr).expect("UTF-8");
    assert!(stderr.starts_with("error: "), "{args:?}: {stderr}");
    assert!(stderr.contains(what), "{args:?}: {stderr}");
}

#[test]
fn a_usage_error_exits_2() {
    assert_usage_error(&["patch", "validate"], "<FILE>");
    assert_usage_error(&["patch", "validate", "a.patch", "b.patch"], "'b.patch'");
    assert_usage_error(
        &["patch", "validate", "--anything", "a.patch"],
        "'--anything'",
    );
}
