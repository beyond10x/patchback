//! The weights of a real pinned ModelSpec run are exported and read back (DESIGN.md § 8.2 steps
//! 2–3, § 12.1 "Model bridge"; Da9-4, Da9-5).
//!
//! The binary is found as `pinned_training.rs` finds it: `MODELSPEC`, then a `modelspec` on
//! `PATH`. When neither names one, the test prints a `SKIPPED` line and passes, unless
//! `PATCHBACK_REQUIRE_MODELSPEC` is set, as CI sets it: then a missing binary fails.
//!
//! The run is `pinned_training.rs`'s: one generation of 64 / 16 / 16 samples, the train and
//! validation splits written z-scored, `modelspec train --steps 4 --eval`. Its whole stdout
//! (`init`, `step`, `param` and `eval` lines) must pass the export's grammar, and every value
//! the export parsed with `f32::from_str` must re-print with `{}` as the token ModelSpec printed.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use patchback_analysis::FEATURES_1_VERSION;
use patchback_dataset::{Generated, Generator, SplitCounts, SplitRole};
use patchback_dataset::{dense, training_stats};
use patchback_model::{
    Backend, DEFAULT_TRAINING_TIMEOUT, MAX_HEADER_BYTES, ModelSource, ModelSourceState, ModelSpec,
    ModelSpecError, ModelSpecPin, PAYLOAD_BYTES, RECOMMENDED_FILE_NAME, TrainingRun,
    TrainingRunState, W1_ELEMENTS, W1_NAME, W2_ELEMENTS, W2_NAME, WeightArtifact, locate,
};

const TRAIN_ROWS: i64 = 64;
const STEPS: u32 = 4;

/// Written to the process's stderr, not through eprintln!, so the harness's output capture does
/// not hide it from the log.
fn say(line: &str) {
    let _ = writeln!(std::io::stderr(), "{line}");
}

/// The pinned binary, or `None` after a `SKIPPED` line when there is none and none is required.
fn binary(test: &str, pin: &ModelSpecPin) -> Option<PathBuf> {
    match locate(None) {
        Ok(binary) => Some(binary),
        Err(ModelSpecError::NotFound { searched }) => {
            assert!(
                std::env::var_os("PATCHBACK_REQUIRE_MODELSPEC").is_none(),
                "PATCHBACK_REQUIRE_MODELSPEC is set and no modelspec binary was found in {searched:?}"
            );
            say(&format!(
                "SKIPPED {test}: no modelspec {} binary (MODELSPEC unset, none of {} PATH entries has one)",
                pin.version(),
                searched.len()
            ));
            None
        }
        Err(other) => panic!("{other}"),
    }
}

fn write(generated: &mut Generated, role: SplitRole, dir: &Path) {
    let split_dir = dir.join(role.dir());
    fs::create_dir_all(&split_dir).unwrap();
    let split = generated.split_id(role);
    let stats = training_stats(generated).unwrap();
    dense::write_split(
        &mut generated.store,
        split,
        &split_dir,
        &stats,
        dense::create_new,
    )
    .unwrap();
}

/// The value tokens of the `param` line of `name`, as printed.
fn printed<'a>(stdout: &'a str, name: &str) -> Vec<&'a str> {
    let prefix = format!("param {name}: ");
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with(&prefix))
        .collect();
    assert_eq!(lines.len(), 1, "one param line for {name}");
    let (_, values) = lines[0].split_once("= [").unwrap();
    values.strip_suffix(']').unwrap().split(", ").collect()
}

#[test]
fn the_weights_of_a_pinned_run_are_exported_and_read_back() {
    let test = "the_weights_of_a_pinned_run_are_exported_and_read_back";
    let pin = ModelSpecPin::repository().unwrap();
    let Some(binary) = binary(test, &pin) else {
        return;
    };
    let modelspec = ModelSpec::open(&binary, &pin).unwrap_or_else(|e| panic!("{e}"));
    let model =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models/patch_regressor.mspec");
    let mut source = ModelSource::register(&model, "PatchRegressor", 16, &pin);
    assert_eq!(source.check(&modelspec).unwrap(), ModelSourceState::Checked);

    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-pinned-export");
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    let started = Instant::now();
    let counts = SplitCounts {
        train: TRAIN_ROWS,
        validation: 16,
        test: 16,
    };
    let mut generated = Generator::new(42, counts, dir.clone()).run().unwrap();
    write(&mut generated, SplitRole::Train, &dir);
    write(&mut generated, SplitRole::Validation, &dir);
    say(&format!(
        "{test}: generation and writing of {TRAIN_ROWS}/16/16 samples {:?}",
        started.elapsed()
    ));

    let mut run = TrainingRun::plan(Backend::Cpu, &pin).unwrap();
    run.bind(&source.trainable().unwrap()).unwrap();
    let started = Instant::now();
    run.start(
        &generated.store,
        generated.split_id(SplitRole::Train),
        Some(generated.split_id(SplitRole::Validation)),
        &modelspec,
        STEPS,
        DEFAULT_TRAINING_TIMEOUT,
    )
    .unwrap();
    let trained = started.elapsed();
    let state = run.finish().unwrap();
    say(&format!(
        "{test}: modelspec train --steps {STEPS} --eval {trained:?}; {state:?}"
    ));
    assert_eq!(
        state,
        TrainingRunState::Completed,
        "{:?}: {}",
        run.failure(),
        run.stderr()
    );
    assert_eq!(run.output_error(), None);

    // Da9-5: the whole stdout of the real run is in the grammar; say which kinds it held.
    let stdout = run.stdout();
    let mut kinds: Vec<(&str, usize)> = Vec::new();
    for line in stdout.lines() {
        let kind = line.split(' ').next().unwrap_or("");
        match kinds.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, count)) => *count += 1,
            None => kinds.push((kind, 1)),
        }
    }
    say(&format!(
        "{test}: stdout {} bytes, {} lines, by first word {kinds:?}",
        stdout.len(),
        stdout.lines().count()
    ));
    for kind in ["init", "step", "param", "eval"] {
        assert!(
            kinds.iter().any(|(k, _)| *k == kind),
            "the real stdout has no {kind} line: {kinds:?}"
        );
    }

    let path = dir.join(RECOMMENDED_FILE_NAME);
    let started = Instant::now();
    let artifact =
        WeightArtifact::export(&run, FEATURES_1_VERSION, &path).unwrap_or_else(|e| panic!("{e}"));
    let read = WeightArtifact::read(&path).unwrap_or_else(|e| panic!("{e}"));
    say(&format!(
        "{test}: export and read back {:?}",
        started.elapsed()
    ));
    assert_eq!(read, artifact);
    assert_eq!(artifact.modelspec_version(), pin.version());
    assert_eq!(artifact.modelspec_commit(), pin.commit());
    assert_eq!(artifact.feature_contract(), FEATURES_1_VERSION);

    let bytes = fs::read(&path).unwrap();
    let header_len = bytes.len() - PAYLOAD_BYTES;
    assert!(header_len <= MAX_HEADER_BYTES);
    let header = std::str::from_utf8(&bytes[..header_len]).unwrap();
    say(&format!("{test}: header as written:\n{header}"));

    // Da9-4: every value re-printed with `{}` is the token ModelSpec printed.
    let mut equal = 0;
    let mut first_other: Option<(String, String)> = None;
    for (name, values, count) in [
        (W1_NAME, artifact.w1(), W1_ELEMENTS),
        (W2_NAME, artifact.w2(), W2_ELEMENTS),
    ] {
        let tokens = printed(stdout, name);
        assert_eq!(tokens.len(), count, "{name}");
        assert_eq!(values.len(), count, "{name}");
        for (token, value) in tokens.iter().zip(values) {
            let parsed: f32 = token.parse().unwrap();
            assert_eq!(parsed.to_bits(), value.to_bits(), "{name} {token}");
            if format!("{value}") == *token {
                equal += 1;
            } else if first_other.is_none() {
                first_other = Some(((*token).to_owned(), format!("{value}")));
            }
        }
    }
    say(&format!(
        "{test}: {equal} of {} values re-print as printed; first that does not: {first_other:?}",
        W1_ELEMENTS + W2_ELEMENTS
    ));
    assert_eq!(
        equal,
        W1_ELEMENTS + W2_ELEMENTS,
        "first token that does not round-trip (printed, re-printed): {first_other:?}"
    );
}
