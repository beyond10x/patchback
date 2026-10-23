//! DESIGN.md § 3: the application crates take no third-party crate. Every dependency of every
//! application crate, normal, dev, build or target-specific, is another `patchback-*` crate of
//! this workspace, reached by `path` directly or through a `workspace = true` entry whose
//! workspace definition is a `path`. Read with `std` only.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The application crates of DESIGN.md § 4.1. `patchback-docs` is documentation tooling and
/// is not one of them.
const APPLICATION_CRATES: &[&str] = &[
    "patchback-core",
    "patchback-dsp",
    "patchback-audio",
    "patchback-analysis",
    "patchback-dataset",
    "patchback-model",
    "patchback-fit",
    "patchback-cli",
];

const DEPENDENCY_KINDS: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];

/// One dependency entry: the table it sits in, the dependency key and its specification text.
#[derive(Debug)]
struct Entry {
    table: String,
    name: String,
    spec: String,
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels under the repository root")
        .to_path_buf()
}

/// Splits a table header on the dots that are outside quotes.
fn segments(header: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for c in header.chars() {
        match (quote, c) {
            (None, '\'' | '"') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '.') => {
                parts.push(current.trim().to_owned());
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    parts.push(current.trim().to_owned());
    parts
}

fn unquote(key: &str) -> String {
    key.trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .to_owned()
}

/// Every dependency entry of `manifest`, from both the `[dependencies]` list form and the
/// `[dependencies.<name>]` table form, under any prefix (`target.<cfg>.`, `workspace.`).
fn dependency_entries(manifest: &str) -> Vec<Entry> {
    enum Table {
        Other,
        List(String),
        One(usize),
    }
    let mut entries: Vec<Entry> = Vec::new();
    let mut table = Table::Other;
    for raw in manifest.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(header) = line.strip_prefix('[') {
            let header = header.trim_start_matches('[');
            let header = header.split(']').next().unwrap_or_default();
            let parts = segments(header);
            let kind = parts
                .iter()
                .rposition(|part| DEPENDENCY_KINDS.contains(&unquote(part).as_str()));
            table = match kind {
                Some(i) if i + 1 == parts.len() => Table::List(header.trim().to_owned()),
                Some(i) if i + 2 == parts.len() => {
                    entries.push(Entry {
                        table: parts[..=i].join("."),
                        name: unquote(&parts[i + 1]),
                        spec: String::new(),
                    });
                    Table::One(entries.len() - 1)
                }
                _ => Table::Other,
            };
            continue;
        }
        match &table {
            Table::Other => {}
            Table::List(header) => {
                let (key, value) = line.split_once('=').unwrap_or((line, ""));
                let (name, spec) = match key.split_once('.') {
                    Some((name, rest)) => (unquote(name), format!("{} = {}", rest.trim(), value)),
                    None => (unquote(key), value.trim().to_owned()),
                };
                entries.push(Entry {
                    table: header.clone(),
                    name,
                    spec,
                });
            }
            Table::One(index) => {
                let spec = &mut entries[*index].spec;
                spec.push_str(line);
                spec.push_str(", ");
            }
        }
    }
    entries
}

/// Whether `spec` sets the key `key` to a value starting with `value` (any value when empty).
fn sets(spec: &str, key: &str, value: &str) -> bool {
    spec.match_indices(key).any(|(at, _)| {
        let before = spec[..at].chars().next_back();
        let boundary = !matches!(before, Some(c) if c.is_alphanumeric() || c == '-' || c == '_');
        let rest = spec[at + key.len()..].trim_start();
        boundary
            && rest
                .strip_prefix('=')
                .is_some_and(|v| v.trim_start().starts_with(value))
    })
}

/// The `[workspace.dependencies]` of the root manifest, by name.
fn workspace_dependencies(root_manifest: &str) -> BTreeMap<String, String> {
    dependency_entries(root_manifest)
        .into_iter()
        .filter(|entry| entry.table == "workspace.dependencies")
        .map(|entry| (entry.name, entry.spec))
        .collect()
}

/// The dependencies of `manifest` that are not workspace-local `patchback-*` crates.
fn violations(manifest: &str, workspace: &BTreeMap<String, String>) -> Vec<String> {
    let mut found = Vec::new();
    for entry in dependency_entries(manifest) {
        let Entry { table, name, spec } = &entry;
        let local = if sets(spec, "workspace", "true") {
            workspace.get(name).is_some_and(|ws| sets(ws, "path", ""))
        } else {
            sets(spec, "path", "")
        };
        let renamed_away = sets(spec, "package", "") && !sets(spec, "package", "\"patchback-");
        if !name.starts_with("patchback-") || !local || renamed_away {
            found.push(format!("[{table}] {name} = {spec}"));
        }
    }
    found
}

#[test]
fn the_application_crates_take_no_third_party_crate() {
    let root = repository_root();
    let root_manifest = fs::read_to_string(root.join("Cargo.toml")).expect("root Cargo.toml");
    let workspace = workspace_dependencies(&root_manifest);
    let mut found = Vec::new();
    for name in APPLICATION_CRATES {
        let path = root.join("crates").join(name).join("Cargo.toml");
        match fs::read_to_string(&path) {
            Ok(manifest) => {
                for violation in violations(&manifest, &workspace) {
                    found.push(format!("{name}: {violation}"));
                }
            }
            Err(e) => found.push(format!("{name}: {}: {e}", path.display())),
        }
    }
    assert!(found.is_empty(), "third-party or missing: {found:#?}");
}

#[test]
fn the_workspace_lists_every_crate() {
    let root = repository_root();
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("root Cargo.toml");
    let members = manifest
        .split_once("members")
        .and_then(|(_, rest)| rest.split_once('['))
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(list, _)| list)
        .expect("a members list");
    let members: Vec<String> = members
        .split(',')
        .map(unquote)
        .filter(|m| !m.is_empty())
        .collect();
    for name in APPLICATION_CRATES.iter().chain(&["patchback-docs"]) {
        let member = format!("crates/{name}");
        assert!(members.contains(&member), "{member} in {members:?}");
    }
}

#[test]
fn the_check_refuses_every_form_of_a_third_party_crate() {
    let workspace = workspace_dependencies(
        "[workspace.dependencies]\n\
         clap = { version = \"4\" }\n\
         patchback-core = { path = \"crates/patchback-core\" }\n",
    );
    let refused = [
        "[dependencies]\nclap = \"4\"\n",
        "[dependencies]\nclap.workspace = true\n",
        "[dependencies]\nclap = { workspace = true }\n",
        "[dev-dependencies]\nserde = { version = \"1\" }\n",
        "[build-dependencies]\ncc = \"1\"\n",
        "[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n",
        "[target.\"x86_64-unknown-linux-gnu\".dev-dependencies]\nlibc = \"0.2\"\n",
        "[dependencies.rand]\nversion = \"0.8\"\n",
        "[dependencies]\npatchback-core = \"0.0.0\"\n",
        "[dependencies]\npatchback-core = { git = \"https://example.invalid/x\" }\n",
        "[dependencies]\npatchback-dsp.workspace = true\n",
        "[dependencies]\nlocal-thing = { path = \"../local-thing\" }\n",
        "[dependencies]\npatchback-fft = { path = \"../x\", package = \"rustfft\" }\n",
        "[dependencies.\"rand\"]\nversion = \"0.8\"\n",
    ];
    for manifest in refused {
        assert_eq!(violations(manifest, &workspace).len(), 1, "{manifest}");
    }
    let admitted = [
        "[package]\nname = \"x\"\n",
        "[dependencies]\npatchback-core = { path = \"../patchback-core\" }\n",
        "[dependencies]\npatchback-core.workspace = true\n",
        "[dev-dependencies]\npatchback-core = { workspace = true }\n",
        "[dependencies.patchback-core]\npath = \"../patchback-core\"\n",
    ];
    for manifest in admitted {
        assert!(violations(manifest, &workspace).is_empty(), "{manifest}");
    }
}
