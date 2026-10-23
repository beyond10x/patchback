//! Fix 3: a slash-free relative binary path is run as `./<name>`, so the file located is the file
//! spawned. This file holds one test because it changes the process's working directory.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use patchback_model::{ModelSpec, ModelSpecPin, locate_with};

#[test]
fn a_bare_relative_binary_runs_the_file_found_in_the_working_directory() {
    let pin = ModelSpecPin::repository().unwrap();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-bare-name");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let script = format!(
        "#!/bin/sh\ncase \"$1\" in --version) echo \"modelspec {}\";; *) exit 0;; esac\n",
        pin.version()
    );
    for name in ["patchback-bare-standin", "modelspec"] {
        let path = dir.join(name);
        fs::write(&path, &script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::env::set_current_dir(&dir).unwrap();

    // `MODELSPEC=patchback-bare-standin`: no such program is on PATH, so a PATH lookup fails.
    let found = locate_with(None, Some("patchback-bare-standin".as_ref()), None).unwrap();
    assert_eq!(found, Path::new("patchback-bare-standin"));
    let modelspec = ModelSpec::open(&found, &pin).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(modelspec.version(), pin.version());

    // An empty PATH entry is the working directory (POSIX); the `modelspec` it finds is run.
    let found = locate_with(None, None, Some(":".as_ref())).unwrap();
    assert_eq!(found, Path::new("modelspec"));
    ModelSpec::open(&found, &pin).unwrap_or_else(|e| panic!("{e}"));
}
