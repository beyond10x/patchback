//! The ModelSpec adapter against stand-in binaries: shell scripts this test writes under the cargo
//! target, so every verdict of `modelspec --version` and `modelspec check` is chosen here.
//! `pinned_binary.rs` runs the real pinned binary.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use patchback_model::{
    CheckError, CheckOutcome, ModelNotChecked, ModelSource, ModelSourceState, ModelSpec,
    ModelSpecError, ModelSpecPin, PinError, locate_with,
};

const SOURCE: &str = "models/patch_regressor.mspec";

fn pin() -> ModelSpecPin {
    ModelSpecPin::repository().expect("models/modelspec.pin parses")
}

/// The stand-in directory. Every script is written before the first test spawns anything, so no
/// spawn can inherit a script's open write descriptor (ETXTBSY).
fn standins() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-standins");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("bin")).unwrap();
        let version = pin().version().to_owned();
        let marker = dir.join("old-was-asked-to-check");
        let scripts = [
            (
                "checks",
                format!(
                    "case \"$1\" in --version) echo \"modelspec {version}\";; check) exit 0;; *) exit 64;; esac"
                ),
            ),
            (
                "rejects",
                format!(
                    "case \"$1\" in --version) echo \"modelspec {version}\";; check) echo \"E-TYPE-001: matmul shapes [16, 32] and [64, 8] do not agree\" >&2; exit 1;; *) exit 64;; esac"
                ),
            ),
            (
                "old",
                format!(
                    "case \"$1\" in --version) echo \"modelspec 0.4.0\";; check) touch {};; *) exit 64;; esac",
                    marker.display()
                ),
            ),
            (
                "no-version",
                "case \"$1\" in --version) echo \"unrecognized option\" >&2; exit 2;; *) exit 0;; esac"
                    .to_owned(),
            ),
            (
                "vanishes",
                format!("case \"$1\" in --version) echo \"modelspec {version}\";; *) exit 0;; esac"),
            ),
            (
                "bin/modelspec",
                format!("case \"$1\" in --version) echo \"modelspec {version}\";; *) exit 0;; esac"),
            ),
        ];
        for (name, body) in scripts {
            let path = dir.join(name);
            fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let plain = dir.join("not-executable");
        fs::write(&plain, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&plain, fs::Permissions::from_mode(0o644)).unwrap();
        dir
    })
}

fn standin(name: &str) -> PathBuf {
    standins().join(name)
}

fn open(name: &str) -> ModelSpec {
    ModelSpec::open(&standin(name), &pin()).expect("the stand-in reports the pinned version")
}

fn source() -> ModelSource {
    ModelSource::register(SOURCE, "PatchRegressor", 16)
}

#[test]
fn the_repository_pin_names_version_commit_and_checksum() {
    let pin = pin();
    assert_eq!(pin.version(), "0.5.0");
    assert_eq!(pin.commit(), "7c45c1adaf6e0cc0ba05f18878b31bfdc57cc529");
    assert_eq!(
        pin.sha256(),
        "171d2880ddd197ef91db8f36da2e6268676aa5aa5adc28d28ea8bc39cae24229"
    );
}

#[test]
fn a_malformed_pin_is_refused() {
    let commit = "commit 7c45c1adaf6e0cc0ba05f18878b31bfdc57cc529";
    let sha = "sha256-x86_64-unknown-linux-gnu 171d2880ddd197ef91db8f36da2e6268676aa5aa5adc28d28ea8bc39cae24229";
    let cases: [(String, PinError); 6] = [
        (
            format!("{commit}\n{sha}\n"),
            PinError::MissingKey("version"),
        ),
        (
            format!("version 0.5.0\nversion 0.5.1\n{commit}\n{sha}\n"),
            PinError::DuplicateKey("version".to_owned()),
        ),
        (
            format!("version 0.5.0\n{commit}\n{sha}\nchannel stable\n"),
            PinError::UnknownKey("channel".to_owned()),
        ),
        (
            format!("version\n{commit}\n{sha}\n"),
            PinError::Malformed(1),
        ),
        (
            format!("version 0.5.0\ncommit 7c45c1ad\n{sha}\n"),
            PinError::InvalidValue("commit"),
        ),
        (
            format!("version 0.5.0\n{commit}\nsha256-x86_64-unknown-linux-gnu XYZ\n"),
            PinError::InvalidValue("sha256-x86_64-unknown-linux-gnu"),
        ),
    ];
    for (text, error) in cases {
        assert_eq!(ModelSpecPin::parse(&text), Err(error), "{text}");
    }
}

#[test]
fn an_explicit_path_wins_over_the_variable_and_path() {
    let explicit = standin("checks");
    let found = locate_with(
        Some(&explicit),
        Some(standin("rejects").as_os_str()),
        Some(standins().join("bin").as_os_str()),
    )
    .unwrap();
    assert_eq!(found, explicit);
}

#[test]
fn the_modelspec_variable_wins_over_path() {
    let found = locate_with(
        None,
        Some(standin("rejects").as_os_str()),
        Some(standins().join("bin").as_os_str()),
    )
    .unwrap();
    assert_eq!(found, standin("rejects"));
}

#[test]
fn path_is_searched_for_a_modelspec_executable() {
    let path = std::env::join_paths([standins().join("nowhere"), standins().join("bin")]).unwrap();
    let found = locate_with(None, None, Some(&path)).unwrap();
    assert_eq!(found, standins().join("bin/modelspec"));
}

#[test]
fn a_missing_binary_is_a_typed_error() {
    let gone = standins().join("gone/modelspec");
    let empty = standins().join("nowhere");
    for (explicit, variable, path) in [
        (Some(gone.as_path()), None, Some(empty.as_os_str())),
        (
            None,
            Some(gone.as_os_str()),
            Some(standins().join("bin").as_os_str()),
        ),
        (None, None, Some(empty.as_os_str())),
        (None, None, None::<&OsStr>),
    ] {
        match locate_with(explicit, variable, path) {
            Err(ModelSpecError::NotFound { searched }) => assert!(!searched.is_empty()),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }
}

#[test]
fn a_binary_that_cannot_be_spawned_is_a_typed_error() {
    match ModelSpec::open(&standin("not-executable"), &pin()) {
        Err(ModelSpecError::Spawn { binary, .. }) => assert_eq!(binary, standin("not-executable")),
        other => panic!("expected Spawn, got {other:?}"),
    }
}

#[test]
fn a_version_mismatch_is_refused_before_any_check_runs() {
    match ModelSpec::open(&standin("old"), &pin()) {
        Err(ModelSpecError::VersionMismatch {
            pinned, reported, ..
        }) => {
            assert_eq!(pinned, "0.5.0");
            assert_eq!(reported, "modelspec 0.4.0");
        }
        other => panic!("expected VersionMismatch, got {other:?}"),
    }
    assert!(!standins().join("old-was-asked-to-check").exists());
}

#[test]
fn a_failing_version_query_is_a_typed_error() {
    match ModelSpec::open(&standin("no-version"), &pin()) {
        Err(ModelSpecError::VersionQueryFailed { code, stderr, .. }) => {
            assert_eq!(code, Some(2));
            assert_eq!(stderr, "unrecognized option");
        }
        other => panic!("expected VersionQueryFailed, got {other:?}"),
    }
}

#[test]
fn an_accepted_source_is_recorded_checked_and_may_be_trained() {
    let modelspec = open("checks");
    assert_eq!(modelspec.version(), "0.5.0");
    assert_eq!(
        modelspec.check(Path::new(SOURCE)).unwrap(),
        CheckOutcome::Checked
    );
    let mut source = source();
    assert_eq!(source.state(), ModelSourceState::Unchecked);
    assert_eq!(source.check(&modelspec).unwrap(), ModelSourceState::Checked);
    let checked = source.trainable().expect("a checked source is trainable");
    assert_eq!(checked.path(), Path::new(SOURCE));
    assert_eq!(checked.model_name(), "PatchRegressor");
    assert_eq!(checked.batch_size(), 16);
    assert_eq!(checked.model_spec_version(), "0.5.0");
}

#[test]
fn a_second_check_is_refused() {
    let modelspec = open("checks");
    let mut source = source();
    source.check(&modelspec).unwrap();
    assert!(matches!(
        source.check(&modelspec),
        Err(CheckError::ModelSourceAlreadyChecked {
            state: ModelSourceState::Checked
        })
    ));
    let mut rejected = source_rejected();
    assert!(matches!(
        rejected.check(&open("checks")),
        Err(CheckError::ModelSourceAlreadyChecked {
            state: ModelSourceState::Rejected
        })
    ));
}

fn source_rejected() -> ModelSource {
    let mut source = source();
    assert_eq!(
        source.check(&open("rejects")).unwrap(),
        ModelSourceState::Rejected
    );
    source
}

#[test]
fn a_refused_source_is_recorded_rejected_with_the_diagnostic() {
    let diagnostic = "E-TYPE-001: matmul shapes [16, 32] and [64, 8] do not agree";
    assert_eq!(
        open("rejects").check(Path::new(SOURCE)).unwrap(),
        CheckOutcome::Rejected {
            code: Some(1),
            diagnostic: diagnostic.to_owned()
        }
    );
    let source = source_rejected();
    assert_eq!(source.diagnostic(), Some(diagnostic));
}

#[test]
fn a_rejected_source_cannot_be_trained() {
    let source = source_rejected();
    assert_eq!(
        source.trainable().err(),
        Some(ModelNotChecked {
            state: ModelSourceState::Rejected
        })
    );
}

#[test]
fn an_unchecked_source_cannot_be_trained() {
    assert_eq!(
        source().trainable().err(),
        Some(ModelNotChecked {
            state: ModelSourceState::Unchecked
        })
    );
}

#[test]
fn a_check_that_cannot_spawn_leaves_the_source_unchecked() {
    let modelspec = open("vanishes");
    fs::remove_file(standin("vanishes")).unwrap();
    let mut source = source();
    match source.check(&modelspec) {
        Err(CheckError::ModelSpec(ModelSpecError::Spawn { .. })) => {}
        other => panic!("expected a spawn error, got {other:?}"),
    }
    assert_eq!(source.state(), ModelSourceState::Unchecked);
}
