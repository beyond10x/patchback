//! `patchback patch validate <file>` (DESIGN.md § 5.1, § 11): loads and validates a `.patch` file
//! with `patchback-core` ([`load_patch_file`], which answers with the refusal the specification
//! declares first) and compiles the validated patch ([`compile`]). Nothing is written and nothing
//! is rendered.
//!
//! One line either way, the path quoted with its control characters escaped (`Path` `Debug`), so
//! a newline in a file name cannot split it:
//!
//! - valid (status 0), on stdout:
//!   `patchback: "<file>" is valid: topology <id>, <n> nodes, <m> edges`;
//! - refused (status 1), on stderr:
//!   `patchback: patch validate refused: <domain error>: "<file>": <where>: <reason>`, where
//!   `<where>` is `line <n>` for a refusal of the file (absent for a condition of the whole
//!   file), or the node (`node <id>`) or edge (its `.patch` line) for a refusal of the graph; a
//!   refusal of the whole patch (format version, sample rate, topology, a cap) names no node; a
//!   cycle names its nodes. An unreadable file is
//!   `patchback: patch validate refused: cannot read "<file>": <error>`.

use std::fmt::Display;
use std::path::Path;
use std::process::ExitCode;

use patchback_core::patch::{compile, load_patch_file};

/// The file was read and refused, or could not be read.
const REFUSED: u8 = 1;

/// Runs `patch validate` on `file`, prints its one line and returns its exit status.
pub fn run(file: &Path) -> ExitCode {
    match check(file) {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(line) => {
            eprintln!("{line}");
            ExitCode::from(REFUSED)
        }
    }
}

/// The line `patch validate` prints: `Ok` on stdout, `Err` on stderr.
fn check(file: &Path) -> Result<String, String> {
    let validated = load_patch_file(file).map_err(|error| match error.spec_error() {
        None => refusal(format_args!("cannot read {file:?}: {error}")),
        Some(spec_error) => refusal(format_args!("{spec_error}: {file:?}: {error}")),
    })?;
    let plan = compile(&validated);
    debug_assert_eq!(plan.steps().len(), validated.patch().nodes.len());
    let patch = validated.patch();
    Ok(one_line(format!(
        "patchback: {file:?} is valid: topology {}, {} nodes, {} edges",
        patch.topology_id,
        patch.nodes.len(),
        patch.edges.len()
    )))
}

fn refusal(message: impl Display) -> String {
    one_line(format!("patchback: patch validate refused: {message}"))
}

/// One line: any control character left in a message (from a token of the file) is escaped.
fn one_line(text: String) -> String {
    text.chars()
        .flat_map(|c| {
            let escaped: Vec<char> = if c.is_control() {
                c.escape_default().collect()
            } else {
                vec![c]
            };
            escaped
        })
        .collect()
}
