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
//! The same walk holds the rest of DESIGN.md § 3 and § 12.2. It judges the full resolved
//! closure of the application crates, so a transitive build requirement is reviewed by name and
//! version like any other. At most one approved audio I/O backend (`AUDIO_BACKENDS`, empty
//! today; ADR-007) is admitted, with its own exact reviewed closure, and only when every
//! workspace manifest that takes it declares it `optional = true` and no default or
//! always-requested feature turns it on; those manifest facts come from `cargo metadata
//! --no-deps`. No `modelspec*` package is admitted: ModelSpec runs as a subprocess (§ 8.2).
//! `task dependency-audit` runs this file alone, and each refusal is one line naming the
//! package and its reason.
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

/// A reviewed audio I/O backend (DESIGN.md § 3, ADR-007): the crate at the exact version the
/// review saw, the approval record that admits it, and its resolved closure, judged like
/// `CLAP_CLOSURE`.
struct AudioBackend {
    name: &'static str,
    version: &'static str,
    /// The AEP approval record that names the crate, as `approval-record:<slug>`.
    approval: &'static str,
    /// Every crates.io package the backend brings in, as the exact `(name, version)` reviewed.
    closure: &'static [(&'static str, &'static str)],
}

/// The approved audio I/O backends: at most one (DESIGN.md § 3), and none is approved today.
/// An entry needs an approval record in the AEP store that names the crate; its id goes in
/// `approval` (`every_approved_audio_backend_names_its_approval_record` holds that), and the
/// backend's exact resolved closure goes in `closure`
/// (`every_approved_audio_backend_closure_is_exactly_its_resolved_closure`). A listed backend is
/// admitted only when every workspace manifest that takes it declares it `optional = true` and
/// no default or unconditionally requested feature turns it on.
const AUDIO_BACKENDS: &[AudioBackend] = &[];

/// Audio I/O crates known by name, so that an unapproved one is refused as what it is.
const AUDIO_IO_CRATES: &[&str] = &[
    "alsa",
    "asio-sys",
    "coreaudio-rs",
    "coreaudio-sys",
    "cpal",
    "jack",
    "kira",
    "libpulse-binding",
    "oboe",
    "portaudio",
    "portaudio-rs",
    "rodio",
    "sdl2",
    "wasapi",
];

/// ModelSpec runs as a subprocess (DESIGN.md § 8.2), so no package whose name starts with this
/// is admitted, from any source and at any depth.
const MODELSPEC_PREFIX: &str = "modelspec";

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

/// The gate with the approved backends of this repository and no manifest facts.
fn lock_violations(lock: &str, roots: &[&str]) -> Vec<String> {
    audit(lock, &Manifests::new(), roots, AUDIO_BACKENDS)
}

/// Whose reviewed closure a package is judged against.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Owner {
    /// A workspace path crate.
    Workspace,
    /// Inside `clap`'s closure (`CLAP_CLOSURE`).
    Clap,
    /// Inside the closure of the approved backend at this index.
    Backend(usize),
    /// Reached only through a package no review admits.
    Unreviewed,
}

/// The reviewed closures: `clap`'s and those of the approved audio backends.
struct Review<'a> {
    backends: &'a [AudioBackend],
}

impl Review<'_> {
    /// The owners that hold a reviewed closure.
    fn owners(&self) -> Vec<Owner> {
        std::iter::once(Owner::Clap)
            .chain((0..self.backends.len()).map(Owner::Backend))
            .collect()
    }

    /// The `(name, version)` pairs `owner` reviewed, its entry crate included.
    fn pairs(&self, owner: Owner) -> Vec<(&str, &str)> {
        match owner {
            Owner::Clap => CLAP_CLOSURE.to_vec(),
            Owner::Backend(i) => {
                let backend = &self.backends[i];
                std::iter::once((backend.name, backend.version))
                    .chain(backend.closure.iter().copied())
                    .collect()
            }
            Owner::Workspace | Owner::Unreviewed => Vec::new(),
        }
    }

    fn name(&self, owner: Owner) -> &str {
        match owner {
            Owner::Backend(i) => self.backends[i].name,
            _ => CLI_PARSER,
        }
    }

    /// Whether `owner` reviewed `package`, from crates.io, at its exact version.
    fn admits(&self, owner: Owner, package: &Package) -> bool {
        package.source.as_deref() == Some(REGISTRY)
            && self
                .pairs(owner)
                .iter()
                .any(|&(name, version)| package.name == name && package.version == version)
    }

    /// The owner a workspace crate may take `package` from directly: `clap`, or an approved
    /// backend at its approved version.
    fn entry(&self, package: &Package) -> Option<Owner> {
        if package.name == CLI_PARSER && self.admits(Owner::Clap, package) {
            return Some(Owner::Clap);
        }
        (0..self.backends.len())
            .map(Owner::Backend)
            .find(|&owner| package.name == self.name(owner) && self.admits(owner, package))
    }

    /// The first reviewed closure that holds `package`.
    fn containing(&self, package: &Package) -> Option<Owner> {
        self.owners()
            .into_iter()
            .find(|&owner| self.admits(owner, package))
    }

    /// Why the registry or git package `package`, judged as part of `owner`, is refused.
    fn refusal(&self, package: &Package, source: &str, owner: Owner) -> String {
        let label = format!("{} {}", package.name, package.version);
        let owned = matches!(owner, Owner::Clap | Owner::Backend(_));
        if source == REGISTRY {
            let approved: Vec<String> = self
                .backends
                .iter()
                .filter(|b| b.name == package.name)
                .map(|b| format!("{} {}", b.name, b.version))
                .collect();
            if !owned && !approved.is_empty() {
                return format!(
                    "{label}: version not the approved audio I/O backend (approved: {})",
                    approved.join(", ")
                );
            }
            let candidates = if owned { vec![owner] } else { self.owners() };
            for candidate in candidates {
                let versions: Vec<&str> = self
                    .pairs(candidate)
                    .iter()
                    .filter(|&&(name, _)| name == package.name)
                    .map(|&(_, version)| version)
                    .collect();
                if !versions.is_empty() {
                    return format!(
                        "{label}: version not in {}'s reviewed closure (reviewed: {} {})",
                        self.name(candidate),
                        package.name,
                        versions.join(", ")
                    );
                }
            }
        }
        if owned {
            return format!(
                "{label}: entered {}'s closure unreviewed (comes from {source})",
                self.name(owner)
            );
        }
        if AUDIO_IO_CRATES.contains(&package.name.as_str()) {
            let approved: Vec<&str> = self.backends.iter().map(|b| b.name).collect();
            return format!(
                "{label}: an audio I/O crate that is not an approved backend (approved: {}; \
                 an entry needs an approval record naming the crate)",
                if approved.is_empty() {
                    "none".to_owned()
                } else {
                    approved.join(", ")
                }
            );
        }
        format!("{label}: comes from {source}")
    }
}

/// The local names of the optional dependencies that each workspace member turns on whenever
/// it is built: through its default features, the features a dependent always asks of it, and
/// the features those turn on in turn.
fn always_enabled(manifests: &Manifests) -> BTreeMap<String, BTreeSet<String>> {
    let mut items: BTreeMap<String, BTreeSet<String>> = manifests
        .keys()
        .map(|name| (name.clone(), BTreeSet::from(["default".to_owned()])))
        .collect();
    let mut enabled: BTreeMap<String, BTreeSet<String>> = manifests
        .keys()
        .map(|name| (name.clone(), BTreeSet::new()))
        .collect();
    loop {
        let mut changed = false;
        // `dep/feature` requests a member makes of its dependencies
        let mut requests: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        for (name, member) in manifests {
            let mut stack: Vec<String> = items[name].iter().cloned().collect();
            let mut visited = BTreeSet::new();
            while let Some(item) = stack.pop() {
                if !visited.insert(item.clone()) {
                    continue;
                }
                if let Some(dependency) = item.strip_prefix("dep:") {
                    changed |= enabled
                        .get_mut(name)
                        .is_some_and(|e| e.insert(dependency.to_owned()));
                } else if let Some((dependency, feature)) = item.split_once('/') {
                    let (dependency, weak) = match dependency.strip_suffix('?') {
                        Some(d) => (d, true),
                        None => (dependency, false),
                    };
                    if !weak {
                        changed |= enabled
                            .get_mut(name)
                            .is_some_and(|e| e.insert(dependency.to_owned()));
                    }
                    requests
                        .entry(name.clone())
                        .or_default()
                        .push((dependency.to_owned(), feature.to_owned()));
                } else if let Some(entries) = member.features.get(&item) {
                    stack.extend(entries.iter().cloned());
                } else if member
                    .declarations
                    .iter()
                    .any(|d| d.optional && d.local == item)
                {
                    // the implicit feature of an optional dependency
                    changed |= enabled
                        .get_mut(name)
                        .is_some_and(|e| e.insert(item.clone()));
                }
                changed |= items.get_mut(name).is_some_and(|i| i.insert(item));
            }
        }
        for (name, member) in manifests {
            for declaration in &member.declarations {
                if declaration.kind == "dev" || !manifests.contains_key(&declaration.package) {
                    continue;
                }
                if declaration.optional && !enabled[name].contains(&declaration.local) {
                    continue;
                }
                let mut asked: Vec<String> = declaration.features.clone();
                if declaration.default_features {
                    asked.push("default".to_owned());
                }
                for (dependency, feature) in requests.get(name).into_iter().flatten() {
                    if *dependency == declaration.local {
                        asked.push(feature.clone());
                    }
                }
                let target = items
                    .get_mut(&declaration.package)
                    .expect("a workspace member");
                for feature in asked {
                    changed |= target.insert(feature);
                }
            }
        }
        if !changed {
            return enabled;
        }
    }
}

/// The dependency gate: everything the walk from `roots` through `lock` reaches that is not
/// admitted, with its reason, one finding per refused package or crate.
///
/// A workspace path crate must be `patchback-*` and not the docs crate. A workspace crate may
/// take from crates.io only `clap` and an approved audio backend at its approved version; every
/// package reached through one of them must be in that one's reviewed closure at its exact
/// version, and a workspace crate that takes a closure member directly is refused. A
/// `modelspec*` package is refused from any source at any depth (ModelSpec runs as a
/// subprocess, DESIGN.md § 8.2). At most one approved backend may be reached, and every workspace
/// crate that takes it must declare it `optional = true` in every manifest table (`manifests`,
/// from `cargo metadata`) with no default or always-requested feature turning it on.
fn audit(
    lock: &str,
    manifests: &Manifests,
    roots: &[&str],
    backends: &[AudioBackend],
) -> Vec<String> {
    let packages = packages(lock);
    let review = Review { backends };
    let mut found: Vec<String> = Vec::new();
    let push = |found: &mut Vec<String>, finding: String| {
        if !found.contains(&finding) {
            found.push(finding);
        }
    };
    let mut queue: Vec<(usize, Owner)> = Vec::new();
    let mut root_indices = BTreeSet::new();
    for root in roots {
        let matching: Vec<usize> = (0..packages.len())
            .filter(|&i| packages[i].name == *root && packages[i].source.is_none())
            .collect();
        match matching.as_slice() {
            [i] => {
                root_indices.insert(*i);
                queue.push((*i, Owner::Workspace));
            }
            _ => push(
                &mut found,
                format!("{root}: not exactly one workspace package of that name in Cargo.lock"),
            ),
        }
    }
    let mut seen = BTreeSet::new();
    // (workspace crate, backend) for every approved backend a workspace crate takes
    let mut entered: BTreeSet<(usize, usize)> = BTreeSet::new();
    while let Some((index, owner)) = queue.pop() {
        if !seen.insert((index, owner)) {
            continue;
        }
        let package = &packages[index];
        let label = format!("{} {}", package.name, package.version);
        if package.name.starts_with(MODELSPEC_PREFIX) {
            push(
                &mut found,
                format!(
                    "{label}: ModelSpec runs as a subprocess (DESIGN.md § 8.2); no ModelSpec \
                     crate is admitted"
                ),
            );
        } else if let Some(source) = &package.source {
            if !review.admits(owner, package) {
                push(&mut found, review.refusal(package, source, owner));
            }
        } else if !root_indices.contains(&index) && !package.name.starts_with("patchback-") {
            push(&mut found, format!("{label}: not a patchback-* crate"));
        } else if package.name == DOCS_CRATE {
            push(&mut found, format!("{label}: documentation tooling"));
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
            let [i] = target.as_slice() else {
                push(
                    &mut found,
                    format!("{label}: unresolved dependency entry '{entry}'"),
                );
                continue;
            };
            let dependency = &packages[*i];
            let next = if dependency.source.is_none() {
                Owner::Workspace
            } else if package.source.is_none() {
                if let Some(entry_owner) = review.entry(dependency) {
                    if let Owner::Backend(b) = entry_owner {
                        entered.insert((index, b));
                    }
                    entry_owner
                } else if let Some(closure) = review.containing(dependency) {
                    push(
                        &mut found,
                        format!(
                            "{label}: takes {} {} directly; it is reviewed only inside {}'s closure",
                            dependency.name,
                            dependency.version,
                            review.name(closure)
                        ),
                    );
                    closure
                } else {
                    Owner::Unreviewed
                }
            } else if matches!(owner, Owner::Clap | Owner::Backend(_)) {
                owner
            } else {
                review.containing(dependency).unwrap_or(Owner::Unreviewed)
            };
            queue.push((*i, next));
        }
    }

    let reached: BTreeSet<usize> = entered.iter().map(|&(_, b)| b).collect();
    let mut reached = reached.into_iter().map(|b| &backends[b]);
    if let Some(first) = reached.next() {
        for other in reached {
            push(
                &mut found,
                format!(
                    "{} {}: a second audio I/O backend beside {} {}; DESIGN.md § 3 admits at most one",
                    other.name, other.version, first.name, first.version
                ),
            );
        }
    }

    let enabled = always_enabled(manifests);
    for (crate_index, b) in entered {
        let workspace_crate = &packages[crate_index];
        let label = format!("{} {}", workspace_crate.name, workspace_crate.version);
        let backend = &backends[b];
        let what = format!("audio backend {} {}", backend.name, backend.version);
        let declarations: Vec<&Declaration> = manifests
            .get(&workspace_crate.name)
            .map(|member| {
                member
                    .declarations
                    .iter()
                    .filter(|d| d.package == backend.name)
                    .collect()
            })
            .unwrap_or_default();
        if declarations.is_empty() {
            push(
                &mut found,
                format!(
                    "{label}: takes {what}, and no manifest of it was read, so it cannot be shown \
                     optional (DESIGN.md § 3)"
                ),
            );
            continue;
        }
        for declaration in declarations {
            if !declaration.optional {
                push(
                    &mut found,
                    format!(
                        "{label}: takes {what} as a {} dependency without optional = true \
                         (DESIGN.md § 3: behind an optional feature)",
                        declaration.kind
                    ),
                );
            } else if enabled
                .get(&workspace_crate.name)
                .is_some_and(|e| e.contains(&declaration.local))
            {
                push(
                    &mut found,
                    format!(
                        "{label}: its default features or a dependent's required features turn \
                         on {what}, so it is not optional (DESIGN.md § 3: behind an optional \
                         feature)"
                    ),
                );
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

/// The findings, one refused package (or crate) and its reason per line.
fn report(found: &[String]) -> String {
    found.join("\n")
}

#[test]
fn the_application_crates_take_no_third_party_crate_but_clap_and_its_closure() {
    let found = audit(
        &repository_lock(),
        &repository_manifests(),
        APPLICATION_CRATES,
        AUDIO_BACKENDS,
    );
    assert!(
        found.is_empty(),
        "the dependency gate refuses:\n{}",
        report(&found)
    );
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

// Manifest facts. Whether a dependency is optional and which features turn it on is not in
// `Cargo.lock`; it is read from `cargo metadata --no-deps`, which resolves every manifest form
// (inline table, dotted key, `[dependencies.<name>]`, `[target.<cfg>]`, `workspace = true`,
// `package =` renames) the way cargo builds it.

/// One dependency declaration of a workspace member.
#[derive(Debug, Clone)]
struct Declaration {
    /// The package the declaration takes.
    package: String,
    /// The name the member's features use for it: the rename, or the package name.
    local: String,
    /// `normal`, `dev` or `build`.
    kind: String,
    optional: bool,
    default_features: bool,
    features: Vec<String>,
}

/// One workspace member: its dependency declarations and its `[features]` table.
#[derive(Debug, Clone, Default)]
struct Member {
    declarations: Vec<Declaration>,
    features: BTreeMap<String, Vec<String>>,
}

/// The workspace members by package name.
type Manifests = BTreeMap<String, Member>;

/// A JSON value, as much of it as `cargo metadata` needs.
#[derive(Debug)]
enum Json {
    Null,
    Bool(bool),
    Number,
    Text(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn text(&self) -> Option<&str> {
        match self {
            Json::Text(text) => Some(text),
            _ => None,
        }
    }

    fn items(&self) -> &[Json] {
        match self {
            Json::Array(items) => items,
            _ => &[],
        }
    }

    fn truth(&self) -> bool {
        matches!(self, Json::Bool(true))
    }
}

struct JsonReader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl JsonReader<'_> {
    fn space(&mut self) {
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        self.space();
        if self.bytes.get(self.at) == Some(&byte) {
            self.at += 1;
            Ok(())
        } else {
            Err(format!("expected '{}' at byte {}", byte as char, self.at))
        }
    }

    fn word(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(format!("unexpected input at byte {}", self.at))
        }
    }

    fn hex(&mut self) -> Result<u32, String> {
        let digits = self
            .bytes
            .get(self.at..self.at + 4)
            .and_then(|d| std::str::from_utf8(d).ok())
            .ok_or("a truncated \\u escape")?;
        self.at += 4;
        u32::from_str_radix(digits, 16).map_err(|e| e.to_string())
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let byte = *self.bytes.get(self.at).ok_or("an unterminated string")?;
            self.at += 1;
            match byte {
                b'"' => break,
                b'\\' => {
                    let escape = *self.bytes.get(self.at).ok_or("a truncated escape")?;
                    self.at += 1;
                    let decoded = match escape {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let mut code = self.hex()?;
                            if (0xD800..0xDC00).contains(&code)
                                && self.bytes[self.at..].starts_with(b"\\u")
                            {
                                self.at += 2;
                                let low = self.hex()?;
                                code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                            }
                            char::from_u32(code).unwrap_or('\u{fffd}')
                        }
                        other => return Err(format!("an unknown escape '{}'", other as char)),
                    };
                    let mut buffer = [0; 4];
                    out.extend_from_slice(decoded.encode_utf8(&mut buffer).as_bytes());
                }
                other => out.push(other),
            }
        }
        String::from_utf8(out).map_err(|e| e.to_string())
    }

    fn value(&mut self) -> Result<Json, String> {
        self.space();
        match self.bytes.get(self.at) {
            Some(b'{') => {
                self.at += 1;
                let mut fields = Vec::new();
                self.space();
                if self.bytes.get(self.at) == Some(&b'}') {
                    self.at += 1;
                    return Ok(Json::Object(fields));
                }
                loop {
                    let key = self.string()?;
                    self.expect(b':')?;
                    fields.push((key, self.value()?));
                    self.space();
                    match self.bytes.get(self.at) {
                        Some(b',') => self.at += 1,
                        Some(b'}') => {
                            self.at += 1;
                            return Ok(Json::Object(fields));
                        }
                        _ => return Err(format!("expected ',' or '}}' at byte {}", self.at)),
                    }
                }
            }
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                self.space();
                if self.bytes.get(self.at) == Some(&b']') {
                    self.at += 1;
                    return Ok(Json::Array(items));
                }
                loop {
                    items.push(self.value()?);
                    self.space();
                    match self.bytes.get(self.at) {
                        Some(b',') => self.at += 1,
                        Some(b']') => {
                            self.at += 1;
                            return Ok(Json::Array(items));
                        }
                        _ => return Err(format!("expected ',' or ']' at byte {}", self.at)),
                    }
                }
            }
            Some(b'"') => self.string().map(Json::Text),
            Some(b't') => self.word("true", Json::Bool(true)),
            Some(b'f') => self.word("false", Json::Bool(false)),
            Some(b'n') => self.word("null", Json::Null),
            Some(_) => {
                let start = self.at;
                while self
                    .bytes
                    .get(self.at)
                    .is_some_and(|b| b"+-.0123456789eE".contains(b))
                {
                    self.at += 1;
                }
                if self.at == start {
                    Err(format!("unexpected input at byte {start}"))
                } else {
                    Ok(Json::Number)
                }
            }
            None => Err("unexpected end of input".to_owned()),
        }
    }
}

fn parse_json(text: &str) -> Result<Json, String> {
    let mut reader = JsonReader {
        bytes: text.as_bytes(),
        at: 0,
    };
    let value = reader.value()?;
    reader.space();
    if reader.at == text.len() {
        Ok(value)
    } else {
        Err(format!("trailing input at byte {}", reader.at))
    }
}

/// The members of a `cargo metadata --no-deps --format-version 1` document.
fn manifests_from_metadata(metadata: &str) -> Manifests {
    let document = parse_json(metadata).expect("cargo metadata is JSON");
    let mut manifests = Manifests::new();
    let packages = document.get("packages").expect("a packages list").items();
    for package in packages {
        let name = package.get("name").and_then(Json::text).expect("a name");
        let mut member = Member::default();
        for dependency in package.get("dependencies").map_or(&[][..], Json::items) {
            let field = |key: &str| dependency.get(key).and_then(Json::text);
            let package_name = field("name").expect("a dependency name").to_owned();
            member.declarations.push(Declaration {
                local: field("rename").unwrap_or(&package_name).to_owned(),
                kind: field("kind").unwrap_or("normal").to_owned(),
                optional: dependency.get("optional").is_some_and(Json::truth),
                default_features: dependency
                    .get("uses_default_features")
                    .is_none_or(Json::truth),
                features: dependency
                    .get("features")
                    .map_or(&[][..], Json::items)
                    .iter()
                    .filter_map(Json::text)
                    .map(str::to_owned)
                    .collect(),
                package: package_name,
            });
        }
        if let Some(Json::Object(features)) = package.get("features") {
            for (feature, enables) in features {
                member.features.insert(
                    feature.clone(),
                    enables
                        .items()
                        .iter()
                        .filter_map(Json::text)
                        .map(str::to_owned)
                        .collect(),
                );
            }
        }
        manifests.insert(name.to_owned(), member);
    }
    manifests
}

/// `cargo metadata --no-deps` of the workspace at `dir`.
fn metadata(dir: &Path) -> Result<String, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let out = Command::new(cargo)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
        ])
        .current_dir(dir)
        .env_remove("CARGO_TARGET_DIR")
        .output()
        .expect("cargo runs");
    if out.status.success() {
        String::from_utf8(out.stdout).map_err(|e| e.to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

fn repository_manifests() -> Manifests {
    manifests_from_metadata(&metadata(&repository_root()).expect("cargo metadata of the workspace"))
}

/// Why the approval of each backend in `backends` does not hold in the store at `root`: its id
/// is not `approval-record:<slug>`, the record does not exist, or it does not name the crate.
fn approval_problems(root: &Path, backends: &[AudioBackend]) -> Vec<String> {
    let mut found = Vec::new();
    for backend in backends {
        let label = format!("{} {}", backend.name, backend.version);
        let Some(slug) = backend
            .approval
            .strip_prefix("approval-record:")
            .filter(|s| !s.is_empty() && !s.contains(['/', '.']))
        else {
            found.push(format!(
                "{label}: approval '{}' is not approval-record:<slug>",
                backend.approval
            ));
            continue;
        };
        let path = root.join(format!(".engineering/planning/approval-record/{slug}.md"));
        match fs::read_to_string(&path) {
            Ok(text) if text.contains(&format!("`{}`", backend.name)) => {}
            Ok(_) => found.push(format!(
                "{label}: {} does not name `{}`",
                backend.approval, backend.name
            )),
            Err(e) => found.push(format!("{label}: {}: {e}", backend.approval)),
        }
    }
    found
}

/// How the resolved closure of `backend` in `lock` differs from its reviewed one, if it does.
fn closure_drift(lock: &str, backend: &AudioBackend) -> Option<String> {
    let resolved = registry_closure(lock, backend.name);
    let reviewed: BTreeSet<(String, String)> = backend
        .closure
        .iter()
        .chain(&[(backend.name, backend.version)])
        .map(|&(name, version)| (name.to_owned(), version.to_owned()))
        .collect();
    (resolved != reviewed).then(|| {
        format!(
            "{}: resolved {:?}, reviewed {:?}",
            backend.name,
            resolved.difference(&reviewed).collect::<Vec<_>>(),
            reviewed.difference(&resolved).collect::<Vec<_>>()
        )
    })
}

// The audio I/O backend rule (DESIGN.md § 3, § 12.2, ADR-007) and ModelSpec (§ 8.2), wave a2,
// story:dependency-gate.

/// Approved backends the fixture cases judge against; not this repository's list.
const CPAL: AudioBackend = AudioBackend {
    name: "cpal",
    version: "0.15.0",
    approval: "approval-record:fixture-cpal",
    closure: &[("alsa", "0.9.0"), ("libc", "0.2.0")],
};
const PORTAUDIO: AudioBackend = AudioBackend {
    name: "portaudio",
    version: "0.7.0",
    approval: "approval-record:fixture-portaudio",
    closure: &[],
};
/// `cpal` reviewed without `libc`, which its resolved closure holds.
const CPAL_NARROW: AudioBackend = AudioBackend {
    name: "cpal",
    version: "0.15.0",
    approval: "approval-record:fixture-cpal",
    closure: &[("alsa", "0.9.0")],
};

/// The registry the audio cases resolve against, as `(name, version, [dependencies])`.
const AUDIO_REGISTRY: &[(&str, &str, &str)] = &[
    ("cpal", "0.15.0", "alsa = \"0.9\"\n\n[features]\nx = []\n"),
    ("alsa", "0.9.0", "libc = \"0.2\"\n"),
    ("libc", "0.2.0", ""),
    ("portaudio", "0.7.0", ""),
    ("rodio", "0.17.0", "cpal = \"0.15\"\n"),
    ("modelspec", "0.1.0", ""),
    ("clap", "4.6.7", ""),
];

/// Resolves a scratch workspace whose members are `members` (`crates/<name>`, each a package of
/// that name with `body` after its `[package]` table) against `AUDIO_REGISTRY`, offline, and
/// returns its `Cargo.lock` and manifest facts, or cargo's error.
fn resolve_members(members: &[(&str, &str)]) -> Result<(String, Manifests), String> {
    static CASE: AtomicUsize = AtomicUsize::new(0);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "audio-case-{}-{}",
        std::process::id(),
        CASE.fetch_add(1, Ordering::Relaxed)
    ));
    let list: Vec<String> = members
        .iter()
        .map(|(name, _)| format!("\"crates/{name}\""))
        .collect();
    write(
        &dir.join("Cargo.toml"),
        &format!(
            "[workspace]\nmembers = [{}]\nresolver = \"3\"\n",
            list.join(", ")
        ),
    );
    for (name, body) in members {
        let crate_dir = dir.join("crates").join(name);
        write(
            &crate_dir.join("Cargo.toml"),
            &format!(
                "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n{body}"
            ),
        );
        write(&crate_dir.join("src/lib.rs"), "");
    }
    for (name, version, dependencies) in AUDIO_REGISTRY {
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
        metadata(&dir).map(|metadata| {
            (
                fs::read_to_string(dir.join("Cargo.lock")).expect("cargo wrote Cargo.lock"),
                manifests_from_metadata(&metadata),
            )
        })
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    };
    fs::remove_dir_all(&dir).expect("remove the scratch workspace");
    result
}

/// What the gate refuses in the workspace of `members`, from every member, with `backends`
/// approved. Panics when cargo cannot resolve it, so an unresolvable case never reads as refused.
fn member_violations(members: &[(&str, &str)], backends: &[AudioBackend]) -> Vec<String> {
    match resolve_members(members) {
        Ok((lock, manifests)) => {
            let roots: Vec<&str> = members.iter().map(|(name, _)| *name).collect();
            audit(&lock, &manifests, &roots, backends)
        }
        Err(e) => panic!("cargo could not resolve {members:?}\n{e}"),
    }
}

const AUDIO_PATH: &str = "patchback-audio = { path = \"../patchback-audio\"";

#[test]
fn a_workspace_with_no_audio_backend_passes() {
    for backends in [&[][..], &[CPAL][..], &[CPAL, PORTAUDIO][..]] {
        let found = member_violations(
            &[
                (
                    "patchback-cli",
                    &format!("[dependencies]\nclap = \"4\"\n{AUDIO_PATH} }}\n"),
                ),
                ("patchback-audio", ""),
            ],
            backends,
        );
        assert_eq!(found, Vec::<String>::new());
    }
}

#[test]
fn one_optional_approved_backend_with_its_reviewed_closure_passes() {
    let audio_forms = [
        "[dependencies]\ncpal = { version = \"0.15\", optional = true }\n\n\
         [features]\nlive = [\"dep:cpal\"]\n",
        "[dependencies]\ncpal = { version = \"0.15\", optional = true }\n",
        "[dependencies.cpal]\nversion = \"0.15\"\noptional = true\n",
        "[target.'cfg(unix)'.dependencies]\ncpal = { version = \"0.15\", optional = true }\n",
        "[dependencies]\nsound = { package = \"cpal\", version = \"0.15\", optional = true }\n\n\
         [features]\nlive = [\"dep:sound\"]\n",
    ];
    for audio in audio_forms {
        let found = member_violations(
            &[
                (
                    "patchback-cli",
                    &format!("[dependencies]\n{AUDIO_PATH} }}\n"),
                ),
                ("patchback-audio", audio),
            ],
            &[CPAL],
        );
        assert_eq!(found, Vec::<String>::new(), "{audio}");
    }
    // The CLI reaches the backend only through features of its own.
    let audio = "[dependencies]\ncpal = { version = \"0.15\", optional = true }\n\n\
                 [features]\nlive = [\"dep:cpal\"]\n";
    let cli_forms = [
        format!(
            "[dependencies]\n{AUDIO_PATH} }}\n\n[features]\nlive = [\"patchback-audio/live\"]\n"
        ),
        format!(
            "[dependencies]\n{AUDIO_PATH}, optional = true, features = [\"live\"] }}\n\n\
             [features]\nlive = [\"dep:patchback-audio\"]\n"
        ),
    ];
    for cli in &cli_forms {
        let found = member_violations(
            &[("patchback-cli", cli), ("patchback-audio", audio)],
            &[CPAL],
        );
        assert_eq!(found, Vec::<String>::new(), "{cli}");
    }
}

#[test]
fn an_audio_backend_that_is_not_optional_is_refused() {
    let optional = "[dependencies]\ncpal = { version = \"0.15\", optional = true }\n";
    let cases: Vec<(String, String)> = vec![
        (
            String::new(),
            "[dependencies]\ncpal = \"0.15\"\n".to_owned(),
        ),
        (
            String::new(),
            "[target.'cfg(unix)'.dependencies]\ncpal = \"0.15\"\n".to_owned(),
        ),
        (
            String::new(),
            format!("{optional}\n[dev-dependencies]\ncpal = \"0.15\"\n"),
        ),
        (
            String::new(),
            "[build-dependencies]\ncpal = \"0.15\"\n".to_owned(),
        ),
        (
            String::new(),
            format!("{optional}\n[features]\ndefault = [\"live\"]\nlive = [\"dep:cpal\"]\n"),
        ),
        (
            String::new(),
            format!("{optional}\n[features]\ndefault = [\"cpal\"]\n"),
        ),
        (
            String::new(),
            format!("{optional}\n[features]\ndefault = [\"cpal/x\"]\n"),
        ),
        // turned on by a dependent that always asks for the feature
        (
            format!("[dependencies]\n{AUDIO_PATH}, features = [\"live\"] }}\n"),
            format!("{optional}\n[features]\nlive = [\"dep:cpal\"]\n"),
        ),
        // turned on by a dependent's default features
        (
            format!(
                "[dependencies]\n{AUDIO_PATH} }}\n\n[features]\ndefault = [\"patchback-audio/live\"]\n"
            ),
            format!("{optional}\n[features]\nlive = [\"dep:cpal\"]\n"),
        ),
    ];
    for (cli, audio) in &cases {
        let cli = if cli.is_empty() {
            format!("[dependencies]\n{AUDIO_PATH} }}\n")
        } else {
            cli.clone()
        };
        let found = member_violations(
            &[("patchback-cli", &cli), ("patchback-audio", audio)],
            &[CPAL],
        );
        assert!(
            found.iter().any(|f| f.starts_with("patchback-")
                && f.contains("audio backend cpal")
                && f.contains("optional")),
            "{cli}\n{audio}\n{found:#?}"
        );
    }
}

#[test]
fn at_most_one_audio_backend_is_admitted() {
    // two backends in one crate, and one in each of two crates
    let cases: [&[(&str, &str)]; 2] = [
        &[(
            "patchback-audio",
            "[dependencies]\ncpal = { version = \"0.15\", optional = true }\n\
             portaudio = { version = \"0.7\", optional = true }\n",
        )],
        &[
            (
                "patchback-audio",
                "[dependencies]\ncpal = { version = \"0.15\", optional = true }\n",
            ),
            (
                "patchback-dsp",
                "[dependencies]\nportaudio = { version = \"0.7\", optional = true }\n",
            ),
        ],
    ];
    for members in cases {
        let found = member_violations(members, &[CPAL, PORTAUDIO]);
        assert_eq!(found.len(), 1, "{found:#?}");
        assert!(found[0].contains("second audio I/O backend"), "{found:#?}");
    }
}

#[test]
fn an_unapproved_audio_crate_is_refused() {
    // rodio, which no approval lists, beside an approved cpal
    let found = member_violations(
        &[(
            "patchback-audio",
            "[dependencies]\nrodio = { version = \"0.17\", optional = true }\n",
        )],
        &[CPAL],
    );
    assert!(
        found
            .iter()
            .any(|f| f.starts_with("rodio 0.17.0: an audio I/O crate that is not an approved")),
        "{found:#?}"
    );
    // cpal itself, against this repository's list, which approves none
    let found = member_violations(
        &[(
            "patchback-audio",
            "[dependencies]\ncpal = { version = \"0.15\", optional = true }\n",
        )],
        AUDIO_BACKENDS,
    );
    assert!(
        found
            .iter()
            .any(|f| f.starts_with("cpal 0.15.0: an audio I/O crate that is not an approved")),
        "{found:#?}"
    );
    // an approved name at a version the review did not see
    let lock = lock_of(&[
        ("patchback-audio", "0.0.0", None, &["cpal"]),
        ("cpal", "0.16.0", Some(REGISTRY), &[]),
    ]);
    let found = audit(&lock, &Manifests::new(), &["patchback-audio"], &[CPAL]);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].starts_with("cpal 0.16.0: version not the approved audio I/O backend"),
        "{found:#?}"
    );
}

#[test]
fn an_approved_backend_with_an_unreviewed_closure_package_is_refused() {
    let found = member_violations(
        &[(
            "patchback-audio",
            "[dependencies]\ncpal = { version = \"0.15\", optional = true }\n",
        )],
        &[CPAL_NARROW],
    );
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].starts_with("libc 0.2.0: entered cpal's closure unreviewed"),
        "{found:#?}"
    );
    // a reviewed closure package at another version
    let lock = lock_of(&[
        ("patchback-audio", "0.0.0", None, &["cpal"]),
        ("cpal", "0.15.0", Some(REGISTRY), &["alsa"]),
        ("alsa", "0.9.1", Some(REGISTRY), &[]),
    ]);
    let found = audit(
        &lock,
        &Manifests::new(),
        &["patchback-audio"],
        &[CPAL_NARROW],
    );
    assert!(
        found
            .iter()
            .any(|f| f.starts_with("alsa 0.9.1: version not in cpal's reviewed closure")),
        "{found:#?}"
    );
    // a backend closure package a workspace crate takes directly
    let lock = lock_of(&[
        ("patchback-audio", "0.0.0", None, &["alsa"]),
        ("alsa", "0.9.0", Some(REGISTRY), &[]),
    ]);
    let found = audit(
        &lock,
        &Manifests::new(),
        &["patchback-audio"],
        &[CPAL_NARROW],
    );
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].contains("takes alsa 0.9.0 directly"), "{found:#?}");
}

#[test]
fn a_backend_whose_manifest_was_not_read_is_refused() {
    let lock = lock_of(&[
        ("patchback-audio", "0.0.0", None, &["cpal"]),
        ("cpal", "0.15.0", Some(REGISTRY), &["alsa"]),
        ("alsa", "0.9.0", Some(REGISTRY), &["libc"]),
        ("libc", "0.2.0", Some(REGISTRY), &[]),
    ]);
    let found = audit(&lock, &Manifests::new(), &["patchback-audio"], &[CPAL]);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].contains("cannot be shown optional"), "{found:#?}");
}

#[test]
fn no_modelspec_crate_is_admitted() {
    let reason = "ModelSpec runs as a subprocess";
    let locks = [
        // from crates.io
        lock_of(&[
            ("patchback-model", "0.0.0", None, &["modelspec"]),
            ("modelspec", "0.1.0", Some(REGISTRY), &[]),
        ]),
        // a path crate
        lock_of(&[
            ("patchback-model", "0.0.0", None, &["modelspec-bridge"]),
            ("modelspec-bridge", "0.0.0", None, &[]),
        ]),
        // under a patchback name is still a path crate; a git ModelSpec
        lock_of(&[
            ("patchback-model", "0.0.0", None, &["modelspec-core"]),
            (
                "modelspec-core",
                "0.2.0",
                Some("git+https://example.invalid/modelspec#0"),
                &[],
            ),
        ]),
        // inside clap's closure
        lock_of(&[
            ("patchback-model", "0.0.0", None, &["clap"]),
            ("clap", "4.6.7", Some(REGISTRY), &["modelspec-types"]),
            ("modelspec-types", "0.1.0", Some(REGISTRY), &[]),
        ]),
        // inside an approved backend's closure
        lock_of(&[
            ("patchback-model", "0.0.0", None, &["cpal"]),
            ("cpal", "0.15.0", Some(REGISTRY), &["modelspec"]),
            ("modelspec", "0.1.0", Some(REGISTRY), &[]),
        ]),
    ];
    for lock in &locks {
        let found = audit(lock, &Manifests::new(), &["patchback-model"], &[CPAL]);
        assert!(
            found
                .iter()
                .any(|f| f.starts_with("modelspec") && f.contains(reason)),
            "{lock}\n{found:#?}"
        );
    }
    let found = member_violations(
        &[(
            "patchback-model",
            "[dependencies]\nmodelspec = { version = \"0.1\", optional = true }\n",
        )],
        &[],
    );
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].starts_with("modelspec 0.1.0:") && found[0].contains(reason),
        "{found:#?}"
    );
}

#[test]
fn the_audio_backend_allow_list_holds_at_most_one_backend() {
    assert!(AUDIO_BACKENDS.len() <= 1, "DESIGN.md § 3: at most one");
}

#[test]
fn every_approved_audio_backend_names_its_approval_record() {
    let root = repository_root();
    assert_eq!(
        approval_problems(&root, AUDIO_BACKENDS),
        Vec::<String>::new()
    );
    // the check itself: a record that names the crate, one that does not, one that is missing,
    // and an id that is not an approval record
    let named = AudioBackend {
        name: "clap",
        version: "4.6.7",
        approval: "approval-record:cli-clap",
        closure: &[],
    };
    assert_eq!(approval_problems(&root, &[named]), Vec::<String>::new());
    for approval in [
        "approval-record:cli-clap",
        "approval-record:no-such-record",
        "approval-record:../story/dependency-gate",
        "story:dependency-gate",
        "",
    ] {
        let backend = AudioBackend {
            name: "portaudio-unnamed",
            version: "0.0.0",
            approval,
            closure: &[],
        };
        assert_eq!(approval_problems(&root, &[backend]).len(), 1, "{approval}");
    }
}

#[test]
fn every_approved_audio_backend_closure_is_exactly_its_resolved_closure() {
    let lock = repository_lock();
    for backend in AUDIO_BACKENDS {
        assert_eq!(closure_drift(&lock, backend), None);
    }
    let (lock, _) = resolve_members(&[(
        "patchback-audio",
        "[dependencies]\ncpal = { version = \"0.15\", optional = true }\n",
    )])
    .expect("the fixture resolves");
    assert_eq!(closure_drift(&lock, &CPAL), None);
    assert!(closure_drift(&lock, &CPAL_NARROW).is_some());
}

#[test]
fn task_dependency_audit_runs_only_the_gate_and_check_calls_it() {
    let taskfile =
        fs::read_to_string(repository_root().join("Taskfile.yml")).expect("Taskfile.yml");
    let block = |name: &str| -> Vec<String> {
        taskfile
            .lines()
            .skip_while(|line| line.trim_end() != format!("  {name}:"))
            .skip(1)
            .take_while(|line| line.is_empty() || line.starts_with("    "))
            .map(|line| line.trim().to_owned())
            .filter(|line| !line.is_empty())
            .collect()
    };
    assert_eq!(
        block("dependency-audit"),
        [
            "cmds:",
            "- cargo test --locked -p patchback-cli --test dependencies"
        ]
    );
    assert!(
        block("check").contains(&"- task: dependency-audit".to_owned()),
        "{:?}",
        block("check")
    );
}

#[test]
fn each_refusal_is_one_line_naming_its_package() {
    let lock = lock_of(&[
        (
            "patchback-cli",
            "0.0.0",
            None,
            &["rand", "syn", "modelspec", "cpal"],
        ),
        ("rand", "0.8.5", Some(REGISTRY), &[]),
        ("syn", "3.0.9", Some(REGISTRY), &[]),
        ("modelspec", "0.1.0", Some(REGISTRY), &[]),
        ("cpal", "0.15.0", Some(REGISTRY), &[]),
    ]);
    let found = audit(
        &lock,
        &Manifests::new(),
        &["patchback-cli", "patchback-fit"],
        &[],
    );
    let text = report(&found);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 5, "{text}");
    for package in [
        "patchback-fit: ",
        "rand 0.8.5: ",
        "modelspec 0.1.0: ",
        "cpal 0.15.0: ",
    ] {
        assert!(
            lines.iter().any(|line| line.starts_with(package)),
            "{package} in\n{text}"
        );
    }
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("patchback-cli 0.0.0: takes syn 3.0.9 directly")),
        "{text}"
    );
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
