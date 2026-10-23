//! The real pinned ModelSpec binary on `models/patch_regressor.mspec` (DESIGN.md § 8.1, § 15).
//!
//! The binary is found as the adapter finds it: `MODELSPEC`, then a `modelspec` on `PATH`. When
//! neither names one, the test prints a `SKIPPED` line and passes, unless
//! `PATCHBACK_REQUIRE_MODELSPEC` is set, as CI sets it: then a missing binary fails.

use std::io::Write;
use std::path::PathBuf;

use patchback_model::{
    ModelSource, ModelSourceState, ModelSpec, ModelSpecError, ModelSpecPin, locate,
};

#[test]
fn the_pinned_modelspec_checks_the_design_model() {
    let pin = ModelSpecPin::repository().unwrap();
    let binary = match locate(None) {
        Ok(binary) => binary,
        Err(ModelSpecError::NotFound { searched }) => {
            assert!(
                std::env::var_os("PATCHBACK_REQUIRE_MODELSPEC").is_none(),
                "PATCHBACK_REQUIRE_MODELSPEC is set and no modelspec binary was found in {searched:?}"
            );
            // Written to the process's stderr, not through eprintln!, so the harness's output
            // capture does not hide it from the log.
            let _ = writeln!(
                std::io::stderr(),
                "SKIPPED the_pinned_modelspec_checks_the_design_model: no modelspec {} binary (MODELSPEC unset, none of {} PATH entries has one)",
                pin.version(),
                searched.len()
            );
            return;
        }
        Err(other) => panic!("{other}"),
    };
    let modelspec = ModelSpec::open(&binary, &pin).unwrap_or_else(|e| panic!("{e}"));
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models/patch_regressor.mspec");
    let mut source = ModelSource::register(&path, "PatchRegressor", 16);
    let state = source.check(&modelspec).unwrap();
    assert_eq!(
        state,
        ModelSourceState::Checked,
        "modelspec {} rejected the § 8.1 model: {:?}",
        pin.version(),
        source.diagnostic()
    );
    source.trainable().unwrap();
}
