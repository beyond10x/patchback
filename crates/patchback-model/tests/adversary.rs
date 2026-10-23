//! Adversarial cases for story:modelspec-pin-and-check. Stand-in binaries live in their own
//! directory under the cargo target, apart from `adapter.rs`'s, and are all written before the
//! first spawn (ETXTBSY).

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use patchback_model::{
    CheckOutcome, ModelSource, ModelSourceState, ModelSpec, ModelSpecError, ModelSpecPin,
    locate_with,
};

fn pin() -> ModelSpecPin {
    ModelSpecPin::repository().unwrap()
}

fn dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-adversary");
        let _ = fs::remove_dir_all(&dir);
        for sub in ["bin", "plain", "dirpath/modelspec"] {
            fs::create_dir_all(dir.join(sub)).unwrap();
        }
        let version = pin().version().to_owned();
        let header = format!("#!/bin/sh\nif [ \"$1\" = --version ]; then echo \"modelspec {version}\"; exit 0; fi\n");
        let scripts = [
            // What the real 0.5.0 binary does on `check <missing file>`: exit 1, stderr
            // "modelspec check <path>: No such file or directory (os error 2)".
            (
                "real-like",
                "if [ ! -e \"$2\" ]; then echo \"modelspec check $2: No such file or directory (os error 2)\" >&2; exit 1; fi\nexit 0",
            ),
            ("killed", "kill -9 $$"),
            ("stdout-only", "echo \"E-TYPE-001: on stdout\"; exit 1"),
            ("bin/modelspec", "exit 0"),
        ];
        for (name, body) in scripts {
            let path = dir.join(name);
            fs::write(&path, format!("{header}{body}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let plain = dir.join("plain/modelspec");
        fs::write(&plain, format!("{header}exit 0\n")).unwrap();
        fs::set_permissions(&plain, fs::Permissions::from_mode(0o644)).unwrap();
        dir
    })
}

fn open(name: &str) -> ModelSpec {
    ModelSpec::open(&dir().join(name), &pin()).unwrap()
}

/// The real pinned binary exits 1 for a source path that does not exist, the same status as a
/// model it refuses. The adapter records that as the terminal `Rejected` state, so a wrong
/// working directory or a typo permanently marks the source as refused by ModelSpec.
#[test]
fn adversary_a_missing_source_file_is_not_the_binarys_verdict() {
    let modelspec = open("real-like");
    let mut source = ModelSource::register(
        dir().join("no-such-dir/patch_regressor.mspec"),
        "PatchRegressor",
        16,
        &pin(),
    );
    let result = source.check(&modelspec);
    assert_ne!(
        source.state(),
        ModelSourceState::Rejected,
        "a source file that does not exist was recorded Rejected (terminal): result {result:?}, diagnostic {:?}",
        source.diagnostic()
    );
}

/// A check process killed by a signal gave no verdict, and the adapter records it as the
/// terminal `Rejected` state.
#[test]
fn adversary_a_check_killed_by_a_signal_is_not_a_rejection() {
    let modelspec = open("killed");
    let mut source = ModelSource::register(dir().join("killed"), "PatchRegressor", 16, &pin());
    let result = source.check(&modelspec);
    assert_ne!(
        source.state(),
        ModelSourceState::Rejected,
        "a check killed by SIGKILL was recorded Rejected (terminal): result {result:?}"
    );
}

/// `MODELSPEC=Cargo.toml` (a bare name of a file in the test's working directory, the crate root)
/// is accepted by `locate_with` because the file exists relative to the working directory, but
/// `Command::new` resolves a bare name through `PATH`, so the adapter runs a different file than
/// the one it located. Running the located file would fail with PermissionDenied (it is not
/// executable); a PATH search fails with NotFound.
#[test]
fn adversary_a_bare_named_binary_is_run_from_where_it_was_found() {
    let found = locate_with(None, Some("Cargo.toml".as_ref()), None).unwrap();
    assert_eq!(found, Path::new("Cargo.toml"));
    match ModelSpec::open(&found, &pin()) {
        Err(ModelSpecError::Spawn { error, .. }) => assert_ne!(
            error.kind(),
            ErrorKind::NotFound,
            "located ./Cargo.toml but spawned a PATH lookup of `Cargo.toml`: {error}"
        ),
        other => panic!("expected a spawn error, got {other:?}"),
    }
}

/// Mutant guard: the stdout fallback for an empty stderr is not exercised by `adapter.rs`.
#[test]
fn adversary_a_rejection_with_empty_stderr_carries_stdout() {
    assert_eq!(
        open("stdout-only").check(Path::new("x.mspec")).unwrap(),
        CheckOutcome::Rejected {
            code: 1,
            diagnostic: "E-TYPE-001: on stdout".to_owned()
        }
    );
}

/// Mutant guard: an empty `MODELSPEC` falls through to `PATH`, and a non-executable file or a
/// directory named `modelspec` on `PATH` is skipped.
#[test]
fn adversary_path_search_skips_what_cannot_run() {
    let path = std::env::join_paths([
        dir().join("plain"),
        dir().join("dirpath"),
        dir().join("bin"),
    ])
    .unwrap();
    let found = locate_with(None, Some("".as_ref()), Some(&path)).unwrap();
    assert_eq!(found, dir().join("bin/modelspec"));
}
