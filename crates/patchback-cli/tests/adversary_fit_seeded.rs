//! Adversary cases for `patchback fit --weights --normalization` (story:regressor-seeded-fit,
//! wave a11, pass 1), through the binary. Every artifact and statistics file is written here.

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
const SMALL: [&str; 6] = ["--max-renders", "2", "--max-restarts", "1", "--seed", "42"];

const REFERENCE_PATCH: &str = "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n\
     node 1 NoteInput\nnode 2 SawOscillator\nnode 3 Drive\nparam 3 drive 12\n\
     node 4 LowpassFilter\nparam 4 filter_cutoff 178.88543819998318\nparam 4 resonance_q 6.25\n\
     node 5 AdsrEnvelope\nparam 5 attack 0.044721359549995794\nparam 5 decay 0.1\n\
     param 5 sustain 0.5\nparam 5 release 0.1\nnode 6 Vca\nnode 7 Output\n\
     param 7 output_gain -12\n\
     edge 1 pitch Control 2 pitch Control\nedge 1 gate Event 5 gate Event\n\
     edge 2 out Audio 3 in Audio\nedge 3 out Audio 4 in Audio\nedge 4 out Audio 6 in Audio\n\
     edge 5 env Control 6 gain Control\nedge 6 out Audio 7 in Audio\n";

const NOTES: &str = "patchback-notes 1\nsample_rate_hz 48000\npatch_id 7f0c0000-0000-4000-8000-000000000911\n\
     note 0 12000 45 0.8\n";

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-adversary-fit-seeded")
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
        end_sample: 12_000,
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

fn artifact_bytes(w1: &[f32], w2: &[f32], features: &str) -> Vec<u8> {
    let payload: Vec<u8> = w1.iter().chain(w2).flat_map(|v| v.to_le_bytes()).collect();
    let pin = ModelSpecPin::repository().unwrap();
    let header = format!(
        "weights/1\nmodel PatchRegressor\nmodelspec {} {}\nfeatures {features}\nw1 32 64\nw2 64 8\nsha256 {}\n\n",
        pin.version(),
        pin.commit(),
        hex(&sha256(&payload))
    );
    [header.as_bytes(), &payload].concat()
}

fn targets_artifact() -> Vec<u8> {
    let mut w1 = vec![0.0f32; W1_ELEMENTS];
    let mut w2 = vec![0.0f32; HIDDEN * OUTPUTS];
    for (k, target) in TARGETS.iter().enumerate() {
        w1[k] = 1.0;
        w2[k * OUTPUTS + k] = *target;
    }
    artifact_bytes(&w1, &w2, "features/1")
}

fn stats_text(features: &[f64], newline: &str) -> String {
    let mut text = format!(
        "patchback-normalization-stats 1{newline}feature_contract features/1{newline}split train{newline}rows 16{newline}std population{newline}"
    );
    for (i, x) in features.iter().enumerate() {
        text.push_str(&format!(
            "feature {i} {} {}{newline}",
            format_decimal(x - 1.0),
            format_decimal(1.0)
        ));
    }
    text
}

fn fit_args<'a>(
    reference: &'a Path,
    notes: &'a Path,
    out: &'a Path,
    weights: &'a Path,
    stats: &'a Path,
) -> Vec<&'a str> {
    let mut args = vec![
        "fit",
        "--input",
        reference.to_str().unwrap(),
        "--notes",
        notes.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--weights",
        weights.to_str().unwrap(),
        "--normalization",
        stats.to_str().unwrap(),
    ];
    args.extend_from_slice(&SMALL);
    args
}

fn refusal(output: &Output) -> String {
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    stderr
}

/// Da11-12: a checksum-valid artifact of finite weights whose hidden unit overflows `f32` (two
/// weights of `f32::MAX` on inputs of about 1) times a zero output weight gives a NaN output,
/// `PredictError::NonFinitePrediction`. It is reachable through the binary and is refused as
/// the fit's NoValidCandidate naming the NaN; nothing is written.
#[test]
fn adversary_a_hidden_overflow_prediction_is_refused_through_the_binary() {
    let dir = scratch("hidden-overflow");
    let inputs = inputs(&dir);
    let mut w1 = vec![0.0f32; W1_ELEMENTS];
    w1[0] = f32::MAX;
    w1[HIDDEN] = f32::MAX;
    let w2 = vec![0.0f32; HIDDEN * OUTPUTS];
    let weights = write(
        &dir,
        "overflow.weights",
        &artifact_bytes(&w1, &w2, "features/1"),
    );
    let stats = write(
        &dir,
        "normalization.stats",
        stats_text(&inputs.features, "\n").as_bytes(),
    );
    let out = dir.join("results");
    let output = patchback(&fit_args(
        &inputs.reference,
        &inputs.notes,
        &out,
        &weights,
        &stats,
    ));
    let stderr = refusal(&output);
    assert!(
        stderr.starts_with("patchback: fit refused: patchback.fit.NoValidCandidate: ")
            && stderr.contains("output 0 is NaN"),
        "{stderr}"
    );
    assert!(!out.exists());
}

/// The CLI's documented order (module documentation): `--input`, `--notes`, `--weights`,
/// `--normalization`, then `<out>`. A missing reference wins over a missing artifact; a missing
/// artifact wins over an `<out>` that already holds `run.meta`; a missing artifact wins over a
/// missing statistics file.
#[test]
fn adversary_the_seed_files_are_answered_between_the_notes_and_out() {
    let dir = scratch("order");
    let inputs = inputs(&dir);
    let missing_weights = dir.join("missing.weights");
    let missing_stats = dir.join("missing.stats");
    let out = dir.join("results");

    let stderr = refusal(&patchback(&fit_args(
        &dir.join("missing.wav"),
        &inputs.notes,
        &out,
        &missing_weights,
        &missing_stats,
    )));
    assert!(
        stderr.starts_with("patchback: fit refused: patchback.fit.ClipNotFound: "),
        "{stderr}"
    );

    let stderr = refusal(&patchback(&fit_args(
        &inputs.reference,
        &inputs.notes,
        &out,
        &missing_weights,
        &missing_stats,
    )));
    assert!(
        stderr.starts_with("patchback: fit refused: patchback.model.UnknownWeightArtifact: "),
        "{stderr}"
    );

    fs::create_dir_all(&out).unwrap();
    write(&out, "run.meta", b"taken\n");
    let stderr = refusal(&patchback(&fit_args(
        &inputs.reference,
        &inputs.notes,
        &out,
        &missing_weights,
        &missing_stats,
    )));
    assert!(
        stderr.starts_with("patchback: fit refused: patchback.model.UnknownWeightArtifact: "),
        "{stderr}"
    );
    assert_eq!(fs::read(out.join("run.meta")).unwrap(), b"taken\n");
}

/// `MAX_STATS_BYTES` (1 MiB): a statistics file of 1 MiB + 1 byte is refused as
/// NormalizationStatsNotFound naming the bound; a statistics file with CRLF line ends is read,
/// and `run.meta`'s `normalization sha256` is the digest of its raw bytes, CRs included.
#[test]
fn adversary_an_oversize_statistics_file_is_refused_and_a_crlf_one_is_digested_raw() {
    let dir = scratch("statistics-bytes");
    let inputs = inputs(&dir);
    let weights = write(&dir, "regressor.weights", &targets_artifact());
    let out = dir.join("results");

    let mut oversize = stats_text(&inputs.features, "\n").into_bytes();
    oversize.resize((1 << 20) + 1, b' ');
    let oversize = write(&dir, "oversize.stats", &oversize);
    let stderr = refusal(&patchback(&fit_args(
        &inputs.reference,
        &inputs.notes,
        &out,
        &weights,
        &oversize,
    )));
    assert!(
        stderr
            .starts_with("patchback: fit refused: patchback.analysis.NormalizationStatsNotFound: ")
            && stderr.contains("larger than the 1048576 bytes"),
        "{stderr}"
    );
    assert!(!out.exists());

    let crlf = stats_text(&inputs.features, "\r\n");
    let crlf_path = write(&dir, "crlf.stats", crlf.as_bytes());
    let output = patchback(&fit_args(
        &inputs.reference,
        &inputs.notes,
        &out,
        &weights,
        &crlf_path,
    ));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let meta = fs::read_to_string(out.join("run.meta")).unwrap();
    let line = format!("normalization sha256 {}\n", hex(&sha256(crlf.as_bytes())));
    assert!(meta.contains(&line), "{meta}");
}
