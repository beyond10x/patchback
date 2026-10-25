//! `patchback fit --weights <path> --normalization <path>` through the binary
//! (story:regressor-seeded-fit, Da11-2, Da11-6, Da11-10, Da11-11): the seeded run's `run.meta`
//! records, byte for byte; the clamp warning on stdout and in `report.txt` when the prediction
//! clamped and nowhere when it did not; one flag without the other is a usage error (2); a
//! missing or corrupt weight artifact, a missing or malformed statistics file and an input of
//! another feature contract are refused (1) before anything is written, the mismatch before
//! the reference's silence. The default path's files are held by `tests/fit.rs` and
//! `tests/fit_results.rs`, unchanged.
//!
//! Every artifact is written here from weights the test chooses (the `weights/1` bytes of
//! `website/docs/reference/files.md` § Exported weights, the a10 writer copied, Da11-14), and
//! every statistics file is centred on the reference's own `features/1` vector, computed here
//! through `patchback_analysis`, so the prediction is near the chosen targets.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_analysis::{
    AnalysisConfig, FeatureContractDefinition, FeatureRegistry, NoteSpan, analyze_clip,
    extract_features,
};
use patchback_core::digest::{hex, sha256};
use patchback_core::patch::format_decimal;
use patchback_model::{ModelSpecPin, W1_ELEMENTS, W1_SHAPE, W2_SHAPE};

const HIDDEN: usize = W1_SHAPE[1];
const OUTPUTS: usize = W2_SHAPE[1];

const TARGETS: [f32; OUTPUTS] = [0.2, 0.3, 0.4, 0.6, 0.7, 0.8, 0.35, 0.65];
const CLAMPED_TARGETS: [f32; OUTPUTS] = [1.5, -0.5, 2.0, 0.5, 0.25, 0.75, 0.4, 0.6];

const SMALL: [&str; 6] = ["--max-renders", "12", "--max-restarts", "1", "--seed", "42"];

/// A fresh, empty directory for one test under Cargo's per-target scratch directory.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-fit-seeded")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("output is UTF-8")
}

/// The errors a domain of `spec/` declares.
fn declared_errors(domain: &str) -> Vec<String> {
    let path = format!(
        "{}/../../spec/domains/{domain}.yaml",
        env!("CARGO_MANIFEST_DIR")
    );
    let spec = fs::read_to_string(path).unwrap();
    let errors = spec.split_once("\nerrors:\n").unwrap().1;
    let errors = errors.split_once("\ncommands:\n").unwrap().0;
    errors
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- name: "))
        .map(str::to_owned)
        .collect()
}

fn write(dir: &Path, name: &str, contents: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, contents).unwrap();
    path
}

fn patchback(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(args)
        .output()
        .expect("the patchback binary runs")
}

/// The fixed chain with the filter cutoff at 179 Hz (`p = 0.25`) and every other parameter at
/// its contract default (`p = 0.5`), as `tests/fit_results.rs` renders it.
const REFERENCE_PATCH: &str = "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n\
     node 1 NoteInput\nnode 2 SawOscillator\nnode 3 Drive\nparam 3 drive 12\n\
     node 4 LowpassFilter\nparam 4 filter_cutoff 178.88543819998318\nparam 4 resonance_q 6.25\n\
     node 5 AdsrEnvelope\nparam 5 attack 0.044721359549995794\nparam 5 decay 0.1\n\
     param 5 sustain 0.5\nparam 5 release 0.1\nnode 6 Vca\nnode 7 Output\n\
     param 7 output_gain -12\n\
     edge 1 pitch Control 2 pitch Control\nedge 1 gate Event 5 gate Event\n\
     edge 2 out Audio 3 in Audio\nedge 3 out Audio 4 in Audio\nedge 4 out Audio 6 in Audio\n\
     edge 5 env Control 6 gain Control\nedge 6 out Audio 7 in Audio\n";

/// One note of MIDI 45 at velocity 0.8, 0.25 s.
const NOTES: &str = "patchback-notes 1\nsample_rate_hz 48000\npatch_id 7f0c0000-0000-4000-8000-000000000911\n\
     note 0 12000 45 0.8\n";

/// The inputs of one seeded fit: the reference WAV, its notes, and the reference's raw
/// `features/1` vector.
struct Inputs {
    reference: PathBuf,
    notes: PathBuf,
    features: Vec<f64>,
}

fn inputs(dir: &Path) -> Inputs {
    let patch = write(dir, "reference.patch", REFERENCE_PATCH.as_bytes());
    let notes = write(dir, "notes.events", NOTES.as_bytes());
    let reference = dir.join("reference.wav");
    let rendered = patchback(&[
        "render",
        "--patch",
        patch.to_str().unwrap(),
        "--notes",
        notes.to_str().unwrap(),
        "--out",
        reference.to_str().unwrap(),
    ]);
    assert_eq!(
        rendered.status.code(),
        Some(0),
        "{}",
        text(&rendered.stderr)
    );
    let clip = patchback_audio::decode(&fs::read(&reference).unwrap()).unwrap();
    let analysis = analyze_clip(&clip, &AnalysisConfig::default()).unwrap();
    let mut registry = FeatureRegistry::new();
    let id = registry
        .define(FeatureContractDefinition::features_1())
        .unwrap();
    registry.publish(id).unwrap();
    let span = NoteSpan {
        start_sample: 0,
        end_sample: 12_000.min(clip.frame_count()),
    };
    let features = extract_features(registry.get(id).unwrap(), &analysis, clip.samples(), span)
        .unwrap()
        .values;
    Inputs {
        reference,
        notes,
        features,
    }
}

/// The bytes of a `weights/1` artifact whose output `k` is `targets[k]` times the first
/// normalized feature, for `features`, under the repository's ModelSpec pin (the a10 writer,
/// copied).
fn artifact_bytes(targets: &[f32; OUTPUTS], features: &str) -> Vec<u8> {
    let mut w1 = vec![0.0f32; W1_ELEMENTS];
    let mut w2 = vec![0.0f32; HIDDEN * OUTPUTS];
    for (k, target) in targets.iter().enumerate() {
        w1[k] = 1.0;
        w2[k * OUTPUTS + k] = *target;
    }
    let payload: Vec<u8> = w1.iter().chain(&w2).flat_map(|v| v.to_le_bytes()).collect();
    let pin = ModelSpecPin::repository().unwrap();
    let header = format!(
        "weights/1\nmodel PatchRegressor\nmodelspec {} {}\nfeatures {features}\nw1 32 64\nw2 64 8\nsha256 {}\n\n",
        pin.version(),
        pin.commit(),
        hex(&sha256(&payload))
    );
    [header.as_bytes(), &payload].concat()
}

/// A `normalization.stats` text of `version` centred on `features`: mean `x[i] - 1`, standard
/// deviation 1.
fn stats_text(version: &str, features: &[f64]) -> String {
    let mut text = format!(
        "patchback-normalization-stats 1\nfeature_contract {version}\nsplit train\nrows 16\nstd population\n"
    );
    for (i, x) in features.iter().enumerate() {
        text.push_str(&format!(
            "feature {i} {} {}\n",
            format_decimal(x - 1.0),
            format_decimal(1.0)
        ));
    }
    text
}

fn seeded_fit(inputs: &Inputs, weights: &Path, stats: &Path, out: &Path) -> Output {
    let mut args = vec![
        "fit",
        "--input",
        inputs.reference.to_str().unwrap(),
        "--notes",
        inputs.notes.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--weights",
        weights.to_str().unwrap(),
        "--normalization",
        stats.to_str().unwrap(),
    ];
    args.extend_from_slice(&SMALL);
    patchback(&args)
}

/// Exit 1, nothing on stdout, one stderr line `patchback: fit refused: <error>: …` naming
/// `error`, which `spec/domains/<domain>.yaml` declares, and nothing at `out`.
fn assert_refused(output: &Output, domain: &str, error: &str, out: &Path) -> String {
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 1, "{stderr}");
    assert!(
        lines[0].starts_with(&format!("patchback: fit refused: {error}: ")),
        "{stderr}"
    );
    assert!(
        declared_errors(domain)
            .iter()
            .any(|declared| declared == error),
        "{error} is not an error of spec/domains/{domain}.yaml"
    );
    assert!(!out.exists(), "a refused fit writes nothing");
    lines[0].to_owned()
}

/// Both flags: exit 0, stdout `init regressor-prediction`, and `run.meta` byte for byte: the
/// three seeded records after `init`, the rest as a default run writes them. The prediction
/// clamped nothing, so neither stdout nor `report.txt` carries a clamp warning.
#[test]
fn both_flags_seed_the_fit_and_write_the_seeded_run_meta() {
    let dir = scratch("seeded");
    let inputs = inputs(&dir);
    let artifact = artifact_bytes(&TARGETS, "features/1");
    let weights = write(&dir, "regressor.weights", &artifact);
    let stats = stats_text("features/1", &inputs.features);
    let stats_path = write(&dir, "normalization.stats", stats.as_bytes());
    let out = dir.join("results");
    let output = seeded_fit(&inputs, &weights, &stats_path, &out);
    let stdout = text(&output.stdout);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[1], "init regressor-prediction", "{stdout}");
    assert_eq!(lines.last(), Some(&"limited false"), "{stdout}");
    assert!(!stdout.contains("warning"), "{stdout}");

    let pin = ModelSpecPin::repository().unwrap();
    let payload_sha256 = hex(&sha256(&artifact[artifact.len() - 10_240..]));
    let expected = format!(
        "patchback-fit-run-meta 1\npatchback {}\ninit regressor-prediction\n\
         weights sha256 {payload_sha256}\nweights_modelspec {} {}\nnormalization sha256 {}\n\
         seed 42\ntopology subtractive-mono/1\nfeature_contract features/1\nprng xoshiro256**\n\
         modelspec {}\nreference sha256 {}\nnotes sha256 {}\nmax_renders 12\n\
         max_wall_time_ms 60000\nmax_restarts 1\nimprovement_threshold 0.0001\nblock_size 256\n",
        env!("CARGO_PKG_VERSION"),
        pin.version(),
        pin.commit(),
        hex(&sha256(stats.as_bytes())),
        pin.version(),
        hex(&sha256(&fs::read(&inputs.reference).unwrap())),
        hex(&sha256(NOTES.as_bytes())),
    );
    assert_eq!(fs::read_to_string(out.join("run.meta")).unwrap(), expected);
    let report = fs::read_to_string(out.join("report.txt")).unwrap();
    assert!(!report.contains("clamped"), "{report}");
}

/// A prediction with three outputs outside [0, 1]: the run starts clamped, and stdout and
/// `report.txt` each end with `warning clamped 3 of 8 predictions` (Da11-11).
#[test]
fn a_clamped_prediction_warns_on_stdout_and_in_the_report() {
    let dir = scratch("clamped");
    let inputs = inputs(&dir);
    let weights = write(
        &dir,
        "regressor.weights",
        &artifact_bytes(&CLAMPED_TARGETS, "features/1"),
    );
    let stats = write(
        &dir,
        "normalization.stats",
        stats_text("features/1", &inputs.features).as_bytes(),
    );
    let out = dir.join("results");
    let output = seeded_fit(&inputs, &weights, &stats, &out);
    let stdout = text(&output.stdout);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(
        stdout.ends_with("limited false\nwarning clamped 3 of 8 predictions\n"),
        "{stdout}"
    );
    let report = fs::read_to_string(out.join("report.txt")).unwrap();
    assert!(
        report.ends_with("\nwarning clamped 3 of 8 predictions\n"),
        "{report}"
    );
    assert_eq!(report.matches("clamped").count(), 1, "{report}");
}

/// Each flag requires the other: one alone is a usage error (2) and nothing is written.
#[test]
fn one_flag_without_the_other_is_a_usage_error() {
    let dir = scratch("one-flag");
    let (input, notes, out) = (dir.join("a.wav"), dir.join("a.events"), dir.join("run"));
    for extra in [
        ["--weights", "regressor.weights"],
        ["--normalization", "normalization.stats"],
    ] {
        let output = patchback(&[
            "fit",
            "--input",
            input.to_str().unwrap(),
            "--notes",
            notes.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            extra[0],
            extra[1],
        ]);
        assert_eq!(output.status.code(), Some(2), "{extra:?}");
        assert!(output.stdout.is_empty());
        let stderr = text(&output.stderr);
        assert!(
            stderr.starts_with("error: the following required arguments were not provided"),
            "{stderr}"
        );
        assert!(!out.exists());
    }
}

/// A `--weights` path with no file is `patchback.model.UnknownWeightArtifact`; a file that is
/// not a `weights/1` artifact whose checksum verifies is `patchback.model.CorruptWeightArtifact`
/// (Da11-6); nothing is written.
#[test]
fn a_missing_or_corrupt_weight_artifact_is_refused() {
    let dir = scratch("weights");
    let inputs = inputs(&dir);
    let stats = write(
        &dir,
        "normalization.stats",
        stats_text("features/1", &inputs.features).as_bytes(),
    );
    let out = dir.join("results");
    let missing = dir.join("missing.weights");
    let line = assert_refused(
        &seeded_fit(&inputs, &missing, &stats, &out),
        "model",
        "patchback.model.UnknownWeightArtifact",
        &out,
    );
    assert!(line.contains(&missing.display().to_string()), "{line}");

    let mut corrupt = artifact_bytes(&TARGETS, "features/1");
    let last = corrupt.len() - 1;
    corrupt[last] ^= 0x01;
    let corrupt = write(&dir, "corrupt.weights", &corrupt);
    let line = assert_refused(
        &seeded_fit(&inputs, &corrupt, &stats, &out),
        "model",
        "patchback.model.CorruptWeightArtifact",
        &out,
    );
    assert!(line.contains("SHA-256"), "{line}");
}

/// A `--normalization` path with no file, or a file `read_stats` refuses, is
/// `patchback.analysis.NormalizationStatsNotFound`; nothing is written.
#[test]
fn a_missing_or_malformed_statistics_file_is_refused() {
    let dir = scratch("statistics");
    let inputs = inputs(&dir);
    let weights = write(
        &dir,
        "regressor.weights",
        &artifact_bytes(&TARGETS, "features/1"),
    );
    let out = dir.join("results");
    let missing = dir.join("missing.stats");
    let line = assert_refused(
        &seeded_fit(&inputs, &weights, &missing, &out),
        "analysis",
        "patchback.analysis.NormalizationStatsNotFound",
        &out,
    );
    assert!(line.contains(&missing.display().to_string()), "{line}");

    let truncated = stats_text("features/1", &inputs.features);
    let truncated = &truncated[..truncated.len() / 2];
    let malformed = write(&dir, "malformed.stats", truncated.as_bytes());
    assert_refused(
        &seeded_fit(&inputs, &weights, &malformed, &out),
        "analysis",
        "patchback.analysis.NormalizationStatsNotFound",
        &out,
    );
}

/// An artifact trained for `features/2`, or statistics of `features/3`, is
/// `patchback.fit.FeatureContractMismatch` naming the input; it is answered before the
/// reference's silence (StartFit's order), so a silent reference with a mismatched artifact
/// is refused for the mismatch.
#[test]
fn an_input_of_another_feature_contract_is_refused_before_silence() {
    let dir = scratch("mismatch");
    let inputs = inputs(&dir);
    let stats_text_1 = stats_text("features/1", &inputs.features);
    let stats = write(&dir, "normalization.stats", stats_text_1.as_bytes());
    let out = dir.join("results");
    let mismatched = write(
        &dir,
        "features-2.weights",
        &artifact_bytes(&TARGETS, "features/2"),
    );
    let line = assert_refused(
        &seeded_fit(&inputs, &mismatched, &stats, &out),
        "fit",
        "patchback.fit.FeatureContractMismatch",
        &out,
    );
    assert!(
        line.contains("weight artifact") && line.contains("features/2"),
        "{line}"
    );

    let weights = write(
        &dir,
        "regressor.weights",
        &artifact_bytes(&TARGETS, "features/1"),
    );
    let other = write(
        &dir,
        "features-3.stats",
        stats_text("features/3", &inputs.features).as_bytes(),
    );
    let line = assert_refused(
        &seeded_fit(&inputs, &weights, &other, &out),
        "fit",
        "patchback.fit.FeatureContractMismatch",
        &out,
    );
    assert!(
        line.contains("normalization statistics") && line.contains("features/3"),
        "{line}"
    );

    let silent = dir.join("silent.wav");
    let clip = patchback_audio::AudioClip::from_mono(vec![0.0; 24_000]).unwrap();
    patchback_audio::write_wav(&silent, &clip, patchback_audio::WavSampleFormat::Float32).unwrap();
    let silent_inputs = Inputs {
        reference: silent,
        notes: inputs.notes.clone(),
        features: inputs.features.clone(),
    };
    assert_refused(
        &seeded_fit(&silent_inputs, &mismatched, &stats, &out),
        "fit",
        "patchback.fit.FeatureContractMismatch",
        &out,
    );
    // With matching inputs the silent reference is refused for its silence.
    assert_refused(
        &seeded_fit(&silent_inputs, &weights, &stats, &out),
        "fit",
        "patchback.fit.ReferenceSilent",
        &out,
    );
}
