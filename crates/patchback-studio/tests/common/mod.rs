//! Helpers the studio tests share: the repository root, scratch run directories under
//! `CARGO_TARGET_TMPDIR`, and the JSON of an export.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The repository root.
pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Asserts that the authored scenario a test holds exists, so a renamed scenario fails here.
pub fn scenario(name: &str) -> PathBuf {
    let path = repository_root()
        .join("spec/scenarios")
        .join(format!("{name}.yaml"));
    assert!(path.is_file(), "{} does not exist", path.display());
    path
}

/// A fresh, empty directory named after `name` and the process.
pub fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("patchback-studio-{name}-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("remove the old scratch directory");
    }
    fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

/// The pluck demo voice, a `subtractive-mono/1` patch.
pub fn demo_patch() -> String {
    fs::read_to_string(repository_root().join("demos/pluck/voice.patch")).expect("the demo voice")
}

/// The one topology a run has today.
pub const TOPOLOGY: &str = "subtractive-mono/1";

/// Its parameter paths in index order, as the `topology` line of `trace.events` lists them.
pub fn topology_paths() -> Vec<&'static str> {
    vec![
        "attack",
        "decay",
        "sustain",
        "release",
        "filter_cutoff",
        "resonance_q",
        "drive",
        "output_gain",
    ]
}

/// A `trace.events` text of `lines`, the `evaluation` lines, closed with `stop_reason`.
pub fn trace_text(lines: &[String], stop_reason: &str) -> String {
    let mut text = format!(
        "patchback-fit-trace 1\ntopology {TOPOLOGY} {}\n",
        topology_paths().join(" ")
    );
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    text.push_str(&format!("completed {stop_reason}\n"));
    text
}

/// A `run.meta` as `patchback fit` writes one.
pub fn default_run_meta() -> &'static str {
    "patchback-fit-run-meta 1\npatchback 0.11.0\ninit default\nseed 0\n\
     topology subtractive-mono/1\nfeature_contract features/1\nprng xoshiro256**\n\
     modelspec 0.5.0\nreference sha256 \
     1f35d70cdfaa069216d16df4e185520076b6eb245f8f973fb5cd22fea68f8765\nnotes sha256 \
     77472ec4368f948104562ba37809f1dae9420a0f00a30e5f21e3de8886a3c7ea\nmax_renders 400\n\
     max_wall_time_ms 600000\nmax_restarts 4\nimprovement_threshold 0.0001\nblock_size 256\n"
}

/// The patch a fit retains at every parameter's normalized 0.5, as `patch.patch` holds it.
pub fn fitted_patch() -> String {
    patch_of(&[0.5; 8])
}

/// The patch `patch_at` makes of `normalized`, as `patch.patch` holds it.
pub fn patch_of(normalized: &[f64]) -> String {
    let topology = patchback_core::patch::known_topology(TOPOLOGY).expect("the topology");
    let patch = patchback_fit::patch_at(&topology, normalized).expect("a patch");
    patchback_core::patch::write_patch(&patch).expect("written")
}

/// The `patch.patch` a fit writes when it retains the values of the hand-written `patch`: each
/// value through its contract's normalized form and back (`patch_at`), so a value may move by a
/// few units of its last digit, as a fit's patch file holds it.
pub fn fit_of(patch: &str) -> String {
    use patchback_core::patch::{known_topology, read_patch};
    let patch = read_patch(patch.as_bytes()).expect("a patch");
    let topology = known_topology(&patch.topology_id).expect("a known topology");
    let values: Vec<f64> = topology
        .contracts()
        .iter()
        .map(|contract| {
            let physical = patch
                .nodes
                .iter()
                .flat_map(|node| &node.parameters)
                .find(|p| p.name == contract.path)
                .expect("every parameter")
                .value;
            contract
                .to_normalized(physical)
                .expect("a normalized value")
        })
        .collect();
    patch_of(&values)
}

/// The `patch.patch` of a run whose last retained `evaluation` line in `lines` is the retained
/// patch: that line's normalized values, its last eight fields.
pub fn retained_patch(lines: &[String]) -> String {
    let line = lines
        .iter()
        .rev()
        .find(|line| line.contains(" first-retained ") || line.contains(" improved "))
        .expect("a retained line");
    let fields: Vec<&str> = line.split_ascii_whitespace().collect();
    let values: Vec<f64> = fields[fields.len() - 8..]
        .iter()
        .map(|v| v.parse().expect("a value"))
        .collect();
    patch_of(&values)
}

/// The normalized values whose `patch_at` holds `patch`'s parameter values exactly as its file
/// writes them, or `None` for a patch that is not a drawable `subtractive-mono/1` patch.
fn normalized_of(patch: &str) -> Option<Vec<f64>> {
    use patchback_core::patch::{format_decimal, known_topology, read_patch};
    let patch = read_patch(patch.as_bytes()).ok()?;
    let topology = known_topology(&patch.topology_id)?;
    let mut values = Vec::new();
    for contract in topology.contracts() {
        let physical = patch
            .nodes
            .iter()
            .flat_map(|node| &node.parameters)
            .find(|p| p.name == contract.path)?
            .value;
        let held = |n: f64| {
            let low = contract.physical_at_zero.min(contract.physical_at_one);
            let high = contract.physical_at_zero.max(contract.physical_at_one);
            contract
                .apply(n)
                .ok()
                .map(|s| format_decimal(s.physical.clamp(low, high)))
        };
        let want = Some(format_decimal(physical));
        let start = contract.to_normalized(physical).ok()?;
        let (mut up, mut down) = (start, start);
        let mut found = None;
        for _ in 0..4096 {
            if held(up) == want {
                found = Some(up);
                break;
            }
            if held(down) == want {
                found = Some(down);
                break;
            }
            up = up.next_up();
            down = down.next_down();
        }
        values.push(found?);
    }
    Some(values)
}

/// A run directory holding `patch` as `patch.patch`, and `run.meta` and `trace.events`: the
/// given texts, or by default a run's that retained `patch` at its one evaluation (a patch that
/// does not parse or validate gets every value at 0.5; the export refuses it before its trace).
pub fn run_dir(name: &str, patch: &str, run_meta: Option<&str>, trace: Option<&str>) -> PathBuf {
    let dir = scratch(name);
    fs::write(dir.join("patch.patch"), patch).expect("write patch.patch");
    fs::write(dir.join("run.meta"), run_meta.unwrap_or(default_run_meta()))
        .expect("write run.meta");
    let values = normalized_of(patch).unwrap_or_else(|| vec![0.5; 8]);
    let values: Vec<String> = values
        .iter()
        .map(|v| patchback_core::patch::format_decimal(*v))
        .collect();
    let default = trace_text(
        &[format!(
            "evaluation 1 1 0 first-retained {TOPOLOGY} 0.4 0.3 0.2 0.1 0.25 0.25 {}",
            values.join(" ")
        )],
        "RenderBudgetExhausted",
    );
    fs::write(dir.join("trace.events"), trace.unwrap_or(&default)).expect("write trace.events");
    dir
}

/// Parses an export's text.
pub fn json(text: &str) -> Value {
    serde_json::from_str(text).expect("the export is JSON")
}

/// Whether `text` is a version-8 RFC 9562 UUID in its lower-case hyphenated form.
pub fn is_uuid_v8(text: &str) -> bool {
    let bytes = text.as_bytes();
    text.len() == 36
        && [8, 13, 18, 23].iter().all(|&at| bytes[at] == b'-')
        && text
            .chars()
            .filter(|c| *c != '-')
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        && bytes[14] == b'8'
        && matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
}
