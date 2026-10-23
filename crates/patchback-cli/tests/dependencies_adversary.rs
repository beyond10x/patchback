//! Adversary pass (wave a2, story:dependency-gate) on the dependency gate in
//! `tests/dependencies.rs`.
//!
//! The gate's functions are private to that test file, so each case below is compiled together
//! with it, unchanged but for its leading `//!` lines, in a scratch package under
//! `CARGO_TARGET_TMPDIR` and run there once. Every outer test fails exactly when its inner case
//! does, and quotes the inner failure. Read with `std` only.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

/// The inner cases, appended to the gate's own source. Names start with `xadv_` so the inner run
/// selects them and nothing else.
const CASES: &str = r####"

// ---- adversary cases, story:dependency-gate ----

fn xadv_store(name: &str, record: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("xadv-{name}-{}", std::process::id()));
    write(&root.join(".engineering/planning/approval-record/fixture-cpal.md"), record);
    root
}

#[test]
fn xadv_a_record_that_names_the_crate_only_to_reject_it_is_no_approval() {
    let root = xadv_store(
        "prose",
        "---\nformat: aep.planning-md/3\nid: approval-record:fixture-cpal\nkind: approval-record\n\
         status: draft\ntitle: No live audio backend yet\nrevision: 1\n---\n\
         # No live audio backend yet\n\n`cpal` was proposed as the live audio backend and is not \
         approved; no backend is admitted.\n",
    );
    let problems = approval_problems(&root, &[CPAL]);
    fs::remove_dir_all(&root).ok();
    assert!(
        !problems.is_empty(),
        "a record whose only mention of `cpal` declines it was accepted as cpal's approval"
    );
}

#[test]
fn xadv_an_archived_or_rejected_approval_record_is_no_approval() {
    for status in ["archived", "rejected", "superseded"] {
        let root = xadv_store(
            status,
            &format!(
                "---\nformat: aep.planning-md/3\nid: approval-record:fixture-cpal\n\
                 kind: approval-record\nstatus: {status}\ntitle: Live audio backend\nrevision: 2\n\
                 ---\n# Live audio backend\n\nApprove `cpal` 0.15.0 as the live audio backend.\n"
            ),
        );
        let problems = approval_problems(&root, &[CPAL]);
        fs::remove_dir_all(&root).ok();
        assert!(
            !problems.is_empty(),
            "an approval record with status {status} was accepted as cpal's approval"
        );
    }
}

#[test]
fn xadv_a_modelspec_crate_in_any_letter_case_is_refused_as_modelspec() {
    // crates.io keeps a crate's own letter case, and Cargo.lock records it.
    let lock = lock_of(&[
        ("patchback-model", "0.0.0", None, &["ModelSpec"]),
        ("ModelSpec", "0.1.0", Some(REGISTRY), &[]),
    ]);
    let found = audit(&lock, &Manifests::new(), &["patchback-model"], &[]);
    assert!(
        found.iter().any(|f| f.starts_with("ModelSpec 0.1.0:") && f.contains("ModelSpec runs as a subprocess")),
        "ModelSpec was refused, but not as a ModelSpec crate: {found:#?}"
    );
}

#[test]
fn xadv_a_dev_dependency_that_asks_for_the_backend_feature_is_refused() {
    // `cargo test --workspace` (task rust-test) unifies dev-dependency features, so cpal is
    // built and linked in every test run although its declaration says optional.
    let found = member_violations(
        &[
            (
                "patchback-cli",
                &format!(
                    "[dependencies]\n{AUDIO_PATH} }}\n\n[dev-dependencies]\n{AUDIO_PATH}, features = [\"live\"] }}\n"
                ),
            ),
            (
                "patchback-audio",
                "[dependencies]\ncpal = { version = \"0.15\", optional = true }\n\n[features]\nlive = [\"dep:cpal\"]\n",
            ),
        ],
        &[CPAL],
    );
    assert!(
        found.iter().any(|f| f.contains("audio backend cpal") && f.contains("optional")),
        "a dev-dependency turns cpal on in every test build and the gate admitted it: {found:#?}"
    );
}

#[test]
fn xadv_a_renamed_backend_turned_on_by_default_is_refused() {
    let found = member_violations(
        &[(
            "patchback-audio",
            "[dependencies]\nsound = { package = \"cpal\", version = \"0.15\", optional = true }\n\n\
             [features]\ndefault = [\"sound\"]\n",
        )],
        &[CPAL],
    );
    assert!(
        found.iter().any(|f| f.contains("audio backend cpal") && f.contains("not optional")),
        "{found:#?}"
    );
}

#[test]
fn xadv_a_package_reached_through_the_backend_is_judged_against_the_backend_closure() {
    // windows-sys 0.61.2 is in clap's reviewed closure, not in cpal's.
    let lock = lock_of(&[
        ("patchback-cli", "0.0.0", None, &["clap", "cpal"]),
        ("clap", "4.6.7", Some(REGISTRY), &["windows-sys"]),
        ("cpal", "0.15.0", Some(REGISTRY), &["windows-sys"]),
        ("windows-sys", "0.61.2", Some(REGISTRY), &[]),
    ]);
    let found = audit(&lock, &Manifests::new(), &["patchback-cli"], &[CPAL]);
    assert!(
        found.iter().any(|f| f.starts_with("windows-sys 0.61.2: entered cpal's closure unreviewed")),
        "{found:#?}"
    );
}

#[test]
fn xadv_metadata_escapes_and_renames_are_read_as_cargo_wrote_them() {
    // `%` stands for a backslash: JSON \u escapes, written so no editor decodes them.
    let metadata = r#"{"packages":[{"name":"p%u0061tchback-audio","version":"0.0.0","metadata":null,
        "dependencies":[{"name":"cpal","source":"registry+https://github.com/rust-lang/crates.io-index",
        "req":"^0.15","kind":null,"rename":"s%u00f6und","optional":true,"uses_default_features":false,
        "features":["a%"b","%ud83c%udfb5","c%%d"],"target":"cfg(unix)","registry":null,"path":null},
        {"name":"cc","kind":"build","rename":null,"optional":false,"uses_default_features":true,"features":[]}],
        "features":{"live":["dep:s%u00F6und","s%u00f6und?/x"],"default":[]},"rust_version":null,
        "edition":"2024","nested":{"a":[1,-2.5e+3,{"b":[[],{}]}]}}],"workspace_members":[],
        "resolve":null,"version":1}"#
        .replace('%', &char::from(92).to_string());
    assert!(metadata.contains(&format!("{}u0061", char::from(92))), "{metadata}");
    let manifests = manifests_from_metadata(&metadata);
    let member = &manifests["patchback-audio"];
    let cpal = &member.declarations[0];
    assert_eq!(cpal.package, "cpal");
    assert_eq!(cpal.local, "s\u{f6}und");
    assert_eq!(cpal.kind, "normal");
    assert!(cpal.optional);
    assert!(!cpal.default_features);
    assert_eq!(cpal.features, ["a\"b", "\u{1f3b5}", "c\\d"]);
    assert_eq!(member.declarations[1].kind, "build");
    assert_eq!(member.features["live"], ["dep:s\u{f6}und", "s\u{f6}und?/x"]);
}

#[test]
fn xadv_a_malformed_surrogate_escape_is_an_error_not_a_panic() {
    let text = r#"{"name":"%ud800%u0041"}"#.replace('%', &char::from(92).to_string());
    let outcome = std::panic::catch_unwind(|| parse_json(&text).is_ok());
    assert!(outcome.is_ok(), "parse_json panicked on a high surrogate followed by \\u0041");
}
"####;

/// The inner run's output, compiled and run once for every outer case.
fn inner() -> &'static str {
    static OUTPUT: OnceLock<String> = OnceLock::new();
    OUTPUT.get_or_init(|| {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let gate = fs::read_to_string(manifest_dir.join("tests/dependencies.rs"))
            .expect("tests/dependencies.rs");
        let body: String = gate
            .lines()
            .skip_while(|line| line.starts_with("//!"))
            .map(|line| format!("{line}\n"))
            .collect();
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("dependency-gate-adversary-{}", std::process::id()));
        fs::create_dir_all(dir.join("tests")).expect("scratch package");
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"gate-adversary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
             publish = false\n\n[workspace]\n",
        )
        .expect("scratch manifest");
        fs::create_dir_all(dir.join("src")).expect("scratch src");
        fs::write(dir.join("src/lib.rs"), "").expect("scratch lib");
        fs::write(dir.join("tests/gate.rs"), format!("{body}{CASES}")).expect("scratch gate");
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
        let out = Command::new(cargo)
            .args([
                "test",
                "--offline",
                "--test",
                "gate",
                "--",
                "xadv_",
                "--test-threads=1",
            ])
            .current_dir(&dir)
            .env_remove("CARGO_TARGET_DIR")
            .output()
            .expect("cargo runs");
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        fs::remove_dir_all(&dir).ok();
        text
    })
}

/// Fails, quoting the inner failure, when the inner case `name` did not pass.
fn case(name: &str) {
    let output = inner();
    let passed = format!("test {name} ... ok");
    if output.lines().any(|line| line.trim() == passed) {
        return;
    }
    let failure: String = output
        .split(&format!("---- {name} stdout ----"))
        .nth(1)
        .map(|rest| {
            let rest = rest.trim_start();
            rest.split("\n\n").next().unwrap_or(rest).to_owned()
        })
        .unwrap_or_else(|| output.to_owned());
    panic!("inner case {name} did not pass:\n{failure}");
}

#[test]
fn adversary_gate_rejected_prose_record_is_no_approval() {
    case("xadv_a_record_that_names_the_crate_only_to_reject_it_is_no_approval");
}

#[test]
fn adversary_gate_archived_record_is_no_approval() {
    case("xadv_an_archived_or_rejected_approval_record_is_no_approval");
}

#[test]
fn adversary_gate_modelspec_any_case() {
    case("xadv_a_modelspec_crate_in_any_letter_case_is_refused_as_modelspec");
}

#[test]
fn adversary_gate_dev_dependency_feature_turns_backend_on() {
    case("xadv_a_dev_dependency_that_asks_for_the_backend_feature_is_refused");
}

#[test]
fn adversary_gate_renamed_backend_default() {
    case("xadv_a_renamed_backend_turned_on_by_default_is_refused");
}

#[test]
fn adversary_gate_closure_attribution_through_backend() {
    case("xadv_a_package_reached_through_the_backend_is_judged_against_the_backend_closure");
}

#[test]
fn adversary_gate_metadata_escapes() {
    case("xadv_metadata_escapes_and_renames_are_read_as_cargo_wrote_them");
}

#[test]
fn adversary_gate_malformed_surrogate() {
    case("xadv_a_malformed_surrogate_escape_is_an_error_not_a_panic");
}
