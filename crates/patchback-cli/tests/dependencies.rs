//! DESIGN.md § 3: the application crates take no third-party crate but `clap`, the one reviewed
//! command-line crate, and the reviewed packages of its resolved closure (`CLAP_CLOSURE`).
//!
//! The check reads what cargo resolved, not what a manifest says: it walks `Cargo.lock` from
//! each application crate through every `dependencies = [...]` entry (normal, dev, build and
//! every target alike, which is how the lock file records them) and refuses any package it
//! reaches that has a `source` (registry, git) and is not a crates.io package whose exact
//! `(name, version)` is in `CLAP_CLOSURE`,
//! a workspace crate that takes a closure member other than `clap` directly, any path package
//! whose name is not `patchback-*`, and `patchback-docs`, which is tooling and takes
//! third-party crates (its `serde_json` stays refused). A `package =` rename, a
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

/// The one reviewed command-line crate (DESIGN.md § 3, "CLI parsing"). A workspace crate may
/// take it from crates.io directly.
const CLI_PARSER: &str = "clap";

/// The resolved closure of `clap` (with `derive`) as `Cargo.lock` holds it, reviewed in wave a2
/// (story:cli-clap): `clap` and every crates.io package it brings in, each as the exact
/// `(name, version)` the review saw. A registry package is admitted only when its exact pair is
/// listed here, only from crates.io, and only through `clap` or another member of this list,
/// never taken directly by a workspace crate. A package that enters the closure later, another
/// version of a listed package (in place of or beside the reviewed one) and a `clap` the review
/// did not see are each refused until they are reviewed and listed here;
/// `the_clap_allow_list_is_exactly_its_resolved_closure` keeps the list exact.
const CLAP_CLOSURE: &[(&str, &str)] = &[
    ("anstream", "1.0.0"),
    ("anstyle", "1.0.14"),
    ("anstyle-parse", "1.0.0"),
    ("anstyle-query", "1.1.5"),
    ("anstyle-wincon", "3.0.11"),
    ("clap", "4.6.7"),
    ("clap_builder", "4.6.7"),
    ("clap_derive", "4.6.7"),
    ("clap_lex", "1.1.1"),
    ("colorchoice", "1.0.5"),
    ("heck", "0.5.0"),
    ("is_terminal_polyfill", "1.70.2"),
    ("once_cell_polyfill", "1.70.2"),
    ("proc-macro2", "1.0.107"),
    ("quote", "1.0.47"),
    ("strsim", "0.11.1"),
    ("syn", "3.0.9"),
    ("unicode-ident", "1.0.27"),
    ("utf8parse", "0.2.2"),
    ("windows-link", "0.2.1"),
    ("windows-sys", "0.61.2"),
];

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

/// Whether `package` is a reviewed member of `clap`'s closure, from crates.io, at the exact
/// version the review saw.
fn reviewed(package: &Package) -> bool {
    package.source.as_deref() == Some(REGISTRY)
        && CLAP_CLOSURE
            .iter()
            .any(|&(name, version)| package.name == name && package.version == version)
}

/// Why the registry package `package`, which is not `reviewed`, is refused.
fn refusal(package: &Package, source: &str) -> String {
    let label = format!("{} {}", package.name, package.version);
    let versions: Vec<&str> = CLAP_CLOSURE
        .iter()
        .filter(|&&(name, _)| name == package.name)
        .map(|&(_, version)| version)
        .collect();
    if source == REGISTRY && !versions.is_empty() {
        format!(
            "{label}: version not in {CLI_PARSER}'s reviewed closure (reviewed: {} {})",
            package.name,
            versions.join(", ")
        )
    } else {
        format!("{label}: comes from {source}")
    }
}

/// Everything the walk from `roots` through `lock` reaches that is neither a workspace-local
/// `patchback-*` crate nor a reviewed member of `clap`'s closure, every workspace crate that
/// takes a closure member other than `clap` directly, and every root or entry the lock file
/// does not resolve.
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
            if !reviewed(package) {
                found.push(refusal(package, source));
            }
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
                [i] => {
                    let dependency = &packages[*i];
                    if package.source.is_none()
                        && reviewed(dependency)
                        && dependency.name != CLI_PARSER
                    {
                        found.push(format!(
                            "{label}: takes {} {} directly; it is reviewed only inside {CLI_PARSER}'s closure",
                            dependency.name, dependency.version
                        ));
                    }
                    queue.push((*i, false));
                }
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

fn repository_lock() -> String {
    fs::read_to_string(repository_root().join("Cargo.lock")).expect("root Cargo.lock")
}

#[test]
fn the_application_crates_take_no_third_party_crate_but_clap_and_its_closure() {
    let found = lock_violations(&repository_lock(), APPLICATION_CRATES);
    assert!(found.is_empty(), "third-party or missing: {found:#?}");
}

#[test]
fn patchback_cli_takes_clap_from_crates_io() {
    let lock = repository_lock();
    let packages = packages(&lock);
    let cli = packages
        .iter()
        .find(|p| p.name == "patchback-cli" && p.source.is_none())
        .expect("patchback-cli in Cargo.lock");
    assert!(
        cli.dependencies
            .iter()
            .any(|entry| entry.split_whitespace().next() == Some(CLI_PARSER)),
        "{:?}",
        cli.dependencies
    );
    let clap: Vec<&Package> = packages.iter().filter(|p| p.name == CLI_PARSER).collect();
    assert_eq!(clap.len(), 1, "one clap in Cargo.lock");
    assert_eq!(clap[0].source.as_deref(), Some(REGISTRY));
}

/// The `(name, version)` of every package `Cargo.lock` reaches from the registry packages named
/// `name` (every version of it), themselves included. Each dependency entry resolves to exactly
/// one package, as in `lock_violations`.
fn registry_closure(lock: &str, name: &str) -> BTreeSet<(String, String)> {
    let packages = packages(lock);
    let mut found = BTreeSet::new();
    let mut queue: Vec<usize> = (0..packages.len())
        .filter(|&i| packages[i].name == name)
        .collect();
    while let Some(index) = queue.pop() {
        let package = &packages[index];
        if !found.insert((package.name.clone(), package.version.clone())) {
            continue;
        }
        assert!(
            package.source.is_some(),
            "{} {} is a registry package",
            package.name,
            package.version
        );
        for entry in &package.dependencies {
            let mut words = entry.split_whitespace();
            let dependency = words.next().unwrap_or_default();
            let version = words.next();
            let target: Vec<usize> = (0..packages.len())
                .filter(|&i| {
                    packages[i].name == dependency
                        && version.is_none_or(|v| packages[i].version == v)
                })
                .collect();
            match target.as_slice() {
                [i] => queue.push(*i),
                _ => panic!(
                    "{} {}: unresolved dependency entry '{entry}'",
                    package.name, package.version
                ),
            }
        }
    }
    found
}

#[test]
fn the_clap_allow_list_is_exactly_its_resolved_closure() {
    let resolved = registry_closure(&repository_lock(), CLI_PARSER);
    let reviewed: BTreeSet<(String, String)> = CLAP_CLOSURE
        .iter()
        .map(|&(name, version)| (name.to_owned(), version.to_owned()))
        .collect();
    assert_eq!(
        reviewed.len(),
        CLAP_CLOSURE.len(),
        "CLAP_CLOSURE lists a pair twice"
    );
    assert_eq!(
        resolved, reviewed,
        "clap's closure in Cargo.lock changed; review the difference and update CLAP_CLOSURE"
    );
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

    // patchback-docs reached from an application crate, and serde_json, which it takes; the
    // clap it also takes is the reviewed CLI parser, so it is not a second finding
    let lock = lock_of(&[
        ("patchback-cli", "0.0.0", None, &["patchback-docs"]),
        ("patchback-docs", "0.0.0", None, &["clap", "serde_json"]),
        ("clap", "4.6.7", Some(REGISTRY), &[]),
        ("serde_json", "1.0.0", Some(REGISTRY), &[]),
    ]);
    let found = lock_violations(&lock, &["patchback-cli"]);
    assert_eq!(found.len(), 2, "{lock}");
    assert!(
        found.iter().any(|f| f.starts_with("patchback-docs ")),
        "{found:?}"
    );
    assert!(
        found.iter().any(|f| f.starts_with("serde_json ")),
        "{found:?}"
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
        ("clap", "4.6.7", Some(REGISTRY), &[]),
    ]);
    assert_eq!(
        lock_violations(&admitted, &["patchback-cli"]),
        Vec::<String>::new()
    );
}

/// `patchback-cli` taking crates.io `clap`, which takes `extra` beside the reviewed packages.
fn clap_lock(clap_source: &str, extra: &[Record]) -> String {
    let mut clap_dependencies = vec!["clap_builder", "clap_derive"];
    clap_dependencies.extend(extra.iter().map(|(name, ..)| *name));
    let mut records: Vec<Record> = vec![
        ("patchback-cli", "0.0.0", None, &["clap"]),
        (
            "clap",
            "4.6.7",
            Some(clap_source),
            clap_dependencies.as_slice(),
        ),
        (
            "clap_builder",
            "4.6.7",
            Some(REGISTRY),
            &["anstream", "anstyle", "clap_lex", "strsim"],
        ),
        (
            "clap_derive",
            "4.6.7",
            Some(REGISTRY),
            &["heck", "proc-macro2", "quote", "syn"],
        ),
        (
            "anstream",
            "1.0.0",
            Some(REGISTRY),
            &["anstyle", "utf8parse"],
        ),
        ("anstyle", "1.0.14", Some(REGISTRY), &[]),
        ("clap_lex", "1.1.1", Some(REGISTRY), &[]),
        ("strsim", "0.11.1", Some(REGISTRY), &[]),
        ("heck", "0.5.0", Some(REGISTRY), &[]),
        ("proc-macro2", "1.0.107", Some(REGISTRY), &["unicode-ident"]),
        ("quote", "1.0.47", Some(REGISTRY), &["proc-macro2"]),
        (
            "syn",
            "3.0.9",
            Some(REGISTRY),
            &["proc-macro2", "quote", "unicode-ident"],
        ),
        ("unicode-ident", "1.0.27", Some(REGISTRY), &[]),
        ("utf8parse", "0.2.2", Some(REGISTRY), &[]),
    ];
    records.extend_from_slice(extra);
    lock_of(&records)
}

#[test]
fn the_lock_walk_admits_clap_and_its_reviewed_closure() {
    let lock = clap_lock(REGISTRY, &[]);
    assert_eq!(
        lock_violations(&lock, &["patchback-cli"]),
        Vec::<String>::new(),
        "{lock}"
    );
}

#[test]
fn a_package_that_enters_clap_closure_unreviewed_is_refused() {
    // a stand-in clap that has started to depend on a stand-in rand
    let lock = clap_lock(REGISTRY, &[("rand", "0.8.5", Some(REGISTRY), &[])]);
    let found = lock_violations(&lock, &["patchback-cli"]);
    assert_eq!(found.len(), 1, "{lock}");
    assert!(found[0].starts_with("rand 0.8.5"), "{found:?}");
}

#[test]
fn every_closure_package_is_admitted_only_at_its_reviewed_version() {
    // For each reviewed pair, a lock that reaches that package through clap: admitted at the
    // reviewed version, refused at any other, in place of the reviewed one and beside it.
    for &(name, version) in CLAP_CLOSURE {
        let other = format!("{version}-unreviewed");
        let lock_at = |versions: &[&str]| {
            let mut records: Vec<Record> = Vec::new();
            let qualified: Vec<String> = versions.iter().map(|v| format!("{name} {v}")).collect();
            let entries: Vec<&str> = qualified.iter().map(String::as_str).collect();
            if name == CLI_PARSER {
                records.push(("patchback-cli", "0.0.0", None, entries.as_slice()));
                for v in versions {
                    records.push((name, *v, Some(REGISTRY), &[]));
                }
                lock_of(&records)
            } else {
                records.push(("patchback-cli", "0.0.0", None, &["clap"]));
                records.push(("clap", "4.6.7", Some(REGISTRY), entries.as_slice()));
                for v in versions {
                    records.push((name, *v, Some(REGISTRY), &[]));
                }
                lock_of(&records)
            }
        };
        let admitted = lock_at(&[version]);
        assert_eq!(
            lock_violations(&admitted, &["patchback-cli"]),
            Vec::<String>::new(),
            "{admitted}"
        );
        for versions in [vec![other.as_str()], vec![version, other.as_str()]] {
            let lock = lock_at(&versions);
            let found = lock_violations(&lock, &["patchback-cli"]);
            let expected = format!("{name} {other}: version not in clap's reviewed closure");
            assert_eq!(found.len(), 1, "{lock}\n{found:?}");
            assert!(found[0].starts_with(&expected), "{found:?}");
        }
    }
}

#[test]
fn the_lock_walk_refuses_a_clap_that_is_not_from_crates_io() {
    for source in [
        "git+https://example.invalid/clap#0",
        "registry+https://example.invalid/index",
    ] {
        let found = lock_violations(&clap_lock(source, &[]), &["patchback-cli"]);
        assert_eq!(found.len(), 1, "{source}: {found:?}");
        assert!(found[0].starts_with("clap 4.6.7"), "{found:?}");
    }
}

#[test]
fn the_lock_walk_refuses_a_closure_package_taken_directly() {
    // syn is reviewed only as part of clap's closure, not as a crate of Patchback's own
    let lock = lock_of(&[
        ("patchback-cli", "0.0.0", None, &["syn"]),
        ("syn", "3.0.9", Some(REGISTRY), &[]),
    ]);
    assert_eq!(
        lock_violations(&lock, &["patchback-cli"]).len(),
        1,
        "{lock}"
    );
    // also when the same crate is reached through clap as well
    let lock = lock_of(&[
        ("patchback-cli", "0.0.0", None, &["clap", "syn"]),
        ("clap", "4.6.7", Some(REGISTRY), &["syn"]),
        ("syn", "3.0.9", Some(REGISTRY), &[]),
    ]);
    assert_eq!(
        lock_violations(&lock, &["patchback-cli"]).len(),
        1,
        "{lock}"
    );
}

#[test]
fn the_lock_walk_refuses_another_cli_parser() {
    let lock = lock_of(&[
        ("patchback-cli", "0.0.0", None, &["argh"]),
        ("argh", "0.1.12", Some(REGISTRY), &[]),
    ]);
    assert_eq!(
        lock_violations(&lock, &["patchback-cli"]).len(),
        1,
        "{lock}"
    );
}

/// The registry the manifest cases resolve against: empty crates standing in for crates.io,
/// as `(name, version, [dependencies])`.
const FIXTURE_REGISTRY: &[(&str, &str, &str)] = &[
    ("clap", "4.6.7", ""),
    // a later clap whose closure has grown by a crate nobody reviewed
    ("clap", "5.0.0", "rand = \"0.8\"\n"),
    ("serde", "1.0.0", ""),
    ("serde_json", "1.0.0", ""),
    ("cc", "1.0.0", ""),
    ("libc", "0.2.0", ""),
    ("rand", "0.8.0", ""),
    ("patchback-core", "0.0.0", ""),
    ("patchback-core", "0.1.0", ""),
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
    for (name, version, dependencies) in FIXTURE_REGISTRY {
        let crate_dir = dir.join("registry").join(format!("{name}-{version}"));
        write(
            &crate_dir.join("Cargo.toml"),
            &format!(
                "[package]\nname = \"{name}\"\nversion = \"{version}\"\nedition = \"2021\"\n\n\
                 [dependencies]\n{dependencies}"
            ),
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
         rand = { version = \"0.8\" }\n\
         patchback-core = { path = \"crates/patchback-core\" }\n",
    );
    let refused = [
        "[dependencies]\nrand = \"0.8\"\n",
        "[dependencies]\nrand.workspace = true\n",
        "[dependencies]\nrand = { workspace = true }\n",
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
    // A clap the review did not see, whose closure has also grown by an unreviewed crate: both
    // the clap version and the new crate are refused.
    let mut found = violations("[dependencies]\nclap = \"5\"\n", &workspace);
    found.sort();
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found[0].starts_with("clap 5.0.0: version not in"),
        "{found:?}"
    );
    assert!(found[1].starts_with("rand 0.8.0: comes from"), "{found:?}");
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
        "[dependencies]\nclap = \"4\"\n",
        "[dependencies]\nclap.workspace = true\n",
        "[dependencies]\nclap = { workspace = true }\n",
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

// Adversary cases (wave a2, story:cli-clap). DESIGN.md § 3 (0.4): "the `Cargo.lock` gate admits
// `clap` and exactly the reviewed packages of its resolved closure, and a package that later
// enters that closure fails the gate until it is reviewed". A `[[package]]` of `Cargo.lock` is a
// name and a version; the review of wave a2 saw the versions the lock holds now.

/// The `(name, version)` of every registry package of the repository's `Cargo.lock`, which is
/// the closure wave a2 reviewed.
fn reviewed_lock_versions() -> BTreeSet<(String, String)> {
    packages(&repository_lock())
        .into_iter()
        .filter(|p| p.source.is_some())
        .map(|p| (p.name, p.version))
        .collect()
}

#[test]
fn adversary_a2_a_second_version_of_a_closure_package_entering_is_refused() {
    // clap_builder starts to take an old `syn 1.0.109` beside the reviewed syn: a package that
    // was not in the reviewed lock has entered the closure under a reviewed name.
    // With two versions present, cargo qualifies every `syn` entry with its version.
    let lock = clap_lock(REGISTRY, &[])
        .replace(
            "name = \"clap_builder\"\nversion = \"4.6.7\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\ndependencies = [\n \"anstream\",",
            "name = \"clap_builder\"\nversion = \"4.6.7\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\ndependencies = [\n \"syn 1.0.109\",\n \"anstream\",",
        )
        .replace(" \"quote\",\n \"syn\",\n", " \"quote\",\n \"syn 3.0.9\",\n")
        + "\n[[package]]\nname = \"syn\"\nversion = \"1.0.109\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n";
    assert!(
        lock.contains("\"syn 1.0.109\""),
        "fixture edit applied: {lock}"
    );
    assert!(
        lock.contains("\"syn 3.0.9\""),
        "fixture edit applied: {lock}"
    );
    let packages = packages(&lock);
    assert_eq!(packages.iter().filter(|p| p.name == "syn").count(), 2);
    let found = lock_violations(&lock, &["patchback-cli"]);
    assert!(
        found.iter().any(|f| f.starts_with("syn 1.0.109")),
        "a second, unreviewed version of syn entered clap's closure and was admitted: {found:?}"
    );
}

#[test]
fn adversary_a2_a_clap_version_outside_the_reviewed_lock_is_refused() {
    // The same closure names at a clap the review never saw: `cargo update -p clap` or a
    // workspace pin moved to "5" changes no name, so nothing in the gate notices.
    let reviewed = reviewed_lock_versions();
    assert!(
        reviewed.contains(&("clap".to_owned(), "4.6.7".to_owned())),
        "{reviewed:?}"
    );
    let lock = clap_lock(REGISTRY, &[]).replace(
        "name = \"clap\"\nversion = \"4.6.7\"",
        "name = \"clap\"\nversion = \"5.0.0\"",
    );
    assert!(lock.contains("version = \"5.0.0\""), "fixture edit applied");
    let found = lock_violations(&lock, &["patchback-cli"]);
    assert!(
        found.iter().any(|f| f.starts_with("clap 5.0.0")),
        "clap 5.0.0, not the reviewed 4.6.7, was admitted: {found:?}"
    );
}
