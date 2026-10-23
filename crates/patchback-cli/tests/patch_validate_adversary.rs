//! Adversary cases for `patchback patch validate` (story:patch-graph-validation, wave a3,
//! pass 1). Files are written under `CARGO_TARGET_TMPDIR`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

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

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patch-validate-adversary")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("old scratch removed");
    }
    fs::create_dir_all(&dir).expect("scratch created");
    dir
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, text).expect("written");
    path
}

fn validate(path: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(["patch".as_ref(), "validate".as_ref(), path.as_os_str()])
        .output()
        .expect("the binary runs")
}

/// Status 1, empty stdout, exactly one stderr line; returns it.
fn refusal(path: &Path) -> String {
    let out = validate(path);
    let stderr = String::from_utf8(out.stderr).expect("UTF-8");
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(out.stdout.is_empty());
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
    lines[0].to_owned()
}

/// `spec/domains/patch.yaml` `LoadPatch` declares `unknown-topology` before `malformed-record`,
/// `malformed-number` and `unbounded-length` (and it is a predicate outcome, which the unit says
/// ESS answers before any external one). `VALIDATE_PATCH_PRECEDENCE` claims the spec's order. A
/// file with an unknown topology and a malformed number is refused as malformed instead.
#[test]
fn adversary_an_unknown_topology_outranks_a_malformed_number() {
    let dir = scratch("topology-vs-number");
    let text = VOICE
        .replacen("subtractive-mono/1", "subtractive-mono/2", 1)
        .replacen("param 7 output_gain -6", "param 7 output_gain -6x", 1);
    let path = write(&dir, "two-faults.patch", &text);
    let line = refusal(&path);
    assert!(
        line.contains("patchback.patch.UnknownTopology"),
        "spec order says UnknownTopology; got: {line}"
    );
}

/// The same for a kind outside the catalogue (`LoadPatch` `lfo-outside-catalogue`, declared
/// before `malformed-number`).
#[test]
fn adversary_a_kind_outside_the_catalogue_outranks_a_malformed_number() {
    let dir = scratch("catalogue-vs-number");
    let text = VOICE.replacen(
        "param 7 output_gain -6",
        "node 8 Lfo\nparam 7 output_gain -6x",
        1,
    );
    let path = write(&dir, "two-faults.patch", &text);
    let line = refusal(&path);
    assert!(
        line.contains("patchback.patch.KindOutsideCatalogue"),
        "spec order says KindOutsideCatalogue; got: {line}"
    );
}

/// DESIGN.md § 12.1 Graph: "duplicates". The same edge line twice reaches validation from a file.
#[test]
fn adversary_a_duplicated_edge_line_is_refused() {
    let dir = scratch("duplicate-edge");
    let text = format!("{VOICE}edge 6 out Audio 7 in Audio\n");
    let path = write(&dir, "dup.patch", &text);
    let out = validate(&path);
    assert_eq!(
        out.status.code(),
        Some(1),
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// A directory is not a file: one "cannot read" line, status 1.
#[test]
fn adversary_a_directory_is_refused_as_unreadable() {
    let dir = scratch("directory");
    let line = refusal(&dir);
    assert!(
        line.starts_with("patchback: patch validate refused: cannot read \""),
        "{line}"
    );
}

/// An endless source is read only up to the file bound, then refused; the command terminates.
#[test]
fn adversary_an_endless_file_is_refused_within_the_bound() {
    let zero = Path::new("/dev/zero");
    if !zero.exists() {
        return;
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(["patch".as_ref(), "validate".as_ref(), zero.as_os_str()])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawns");
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("waits") {
            break status;
        }
        if Instant::now() > deadline {
            child.kill().expect("killed");
            panic!("patch validate /dev/zero did not terminate in 20 s");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(status.code(), Some(1));
}

/// A file one byte past `MAX_FILE_BYTES` is refused with the length bound, one line.
#[test]
fn adversary_a_file_past_the_byte_bound_is_refused_with_the_length_bound() {
    let dir = scratch("huge");
    let mut text = String::from(VOICE);
    while text.len() <= 1_048_576 {
        text.push_str("edge 6 out Audio 7 in Audio\n");
    }
    let path = write(&dir, "huge.patch", &text);
    let line = refusal(&path);
    assert!(
        line.contains("patchback.patch.LengthBoundExceeded"),
        "{line}"
    );
}
