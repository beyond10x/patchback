//! Adversary pass 1 (wave a2, story:cli-clap): the specification's own words about the
//! dependency policy against DESIGN.md revision 0.4, which admits `clap` beside audio I/O.
//! Read with `std` only.

use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels under the repository root")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    fs::read_to_string(repository_root().join(relative)).expect(relative)
}

/// Whitespace collapsed and Markdown emphasis dropped, so a quote wrapped over comment lines
/// compares with the sentence it quotes.
fn normal(text: &str) -> String {
    text.replace("**", "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The `"..."` quotes in the leading `#` comment of a scenario file.
fn header_quotes(scenario: &str) -> Vec<String> {
    let header: String = scenario
        .lines()
        .take_while(|line| line.starts_with('#'))
        .map(|line| line.trim_start_matches('#'))
        .collect::<Vec<_>>()
        .join(" ");
    header.split('"').skip(1).step_by(2).map(normal).collect()
}

#[test]
fn adversary_a2_dependency_scenarios_quote_design_verbatim() {
    let design = normal(&read("DESIGN.md"));
    let mut missing = Vec::new();
    for name in [
        "dependency-gate-audio-backend",
        "dependency-gate-cli-parser",
        "dependency-gate-forbidden-crate",
        "dependency-gate-no-crates-passes",
    ] {
        let path = format!("spec/scenarios/{name}.yaml");
        for quote in header_quotes(&read(&path)) {
            if !design.contains(&quote) {
                missing.push(format!("{path}: \"{quote}\""));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "quoted as DESIGN.md and not in DESIGN.md 0.4: {missing:#?}"
    );
}

#[test]
fn adversary_a2_no_dependency_text_says_audio_io_is_the_only_admitted_category() {
    // DESIGN.md 0.4 admits clap, so a passed audit no longer means "nothing beyond an audio
    // backend", and CLI parsing is a category that is not refused.
    let stale = [
        (
            "spec/domains/dependency.yaml",
            "application crate beyond at most one approved optional audio I/O backend",
        ),
        (
            "spec/scenarios/dependency-gate-forbidden-crate.yaml",
            "Every category but audio I/O is refused",
        ),
    ];
    let found: Vec<String> = stale
        .iter()
        .filter(|(path, sentence)| normal(&read(path)).contains(&normal(sentence)))
        .map(|(path, sentence)| format!("{path}: \"{sentence}\""))
        .collect();
    assert!(
        found.is_empty(),
        "still states the pre-0.4 policy: {found:#?}"
    );
}
