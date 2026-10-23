//! The ModelSpec adapter against stand-in binaries: shell scripts this test writes under the cargo
//! target, so every verdict of `modelspec --version` and `modelspec check` is chosen here.
//! `pinned_binary.rs` runs the real pinned binary.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use patchback_model::{
    CheckError, CheckOutcome, DEFAULT_TIMEOUT, ModelNotChecked, ModelSource, ModelSourceState,
    ModelSpec, ModelSpecError, ModelSpecPin, PinError, locate_with,
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
            (
                "records",
                format!(
                    "case \"$1\" in --version) echo \"modelspec {version}\";; check) touch {};; *) exit 64;; esac",
                    dir.join("records-was-asked-to-check").display()
                ),
            ),
            (
                "killed",
                format!("case \"$1\" in --version) echo \"modelspec {version}\";; *) kill -9 $$;; esac"),
            ),
            (
                "usage-error",
                format!(
                    "case \"$1\" in --version) echo \"modelspec {version}\";; *) echo \"error: unexpected argument\" >&2; exit 2;; esac"
                ),
            ),
            (
                "sleeps",
                format!("case \"$1\" in --version) echo \"modelspec {version}\";; *) exec sleep 30;; esac"),
            ),
            (
                "loud",
                format!(
                    "case \"$1\" in --version) echo \"modelspec {version}\";; *) head -c 1000000 /dev/zero | tr '\\0' o; head -c 1000000 /dev/zero | tr '\\0' e >&2; exit 1;; esac"
                ),
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
        let unreadable = dir.join("unreadable.mspec");
        fs::write(&unreadable, "model X {}\n").unwrap();
        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o644)).unwrap();
        fs::create_dir_all(dir.join("a-directory.mspec")).unwrap();
        dir
    })
}

fn standin(name: &str) -> PathBuf {
    standins().join(name)
}

fn open(name: &str) -> ModelSpec {
    ModelSpec::open(&standin(name), &pin()).expect("the stand-in reports the pinned version")
}

/// The repository root: `cargo test` runs in the crate directory, and `check` now refuses a
/// source it cannot read before spawning.
fn repository_file(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn source() -> ModelSource {
    ModelSource::register(repository_file(SOURCE), "PatchRegressor", 16, &pin())
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
    assert_eq!(checked.path(), repository_file(SOURCE));
    assert_eq!(checked.model_name(), "PatchRegressor");
    assert_eq!(checked.batch_size(), 16);
    assert_eq!(checked.model_spec_version(), "0.5.0");
    assert_eq!(checked.model_spec_pin(), &pin());
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
            code: 1,
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

/// Mode 000 on a file for as long as the guard lives. A file left unreadable under `target/` makes
/// `git` fail when a worktree is archived, so the mode goes back to 0644 even when the test panics.
struct Unreadable(PathBuf);

impl Unreadable {
    fn new(path: PathBuf) -> Self {
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        Unreadable(path)
    }
}

impl Drop for Unreadable {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o644));
    }
}

/// Fix 1: a source that is missing, a directory or unreadable is refused before any spawn, and the
/// source stays `Unchecked`; the binary's exit 1 for an unreadable path is not its verdict.
#[test]
fn an_unreadable_source_is_refused_before_any_spawn_and_stays_unchecked() {
    let _unreadable = Unreadable::new(standins().join("unreadable.mspec"));
    let marker = standins().join("records-was-asked-to-check");
    let modelspec = open("records");
    for path in [
        standins().join("no-such-dir/patch_regressor.mspec"),
        standins().join("a-directory.mspec"),
        standins().join("unreadable.mspec"),
    ] {
        let mut source = ModelSource::register(&path, "PatchRegressor", 16, &pin());
        match source.check(&modelspec) {
            Err(CheckError::ModelSpec(ModelSpecError::SourceUnreadable {
                source: named, ..
            })) => {
                assert_eq!(named, path)
            }
            other => panic!(
                "expected SourceUnreadable for {}, got {other:?}",
                path.display()
            ),
        }
        assert_eq!(source.state(), ModelSourceState::Unchecked);
        assert_eq!(source.diagnostic(), None);
    }
    assert!(
        !marker.exists(),
        "the binary was spawned for an unreadable source"
    );
}

/// Fix 2: a check ended by a signal gave no verdict.
#[test]
fn a_check_ended_by_a_signal_is_an_error_and_leaves_the_source_unchecked() {
    let modelspec = open("killed");
    assert!(matches!(
        modelspec.check(&repository_file(SOURCE)),
        Err(ModelSpecError::Signalled { .. })
    ));
    let mut source = source();
    match source.check(&modelspec) {
        Err(CheckError::ModelSpec(ModelSpecError::Signalled { binary })) => {
            assert_eq!(binary, standin("killed"))
        }
        other => panic!("expected Signalled, got {other:?}"),
    }
    assert_eq!(source.state(), ModelSourceState::Unchecked);
}

/// The class of fixes 1 and 2: only exit 0 (accepted) and exit 1 (refused) are the binary's
/// verdict. Any other exit status, such as clap's usage error 2, leaves the source `Unchecked`.
#[test]
fn an_exit_status_that_is_not_a_verdict_leaves_the_source_unchecked() {
    let mut source = source();
    match source.check(&open("usage-error")) {
        Err(CheckError::ModelSpec(ModelSpecError::CheckFailed {
            code, diagnostic, ..
        })) => {
            assert_eq!(code, 2);
            assert_eq!(diagnostic, "error: unexpected argument");
        }
        other => panic!("expected CheckFailed, got {other:?}"),
    }
    assert_eq!(source.state(), ModelSourceState::Unchecked);
}

/// Fix 4: `open` takes the documented default bound.
#[test]
fn the_default_timeout_bounds_every_run() {
    assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(60));
    assert_eq!(open("checks").timeout(), DEFAULT_TIMEOUT);
}

/// Fix 4: a check that outlasts the caller's bound is killed and waited for, and the source stays
/// `Unchecked`.
#[test]
fn a_check_that_outlasts_the_timeout_is_killed_and_leaves_the_source_unchecked() {
    let bound = Duration::from_millis(300);
    let modelspec = ModelSpec::open_with_timeout(&standin("sleeps"), &pin(), bound).unwrap();
    assert_eq!(modelspec.timeout(), bound);
    let mut source = source();
    let started = Instant::now();
    match source.check(&modelspec) {
        Err(CheckError::ModelSpec(ModelSpecError::TimedOut { timeout, .. })) => {
            assert_eq!(timeout, bound)
        }
        other => panic!("expected TimedOut, got {other:?}"),
    }
    let elapsed = started.elapsed();
    assert!(elapsed < Duration::from_secs(10), "waited {elapsed:?}");
    assert_eq!(source.state(), ModelSourceState::Unchecked);
}

/// Fix 4: stdout and stderr are drained while the child runs, so output larger than a pipe buffer
/// (64 KiB on Linux) on both streams cannot stall the check.
#[test]
fn a_check_with_more_output_than_a_pipe_holds_completes() {
    let modelspec =
        ModelSpec::open_with_timeout(&standin("loud"), &pin(), Duration::from_secs(20)).unwrap();
    match modelspec.check(&repository_file(SOURCE)).unwrap() {
        CheckOutcome::Rejected { code, diagnostic } => {
            assert_eq!(code, 1);
            assert_eq!(diagnostic.len(), 1_000_000);
            assert!(diagnostic.bytes().all(|b| b == b'e'));
        }
        other => panic!("expected Rejected, got {other:?}"),
    }
}

/// Fix 6: the source carries the pin it was registered with, and the checked model records it.
#[test]
fn a_checked_model_records_the_pin_it_was_registered_with() {
    let mut source = source();
    assert_eq!(source.model_spec_pin(), &pin());
    source.check(&open("checks")).unwrap();
    assert_eq!(source.trainable().unwrap().model_spec_pin(), &pin());
}

/// Fix 6: a binary opened under another pin does not check a source registered under this one.
#[test]
fn a_source_is_not_checked_by_a_binary_of_another_pin() {
    let other = ModelSpecPin::parse(
        "version 0.4.0\ncommit 7c45c1adaf6e0cc0ba05f18878b31bfdc57cc529\nsha256-x86_64-unknown-linux-gnu 171d2880ddd197ef91db8f36da2e6268676aa5aa5adc28d28ea8bc39cae24229\n",
    )
    .unwrap();
    let mut source = ModelSource::register(repository_file(SOURCE), "PatchRegressor", 16, &other);
    match source.check(&open("checks")) {
        Err(CheckError::OtherPin { registered, binary }) => {
            assert_eq!(registered, "0.4.0");
            assert_eq!(binary, "0.5.0");
        }
        other => panic!("expected OtherPin, got {other:?}"),
    }
    assert_eq!(source.state(), ModelSourceState::Unchecked);
}
