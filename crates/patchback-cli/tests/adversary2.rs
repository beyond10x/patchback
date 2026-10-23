//! Adversary pass 2 (wave a1): the dependency check is only a gate if the gate runs it.
//!
//! `task check` is what CI runs (`.github/workflows/check.yml`, step "task check"). The story
//! says "the dependency gate passes on it"; this case asserts that some task reachable from
//! `check` runs `cargo test` over `patchback-cli` (by `--workspace` or `-p patchback-cli`), so
//! `tests/dependencies.rs` executes in CI. Read with `std` only.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// The command lines of each task under `tasks:`, by task name.
fn task_blocks(taskfile: &str) -> BTreeMap<String, Vec<String>> {
    let mut blocks: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut inside_tasks = false;
    let mut current: Option<String> = None;
    for line in taskfile.lines() {
        if !line.starts_with(' ') && !line.is_empty() {
            inside_tasks = line.trim_end() == "tasks:";
            current = None;
            continue;
        }
        if !inside_tasks {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();
        if indent == 2 && trimmed.ends_with(':') {
            let name = trimmed.trim_end_matches(':').to_owned();
            blocks.entry(name.clone()).or_default();
            current = Some(name);
            continue;
        }
        if let Some(name) = &current {
            blocks
                .entry(name.clone())
                .or_default()
                .push(trimmed.to_owned());
        }
    }
    blocks
}

/// Every line of every task reachable from `root` through `- task: <name>` and `deps:`.
fn reachable_lines(blocks: &BTreeMap<String, Vec<String>>, root: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut queue = vec![root.to_owned()];
    let mut lines = Vec::new();
    while let Some(name) = queue.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some(block) = blocks.get(&name) else {
            continue;
        };
        for line in block {
            if let Some(called) = line.strip_prefix("- task:") {
                queue.push(called.trim().to_owned());
            }
            if let Some(deps) = line.strip_prefix("deps:") {
                for dep in deps.trim().trim_matches(['[', ']']).split(',') {
                    queue.push(dep.trim().to_owned());
                }
            }
            lines.push(line.clone());
        }
    }
    lines
}

#[test]
fn adversary2_task_check_runs_the_patchback_cli_tests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repository root");
    let taskfile = fs::read_to_string(root.join("Taskfile.yml")).expect("Taskfile.yml");
    let blocks = task_blocks(&taskfile);
    assert!(blocks.contains_key("check"), "{:?}", blocks.keys());
    let lines = reachable_lines(&blocks, "check");
    let runs_cli_tests = lines.iter().any(|line| {
        line.contains("cargo test")
            && (line.contains("--workspace") || line.contains("-p patchback-cli"))
    });
    let cargo_tests: Vec<&String> = lines.iter().filter(|l| l.contains("cargo test")).collect();
    assert!(
        runs_cli_tests,
        "task check never runs the patchback-cli tests (tests/dependencies.rs, tests/dispatch.rs); \
         its cargo test lines: {cargo_tests:?}"
    );
}
