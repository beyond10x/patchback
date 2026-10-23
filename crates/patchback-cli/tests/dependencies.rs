//! DESIGN.md § 3: the application crates take no third-party crate.
//!
//! The check reads what cargo resolved, not what a manifest says: it walks `Cargo.lock` from
//! each application crate through every `dependencies = [...]` entry (normal, dev, build and
//! every target alike, which is how the lock file records them) and refuses any package it
//! reaches that has a `source` (registry, git) or whose name is not `patchback-*`, and
//! `patchback-docs`, which is tooling and takes third-party crates. A `package =` rename, a
//! dotted key, a `[target.<cfg>]` table or a comment cannot hide a crate from the lock file.
//! Every directory under `crates/` is either an application crate or `patchback-docs`, so a
//! new crate cannot be left out of the walk. Read with `std` only.
//!
//! The manifest cases below run through cargo itself: each manifest is resolved offline in a
//! scratch workspace whose crates.io is replaced by a local directory of empty fixture crates,
//! and the resulting `Cargo.lock` goes through the same walk.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The application crates of DESIGN.md § 4.1.
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

/// Documentation tooling: a workspace crate that is not an application crate and that no
/// application crate may reach.
const DOCS_CRATE: &str = "patchback-docs";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels under the repository root")
        .to_path_buf()
}

fn unquote(key: &str) -> String {
    key.trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .to_owned()
}

/// One `[[package]]` record of a `Cargo.lock`.
#[derive(Debug, Default)]
struct Package {
    name: String,
    version: String,
    source: Option<String>,
    dependencies: Vec<String>,
}

/// The `[[package]]` records of `lock`.
fn packages(lock: &str) -> Vec<Package> {
    let mut found: Vec<Package> = Vec::new();
    let mut list: Option<String> = None;
    for raw in lock.lines() {
        let line = raw.trim();
        if let Some(open) = list.as_mut() {
            open.push_str(line);
            if line.ends_with(']') {
                let text = list.take().unwrap_or_default();
                if let Some(package) = found.last_mut() {
                    package.dependencies = entries(&text);
                }
            }
            continue;
        }
        if line == "[[package]]" {
            found.push(Package::default());
            continue;
        }
        if line.starts_with('[') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let Some(package) = found.last_mut() else {
            continue;
        };
        match key.trim() {
            "name" => package.name = unquote(value),
            "version" => package.version = unquote(value),
            "source" => package.source = Some(unquote(value)),
            "dependencies" => {
                let value = value.trim();
                if value.ends_with(']') {
                    package.dependencies = entries(value);
                } else {
                    list = Some(value.to_owned());
                }
            }
            _ => {}
        }
    }
    found
}

/// The quoted strings of a lock-file array, `["a", "b 1.0"]`.
fn entries(array: &str) -> Vec<String> {
    array
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

/// Everything the walk from `roots` through `lock` reaches that is not a workspace-local
/// `patchback-*` crate, plus every root or entry the lock file does not resolve.
fn lock_violations(lock: &str, roots: &[&str]) -> Vec<String> {
    let packages = packages(lock);
    let mut found = Vec::new();
    let mut queue: Vec<(usize, bool)> = Vec::new();
    for root in roots {
        let matching: Vec<usize> = (0..packages.len())
            .filter(|&i| packages[i].name == *root && packages[i].source.is_none())
            .collect();
        match matching.as_slice() {
            [i] => queue.push((*i, true)),
            _ => found.push(format!(
                "{root}: not exactly one workspace package of that name in Cargo.lock"
            )),
        }
    }
    let mut seen = BTreeSet::new();
    while let Some((index, is_root)) = queue.pop() {
        if !seen.insert(index) {
            continue;
        }
        let package = &packages[index];
        let label = format!("{} {}", package.name, package.version);
        if let Some(source) = &package.source {
            found.push(format!("{label}: comes from {source}"));
        } else if !is_root && !package.name.starts_with("patchback-") {
            found.push(format!("{label}: not a patchback-* crate"));
        } else if package.name == DOCS_CRATE {
            found.push(format!("{label}: documentation tooling"));
        }
        for entry in &package.dependencies {
            let mut words = entry.split_whitespace();
            let name = words.next().unwrap_or_default();
            let version = words.next();
            let target: Vec<usize> = (0..packages.len())
                .filter(|&i| {
                    packages[i].name == name && version.is_none_or(|v| packages[i].version == v)
                })
                .collect();
            match target.as_slice() {
                [i] => queue.push((*i, false)),
                _ => found.push(format!("{label}: unresolved dependency entry '{entry}'")),
            }
        }
    }
    found
}

/// The directories under `crates/` that are neither an application crate nor the docs crate.
fn unclassified<'a>(directories: &[&'a str]) -> Vec<&'a str> {
    directories
        .iter()
        .copied()
        .filter(|name| *name != DOCS_CRATE && !APPLICATION_CRATES.contains(name))
        .collect()
}

#[test]
fn the_application_crates_take_no_third_party_crate() {
    let root = repository_root();
    let lock = fs::read_to_string(root.join("Cargo.lock")).expect("root Cargo.lock");
    let found = lock_violations(&lock, APPLICATION_CRATES);
    assert!(found.is_empty(), "third-party or missing: {found:#?}");
}

#[test]
fn every_crate_directory_is_an_application_crate_or_the_docs_crate() {
    let root = repository_root();
    let mut names: Vec<String> = Vec::new();
    for entry in fs::read_dir(root.join("crates")).expect("crates/") {
        let entry = entry.expect("a crates/ entry");
        if entry.path().join("Cargo.toml").is_file() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    assert!(names.contains(&DOCS_CRATE), "{names:?}");
    for name in APPLICATION_CRATES {
        assert!(names.contains(name), "{name} in {names:?}");
    }
    assert_eq!(unclassified(&names), Vec::<&str>::new());
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
    for name in APPLICATION_CRATES.iter().chain(&[DOCS_CRATE]) {
        let member = format!("crates/{name}");
        assert!(members.contains(&member), "{member} in {members:?}");
    }
}

#[test]
fn a_crate_directory_outside_both_lists_is_refused() {
    assert_eq!(
        unclassified(&["patchback-core", "patchback-docs", "patchback-extra", "fft"]),
        ["patchback-extra", "fft"]
    );
    assert!(unclassified(APPLICATION_CRATES).is_empty());
}

const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";

/// One `[[package]]` record to write: name, version, source, dependency entries.
type Record<'a> = (&'a str, &'a str, Option<&'a str>, &'a [&'a str]);

/// A lock file of `records`.
fn lock_of(records: &[Record]) -> String {
    let mut lock = String::from("version = 4\n");
    for (name, version, source, dependencies) in records {
        lock.push_str(&format!(
            "\n[[package]]\nname = \"{name}\"\nversion = \"{version}\"\n"
        ));
        if let Some(source) = source {
            lock.push_str(&format!("source = \"{source}\"\n"));
        }
        if !dependencies.is_empty() {
            lock.push_str("dependencies = [\n");
            for dependency in *dependencies {
                lock.push_str(&format!(" \"{dependency}\",\n"));
            }
            lock.push_str("]\n");
        }
    }
    lock
}

#[test]
fn the_lock_walk_refuses_every_kind_of_foreign_package() {
    let refused: &[&[Record]] = &[
        // a registry crate
        &[
            ("patchback-cli", "0.0.0", None, &["rand"]),
            ("rand", "0.8.5", Some(REGISTRY), &[]),
        ],
        // a git crate under a patchback name
        &[
            ("patchback-cli", "0.0.0", None, &["patchback-core"]),
            (
                "patchback-core",
                "0.0.0",
                Some("git+https://example.invalid/x#0"),
                &[],
            ),
        ],
        // a registry crate under a patchback name
        &[
            ("patchback-cli", "0.0.0", None, &["patchback-core"]),
            ("patchback-core", "0.1.0", Some(REGISTRY), &[]),
        ],
        // a path crate that is not patchback-* (a `package =` rename resolves to its real name)
        &[
            ("patchback-cli", "0.0.0", None, &["rustfft"]),
            ("rustfft", "6.0.0", None, &[]),
        ],
        // a third-party crate two hops away, through a patchback path crate
        &[
            ("patchback-cli", "0.0.0", None, &["patchback-core"]),
            ("patchback-core", "0.0.0", None, &["libc 0.2.0"]),
            ("libc", "0.2.0", Some(REGISTRY), &[]),
        ],
        // a version-and-source qualified entry
        &[
            (
                "patchback-cli",
                "0.0.0",
                None,
                &[&*format!("rand 0.8.5 ({REGISTRY})")],
            ),
            ("rand", "0.8.5", Some(REGISTRY), &[]),
        ],
        // an entry the lock does not resolve
        &[("patchback-cli", "0.0.0", None, &["ghost"])],
    ];
    for records in refused {
        let lock = lock_of(records);
        assert_eq!(
            lock_violations(&lock, &["patchback-cli"]).len(),
            1,
            "{lock}"
        );
    }

    // patchback-docs reached from an application crate, and the two crates it takes
    let lock = lock_of(&[
        ("patchback-cli", "0.0.0", None, &["patchback-docs"]),
        ("patchback-docs", "0.0.0", None, &["clap", "serde_json"]),
        ("clap", "4.6.0", Some(REGISTRY), &[]),
        ("serde_json", "1.0.0", Some(REGISTRY), &[]),
    ]);
    assert_eq!(
        lock_violations(&lock, &["patchback-cli"]).len(),
        3,
        "{lock}"
    );

    // a root the lock file does not hold
    let lock = lock_of(&[("patchback-cli", "0.0.0", None, &[])]);
    assert_eq!(
        lock_violations(&lock, &["patchback-cli", "patchback-fit"]).len(),
        1,
        "{lock}"
    );

    let admitted = lock_of(&[
        (
            "patchback-cli",
            "0.0.0",
            None,
            &["patchback-core", "patchback-dsp 0.0.0"],
        ),
        ("patchback-core", "0.0.0", None, &[]),
        ("patchback-dsp", "0.0.0", None, &["patchback-core"]),
        ("patchback-docs", "0.0.0", None, &["clap"]),
        ("clap", "4.6.0", Some(REGISTRY), &[]),
    ]);
    assert_eq!(
        lock_violations(&admitted, &["patchback-cli"]),
        Vec::<String>::new()
    );
}

/// The registry the manifest cases resolve against: empty crates standing in for crates.io.
const FIXTURE_REGISTRY: &[(&str, &str)] = &[
    ("clap", "4.0.0"),
    ("serde", "1.0.0"),
    ("serde_json", "1.0.0"),
    ("cc", "1.0.0"),
    ("libc", "0.2.0"),
    ("rand", "0.8.0"),
    ("patchback-core", "0.0.0"),
    ("patchback-core", "0.1.0"),
];

/// Local path crates the manifest cases name, `(directory, manifest)`.
const FIXTURE_PATHS: &[(&str, &str)] = &[
    (
        "crates/patchback-core",
        "[package]\nname = \"patchback-core\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    ),
    // Stands in for the real docs crate, which takes clap and serde_json.
    (
        "crates/patchback-docs",
        "[package]\nname = \"patchback-docs\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
         [dependencies]\nclap = \"4\"\nserde_json = \"1\"\n",
    ),
    (
        "crates/local-thing",
        "[package]\nname = \"local-thing\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    ),
    (
        "crates/x",
        "[package]\nname = \"rustfft\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    ),
    (
        "vendor/rustfft",
        "[package]\nname = \"rustfft\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    ),
];

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("create a fixture directory");
    fs::write(path, text).expect("write a fixture file");
}

/// The `[workspace.dependencies]` entries of a root manifest, by name, as written. Only used to
/// build the scratch workspace; cargo reads them, the check does not.
fn workspace_dependencies(root_manifest: &str) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    let mut inside = false;
    for line in root_manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == "[workspace.dependencies]";
            continue;
        }
        if let (true, Some((name, spec))) = (inside, line.split_once('=')) {
            found.insert(name.trim().to_owned(), spec.trim().to_owned());
        }
    }
    found
}

/// Resolves `manifest` with cargo, offline, as the one member `crates/member` of a scratch
/// workspace whose `[workspace.dependencies]` are `workspace`, and returns its `Cargo.lock`,
/// or cargo's error.
fn resolve(manifest: &str, workspace: &BTreeMap<String, String>) -> Result<String, String> {
    static CASE: AtomicUsize = AtomicUsize::new(0);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "dependency-case-{}-{}",
        std::process::id(),
        CASE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut root = String::from(
        "[workspace]\nmembers = [\"crates/member\"]\nresolver = \"3\"\n\n\
         [workspace.dependencies]\n",
    );
    for (name, spec) in workspace {
        root.push_str(&format!("{name} = {spec}\n"));
    }
    write(&dir.join("Cargo.toml"), &root);
    let member = if manifest.contains("[package]") {
        manifest.to_owned()
    } else {
        format!("[package]\nname = \"x\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n{manifest}")
    };
    write(&dir.join("crates/member/Cargo.toml"), &member);
    write(&dir.join("crates/member/src/lib.rs"), "");
    for (path, manifest) in FIXTURE_PATHS {
        write(&dir.join(path).join("Cargo.toml"), manifest);
        write(&dir.join(path).join("src/lib.rs"), "");
    }
    for (name, version) in FIXTURE_REGISTRY {
        let crate_dir = dir.join("registry").join(format!("{name}-{version}"));
        write(
            &crate_dir.join("Cargo.toml"),
            &format!("[package]\nname = \"{name}\"\nversion = \"{version}\"\nedition = \"2021\"\n"),
        );
        write(&crate_dir.join("src/lib.rs"), "");
        write(
            &crate_dir.join(".cargo-checksum.json"),
            "{\"files\":{},\"package\":null}",
        );
    }
    write(
        &dir.join(".cargo/config.toml"),
        "[source.crates-io]\nreplace-with = \"fixture\"\n\n\
         [source.fixture]\ndirectory = \"registry\"\n",
    );
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let out = Command::new(cargo)
        .args(["generate-lockfile", "--offline"])
        .current_dir(&dir)
        .env_remove("CARGO_TARGET_DIR")
        .output()
        .expect("cargo runs");
    let result = if out.status.success() {
        Ok(fs::read_to_string(dir.join("Cargo.lock")).expect("cargo wrote Cargo.lock"))
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    };
    fs::remove_dir_all(&dir).expect("remove the scratch workspace");
    result
}

/// What the walk refuses in the lock file cargo resolves `manifest` to, from its package.
/// Panics when cargo cannot resolve it, so an unresolvable case never reads as refused.
fn violations(manifest: &str, workspace: &BTreeMap<String, String>) -> Vec<String> {
    match resolve(manifest, workspace) {
        Ok(lock) => lock_violations(&lock, &["x"]),
        Err(e) => panic!("cargo could not resolve\n{manifest}\n{e}"),
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
        "[target.'cfg(windows)'.dependencies]\nlibc = \"0.2\"\n",
        "[target.\"x86_64-unknown-linux-gnu\".dev-dependencies]\nlibc = \"0.2\"\n",
        "[dependencies.rand]\nversion = \"0.8\"\n",
        "[dependencies]\npatchback-core = \"0.0.0\"\n",
        "[dependencies]\nlocal-thing = { path = \"../local-thing\" }\n",
        "[dependencies]\npatchback-fft = { path = \"../x\", package = \"rustfft\" }\n",
        "[dependencies.\"rand\"]\nversion = \"0.8\"\n",
    ];
    for manifest in refused {
        assert_eq!(violations(manifest, &workspace).len(), 1, "{manifest}");
    }
    // Cargo itself refuses these, so no lock file can carry them; a git source that did
    // resolve is refused by the lock walk (`the_lock_walk_refuses_every_kind_of_foreign_package`).
    let unresolvable = [
        "[dependencies]\npatchback-core = { git = \"https://example.invalid/x\" }\n",
        "[dependencies]\npatchback-dsp.workspace = true\n",
    ];
    for manifest in unresolvable {
        assert!(resolve(manifest, &workspace).is_err(), "{manifest}");
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

// Adversary cases (wave a1). Each manifest below is one `cargo metadata --no-deps` reads as a
// registry dependency, so the check must refuse it.

fn assert_refused(manifest: &str, workspace: &BTreeMap<String, String>) {
    assert!(
        !violations(manifest, workspace).is_empty(),
        "a third-party crate was admitted:\n{manifest}"
    );
}

#[test]
fn adversary_refuses_a_dotted_dependencies_key_before_any_table() {
    assert_refused(
        "dependencies.rand = \"0.8\"\n\n[package]\nname = \"x\"\n",
        &BTreeMap::new(),
    );
}

#[test]
fn adversary_refuses_a_dependencies_key_inside_a_target_table() {
    assert_refused(
        "[package]\nname = \"x\"\n\n[target.'cfg(unix)']\ndependencies = { libc = \"0.2\" }\n",
        &BTreeMap::new(),
    );
}

#[test]
fn adversary_refuses_a_registry_crate_whose_trailing_comment_mentions_path() {
    assert_refused(
        "[dependencies]\npatchback-core = \"0.1\" # path = \"../patchback-core\"\n",
        &BTreeMap::new(),
    );
}

#[test]
fn adversary_refuses_a_workspace_entry_that_renames_a_third_party_crate() {
    let workspace = workspace_dependencies(
        "[workspace.dependencies]\n\
         patchback-fft = { path = \"vendor/rustfft\", package = \"rustfft\" }\n",
    );
    assert_refused(
        "[dependencies]\npatchback-fft.workspace = true\n",
        &workspace,
    );
}

#[test]
fn adversary_refuses_a_patchback_crate_that_itself_takes_third_party_crates() {
    // patchback-docs is a workspace `patchback-*` path crate that depends on clap and
    // serde_json; reaching it from an application crate brings both into the build.
    assert_refused(
        "[dependencies]\npatchback-docs = { path = \"../patchback-docs\" }\n",
        &BTreeMap::new(),
    );
}
