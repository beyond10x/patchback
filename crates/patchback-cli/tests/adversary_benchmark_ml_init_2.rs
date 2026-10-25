//! Adversary cases for `patchback benchmark --kind ml-init` (story:ml-init-benchmark, wave a12,
//! pass 2). Each case drives the binary against a statement of `spec/domains/fit.yaml`, of the
//! verb's module documentation (`crates/patchback-cli/src/benchmark.rs`) or of a decision of
//! approval-record:wave-a12, on ground pass 1 did not take:
//!
//! - `StartBenchmark`'s outcome order on `--kind ml-init` without inputs, against `--examples
//!   0`, an unknown `--topology`, a bound of 0 and an existing `benchmark.txt` (Da12-2).
//! - The usage errors of Da12-2 and Da12-17, each by its own message, and that no input file is
//!   read before they pass.
//! - The records' grammar (Da12-5): the `modelspec` record is the artifact's, not the
//!   repository pin's; odd holdouts (3 and 5 clips) with the six medians recomputed from
//!   `clips.txt` and a clamp count that is neither 0 nor every clip.
//! - The listening examples (Da12-6) at 1, 2, 3 and 5 clips.
//! - A clip refusal mid-run: weights whose prediction is NaN for clip 2 only.
//! - A binding `--max-wall-time-ms`: both searches stop on the time bound.
//! - The statistics bound, a directory and an empty file as `--weights`; `--help`.
//!
//! Every artifact and statistics text is written here; the holdout's references are rendered
//! here by the documented stream (Da8-46) to choose the statistics.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_analysis::{
    FEATURES_1_VALUE_COUNT, FeatureContractDefinition, FeatureRegistry, NoteSpan,
    RELEASE_OFFSET_FRAMES, RELEASE_WINDOW_FRAMES, SAMPLE_RATE_HZ, analyze_and_extract, read_stats,
};
use patchback_audio::AudioClip;
use patchback_core::digest::{hex, sha256};
use patchback_core::parameter::Topology;
use patchback_core::patch::{compile, format_decimal, validate};
use patchback_core::score::NoteSequence;
use patchback_dsp::{RenderConfig, render};
use patchback_model::{ModelSpecPin, Predictor, W1_ELEMENTS, W1_SHAPE, W2_SHAPE};

const HIDDEN: usize = W1_SHAPE[1];
const OUTPUTS: usize = W2_SHAPE[1];
const N: usize = FEATURES_1_VALUE_COUNT;

/// Eight targets inside [0, 1] (the implementor's `TARGETS`).
const TARGETS: [f32; OUTPUTS] = [0.2, 0.3, 0.4, 0.6, 0.7, 0.8, 0.35, 0.65];

/// `HOLDOUT_DOMAIN` of the verb (Da8-46).
const HOLDOUT_DOMAIN: u64 = 0x686f_6c64_6f75_7431;

/// `MAX_STATS_BYTES` of `crates/patchback-cli/src/fit.rs` (1 MiB).
const MAX_STATS_BYTES: usize = 1 << 20;

/// The four stop reasons of `clips.txt` (module documentation, `clips.txt`).
const STOP_REASONS: [&str; 4] = [
    "RenderBudgetExhausted",
    "TimeBudgetExhausted",
    "RestartsExhausted",
    "ImprovementBelowThreshold",
];

/// The fourteen column names of an ML-init `clips.txt`, after `columns` (module documentation).
const ML_INIT_COLUMNS: [&str; 14] = [
    "index",
    "patch_seed",
    "fit_seed",
    "default_loss",
    "best_loss",
    "evaluations",
    "restarts",
    "stop_reason",
    "raw_loss",
    "seeded_best_loss",
    "seeded_evaluations",
    "seeded_restarts",
    "seeded_stop_reason",
    "clamped",
];

/// The five listening files of an ML-init example clip, in their recorded order (Da12-6).
const FIVE: [&str; 5] = [
    "reference.wav",
    "default.wav",
    "fit.wav",
    "predicted.wav",
    "seeded.wav",
];

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-adversary-ml-init-2")
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

fn s(path: &Path) -> &str {
    path.to_str().unwrap()
}

/// `patchback benchmark --out <out> <flags>`.
fn benchmark(out: &Path, flags: &[&str]) -> Output {
    let mut args = vec![
        "benchmark".to_owned(),
        "--out".to_owned(),
        s(out).to_owned(),
    ];
    args.extend(flags.iter().map(|f| (*f).to_owned()));
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(&args)
        .output()
        .expect("the patchback binary runs")
}

/// The bytes of a `weights/1` artifact with `w1` and `w2` and the `modelspec` line `version
/// commit` (the bytes of `website/docs/reference/files.md` § Exported weights, as the a10
/// writer lays them out).
fn artifact(w1: &[f32], w2: &[f32], version: &str, commit: &str) -> Vec<u8> {
    assert_eq!(w1.len(), W1_ELEMENTS);
    assert_eq!(w2.len(), HIDDEN * OUTPUTS);
    let payload: Vec<u8> = w1.iter().chain(w2).flat_map(|v| v.to_le_bytes()).collect();
    let header = format!(
        "weights/1\nmodel PatchRegressor\nmodelspec {version} {commit}\nfeatures features/1\nw1 32 64\nw2 64 8\nsha256 {}\n\n",
        hex(&sha256(&payload))
    );
    [header.as_bytes(), &payload].concat()
}

/// The artifact's payload: its last `32 × 64 + 64 × 8` float32 values.
fn payload(artifact: &[u8]) -> &[u8] {
    &artifact[artifact.len() - (W1_ELEMENTS + HIDDEN * OUTPUTS) * 4..]
}

/// Weights whose output `k` is `TARGETS[k]` times the first normalized feature, through hidden
/// unit 1 (hidden unit 0 is left for a case's own trigger).
fn target_weights() -> (Vec<f32>, Vec<f32>) {
    let mut w1 = vec![0.0f32; W1_ELEMENTS];
    let mut w2 = vec![0.0f32; HIDDEN * OUTPUTS];
    w1[1] = 1.0;
    for (k, target) in TARGETS.iter().enumerate() {
        w2[OUTPUTS + k] = *target;
    }
    (w1, w2)
}

fn pinned(w1: &[f32], w2: &[f32]) -> Vec<u8> {
    let pin = ModelSpecPin::repository().unwrap();
    artifact(w1, w2, pin.version(), pin.commit())
}

/// A `normalization.stats` text: every feature mean `-10^6`, deviation `10^6`, but those in
/// `custom`, `(index, mean, std)`.
fn stats(custom: &[(usize, f64, f64)]) -> String {
    let mut text = "patchback-normalization-stats 1\nfeature_contract features/1\nsplit train\nrows 16\nstd population\n".to_owned();
    for i in 0..N {
        let (mean, std) = custom
            .iter()
            .find(|(index, _, _)| *index == i)
            .map_or((-1e6, 1e6), |(_, mean, std)| (*mean, *std));
        text.push_str(&format!(
            "feature {i} {} {}\n",
            format_decimal(mean),
            format_decimal(std)
        ));
    }
    read_stats(&text).expect("the statistics read");
    text
}

/// The holdout's draws by the documented stream (Da8-46): `(patch_seed, fit_seed)`.
fn holdout_seeds(seed: u64, count: usize) -> Vec<(u64, u64)> {
    let pipeline = patchback_dataset::Pipeline::new();
    let mut stream = patchback_core::prng::SplitMix64::new(seed ^ HOLDOUT_DOMAIN);
    let mut seeds = Vec::new();
    while seeds.len() < count {
        let candidate = stream.next_u64();
        let draw = patchback_dataset::PatchDraw::from_seed(candidate);
        if let patchback_dataset::Rendering::Sample(_) = pipeline.run(&draw).unwrap() {
            seeds.push((candidate, stream.next_u64()));
        }
    }
    seeds
}

/// Clip `k`'s reference and notes as the verb renders them at the default block size.
fn reference(patch_seed: u64) -> (AudioClip, NoteSequence) {
    let draw = patchback_dataset::PatchDraw::from_seed(patch_seed);
    let topology = Topology::subtractive_mono();
    let notes = draw.notes();
    let validated = validate(&draw.patch(&topology)).unwrap();
    let rendered = render(&compile(&validated), &notes, &RenderConfig::default()).unwrap();
    (AudioClip::from_mono(rendered.samples).unwrap(), notes)
}

/// The raw `features/1` vector of the first note of `notes` in `clip`, as the seeded fit
/// computes it (`crates/patchback-fit/src/search.rs`, refusal step 6).
fn raw_features(clip: &AudioClip, notes: &NoteSequence) -> [f64; N] {
    let mut registry = FeatureRegistry::new();
    let id = registry
        .define(FeatureContractDefinition::features_1())
        .unwrap();
    registry.publish(id).unwrap();
    let note = notes.notes[0];
    let frames = clip.frame_count();
    let start = usize::try_from(note.start_sample).unwrap().min(frames);
    let end = usize::try_from(note.end_sample).unwrap().min(frames);
    let tail = (end + RELEASE_OFFSET_FRAMES + RELEASE_WINDOW_FRAMES).min(frames);
    let raw = analyze_and_extract(
        registry.get(id).unwrap(),
        &clip.samples()[start..tail],
        SAMPLE_RATE_HZ,
        NoteSpan {
            start_sample: 0,
            end_sample: end - start,
        },
    )
    .unwrap()
    .values;
    raw.as_slice().try_into().unwrap()
}

/// The raw features of the holdout's first `count` clips of `seed`.
fn holdout_features(seed: u64, count: usize) -> Vec<[f64; N]> {
    holdout_seeds(seed, count)
        .into_iter()
        .map(|(patch_seed, _)| {
            let (clip, notes) = reference(patch_seed);
            raw_features(&clip, &notes)
        })
        .collect()
}

/// The `(key, value)` records of a `key value` text, in order.
fn records(text: &str) -> Vec<(String, String)> {
    assert!(text.ends_with('\n'), "{text:?}");
    text.lines()
        .map(|line| {
            let (key, value) = line.split_once(' ').unwrap_or_else(|| panic!("{line:?}"));
            (key.to_owned(), value.to_owned())
        })
        .collect()
}

fn value<'a>(records: &'a [(String, String)], key: &str) -> &'a str {
    let found: Vec<&str> = records
        .iter()
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(found.len(), 1, "{key}: {records:?}");
    found[0]
}

/// Every file and directory under `dir`, relative to it, sorted.
fn listing(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, into: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            into.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
            if path.is_dir() {
                walk(root, &path, into);
            }
        }
    }
    let mut all = Vec::new();
    walk(dir, dir, &mut all);
    all.sort();
    all
}

/// Exit 1, nothing on stdout, exactly one stderr line, returned.
fn refused_line(output: &Output) -> String {
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 1, "{stderr}");
    lines[0].to_owned()
}

/// Exit 2, nothing on stdout, clap's `error: ` first; the whole of stderr.
fn usage_error(output: &Output) -> String {
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    assert!(stderr.starts_with("error: "), "{stderr}");
    stderr
}

/// One `clip` line of an ML-init `clips.txt`, its fifteen fields.
fn clip_lines(clips: &str) -> Vec<Vec<String>> {
    let mut lines = clips.lines();
    assert_eq!(lines.next(), Some("patchback-benchmark-clips 1"));
    assert_eq!(
        lines.next(),
        Some(format!("columns {}", ML_INIT_COLUMNS.join(" ")).as_str())
    );
    lines
        .enumerate()
        .map(|(k, line)| {
            let fields: Vec<String> = line.split(' ').map(str::to_owned).collect();
            assert_eq!(fields.len(), 15, "{line:?}");
            assert_eq!(fields[0], "clip");
            assert_eq!(fields[1], k.to_string());
            fields
        })
        .collect()
}

fn lower_median<T: Copy + PartialOrd>(values: &[T]) -> T {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sorted[(sorted.len() - 1) / 2]
}

/// `StartBenchmark` (`fit.yaml` 1384-1441) answers `invalid-holdout-size`, then
/// `weight-artifact-required`, then `unknown-topology`; the verb's step 2 (bounds) and step 3
/// (`<out>`) follow. So `--kind ml-init` without inputs is refused `InvalidHoldoutSize` at
/// `--examples 0` (even with an unknown topology), and `WeightArtifactRequired` ahead of an
/// unknown topology, a bound of 0 and an existing `benchmark.txt`, which is left untouched;
/// nothing else is written.
#[test]
fn ml_init_without_inputs_answers_start_benchmark_in_the_models_order() {
    let dir = scratch("order");
    let out = dir.join("bench");
    let ml = ["--kind", "ml-init"];
    let cases: [(&[&str], &str); 5] = [
        (&["--examples", "0"], "patchback.fit.InvalidHoldoutSize: "),
        (
            &["--examples", "0", "--topology", "no-such/1"],
            "patchback.fit.InvalidHoldoutSize: ",
        ),
        (
            &["--examples", "1", "--topology", "no-such/1"],
            "patchback.fit.WeightArtifactRequired: ",
        ),
        (
            &["--examples", "1", "--max-renders", "0"],
            "patchback.fit.WeightArtifactRequired: ",
        ),
        (
            &["--examples", "1", "--max-restarts", "0"],
            "patchback.fit.WeightArtifactRequired: ",
        ),
    ];
    for (flags, prefix) in cases {
        let all: Vec<&str> = ml.iter().chain(flags).copied().collect();
        let line = refused_line(&benchmark(&out, &all));
        assert!(
            line.starts_with(&format!("patchback: benchmark refused: {prefix}")),
            "{flags:?}: {line}"
        );
        assert!(!out.exists(), "{flags:?}: nothing written");
    }
    // The engine baseline's own order for the same unknown topology: TopologyNotFound.
    let line = refused_line(&benchmark(
        &out,
        &["--examples", "1", "--topology", "no-such/1"],
    ));
    assert!(
        line.starts_with("patchback: benchmark refused: patchback.fit.TopologyNotFound: "),
        "{line}"
    );
    // An existing benchmark.txt: the start refuses first; the file is untouched.
    fs::create_dir_all(&out).unwrap();
    let existing = write(&out, "benchmark.txt", b"someone else's\n");
    let line = refused_line(&benchmark(&out, &["--kind", "ml-init", "--examples", "1"]));
    assert!(
        line.starts_with("patchback: benchmark refused: patchback.fit.WeightArtifactRequired: "),
        "{line}"
    );
    assert!(line.contains("\"subtractive-mono/1\""), "{line}");
    assert_eq!(fs::read(&existing).unwrap(), b"someone else's\n");
    assert_eq!(listing(&out), ["benchmark.txt"]);
}

/// Da12-2 and Da12-17: each flag misuse is a usage error (2) with its own message, and no input
/// file is read before the usage checks pass (a missing or corrupt file named in a usage error
/// gives 2, not the input refusal's 1). With both files missing on `--kind ml-init`, the
/// artifact is reported, not the statistics.
#[test]
fn usage_errors_are_named_and_precede_every_read() {
    let dir = scratch("usage");
    let out = dir.join("bench");
    let missing_w = dir.join("absent.weights");
    let missing_s = dir.join("absent.stats");
    let corrupt = write(&dir, "corrupt.weights", b"weights/1\nnot an artifact\n");
    let (w, n, c) = (s(&missing_w), s(&missing_s), s(&corrupt));
    let conflict = |flag: &str| {
        format!(
            "error: the argument '{flag}' cannot be used with '--kind engine-baseline'; it belongs to '--kind ml-init'\n"
        )
    };
    for (flags, expected) in [
        (
            vec!["--weights", w, "--normalization", n],
            conflict("--weights <WEIGHTS>"),
        ),
        (
            vec![
                "--kind",
                "engine-baseline",
                "--weights",
                c,
                "--normalization",
                n,
            ],
            conflict("--weights <WEIGHTS>"),
        ),
    ] {
        let stderr = usage_error(&benchmark(
            &out,
            &[&["--examples", "1"][..], &flags].concat(),
        ));
        assert!(stderr.starts_with(&expected), "{flags:?}: {stderr}");
        assert!(stderr.contains("Usage: patchback benchmark"), "{stderr}");
        assert!(!out.exists());
    }
    for (flags, needed) in [
        (vec!["--weights", c], "--normalization <NORMALIZATION>"),
        (vec!["--normalization", n], "--weights <WEIGHTS>"),
        (
            vec!["--kind", "ml-init", "--weights", c],
            "--normalization <NORMALIZATION>",
        ),
        (
            vec!["--kind", "ml-init", "--normalization", n],
            "--weights <WEIGHTS>",
        ),
    ] {
        let stderr = usage_error(&benchmark(
            &out,
            &[&["--examples", "1"][..], &flags].concat(),
        ));
        assert!(
            stderr.starts_with("error: the following required arguments were not provided:\n"),
            "{flags:?}: {stderr}"
        );
        assert!(stderr.contains(needed), "{flags:?}: {stderr}");
        assert!(!out.exists());
    }
    let stderr = usage_error(&benchmark(&out, &["--examples", "1", "--kind", "foo"]));
    assert!(
        stderr.starts_with("error: invalid value 'foo' for '--kind <KIND>'"),
        "{stderr}"
    );
    assert!(
        stderr.contains("[possible values: engine-baseline, ml-init]"),
        "{stderr}"
    );
    assert!(!out.exists());
    // Both files missing on ml-init: the artifact first (step 0's order), exit 1.
    let line = refused_line(&benchmark(
        &out,
        &[
            "--examples",
            "1",
            "--kind",
            "ml-init",
            "--weights",
            w,
            "--normalization",
            n,
        ],
    ));
    assert_eq!(
        line,
        format!(
            "patchback: benchmark refused: patchback.model.UnknownWeightArtifact: no weight artifact at {missing_w:?}"
        )
    );
    assert!(!out.exists());
}

/// Da12-5: `modelspec <version> <commit>` is the release the artifact names, not the
/// repository's pin; `weights sha256` is the payload's SHA-256 (64 lower-case hex),
/// `normalization sha256` the statistics bytes', `feature_contract features/1`; the records in
/// the documented order; `clamped_predictions 0 of 1`; one example directory of five files and
/// five `listening_example` lines (Da12-6).
#[test]
fn the_modelspec_record_is_the_artifacts_and_a_holdout_of_one_has_five_examples() {
    let dir = scratch("modelspec");
    let pin = ModelSpecPin::repository().unwrap();
    let (version, commit) = ("9.8.7", "0123456789abcdef0123456789abcdef01234567");
    assert_ne!(pin.version(), version);
    assert_ne!(pin.commit(), commit);
    let (w1, w2) = target_weights();
    let bytes = artifact(&w1, &w2, version, commit);
    let weights = write(&dir, "other.weights", &bytes);
    let stats_text = stats(&[]);
    let stats_path = write(&dir, "normalization.stats", stats_text.as_bytes());
    let out = dir.join("bench");
    let output = benchmark(
        &out,
        &[
            "--examples",
            "1",
            "--seed",
            "3",
            "--max-renders",
            "2",
            "--max-restarts",
            "1",
            "--kind",
            "ml-init",
            "--weights",
            s(&weights),
            "--normalization",
            s(&stats_path),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let file = fs::read_to_string(out.join("benchmark.txt")).unwrap();
    let recs = records(&file);
    let keys: Vec<&str> = recs.iter().map(|(k, _)| k.as_str()).collect();
    let mut expected_keys = vec![
        "patchback-benchmark",
        "patchback",
        "kind",
        "topology",
        "seed",
        "example_count",
        "sample_rate",
        "block_size",
        "max_renders",
        "max_wall_time_ms",
        "max_restarts",
        "improvement_threshold",
        "prng",
        "weights",
        "modelspec",
        "normalization",
        "feature_contract",
        "default_median_loss",
        "default_search_median_loss",
        "raw_regressor_median_loss",
        "regressor_search_median_loss",
        "default_search_median_evaluations",
        "regressor_search_median_evaluations",
        "clamped_predictions",
        "wall_time_ms",
    ];
    expected_keys.extend(["listening_example"; 5]);
    assert_eq!(keys, expected_keys);
    assert_eq!(value(&recs, "modelspec"), format!("{version} {commit}"));
    let digest = value(&recs, "weights").strip_prefix("sha256 ").unwrap();
    assert_eq!(digest.len(), 64);
    assert!(
        digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    assert_eq!(digest, hex(&sha256(payload(&bytes))));
    assert_eq!(
        value(&recs, "normalization"),
        format!("sha256 {}", hex(&sha256(stats_text.as_bytes())))
    );
    assert_eq!(value(&recs, "feature_contract"), "features/1");
    assert_eq!(value(&recs, "clamped_predictions"), "0 of 1");
    let lines: Vec<&str> = recs
        .iter()
        .filter(|(k, _)| k == "listening_example")
        .map(|(_, v)| v.as_str())
        .collect();
    let five: Vec<String> = FIVE.iter().map(|f| format!("examples/0/{f}")).collect();
    assert_eq!(lines, five);
    let mut expected_listing = vec![
        "benchmark.txt".to_owned(),
        "clips.txt".to_owned(),
        "examples".to_owned(),
        "examples/0".to_owned(),
    ];
    expected_listing.extend(five);
    expected_listing.sort();
    assert_eq!(listing(&out), expected_listing);
}

/// Statistics on feature `j` that put every clip at or above 0 and the clips whose raw feature
/// is above `cut` above 1: output 0 reads feature `j` alone, so it clamps on exactly those clips.
fn clamp_some(features: &[[f64; N]], cut_rank: usize) -> Option<(usize, f64, f64)> {
    (0..N).find_map(|j| {
        let mut values: Vec<f64> = features.iter().map(|f| f[j]).collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let (low, below, above) = (values[0], values[cut_rank - 1], values[cut_rank]);
        (below < above && low < below && below.is_finite() && above.is_finite()).then(|| {
            let cut = below + (above - below) / 2.0;
            (j, low, cut - low)
        })
    })
}

/// Da12-5 at odd clip counts (3 and 5) on weights whose output 0 clamps on some clips and not on
/// others: the `columns` line exact, every `clip` line fifteen fields, `clamped` an integer
/// 0 to 8 equal to the predictor's own count for the clip, both stop reasons one of the four;
/// the six median records recomputed from `clips.txt` (the lower median of an odd count is the
/// middle value); `clamped_predictions <n> of <clips>` with `0 < n < clips`; three example
/// directories of five files, fifteen `listening_example` lines.
#[test]
fn odd_holdouts_record_every_median_and_a_partial_clamp_count() {
    for (count, cut_rank) in [(3_usize, 2_usize), (5, 3)] {
        let dir = scratch(&format!("odd-{count}"));
        let features = holdout_features(5, count);
        let (j, mean, std) = clamp_some(&features, cut_rank).expect("a feature that splits");
        let (mut w1, mut w2) = target_weights();
        w1[j * HIDDEN] = 1.0;
        w2[0] = 1.0;
        w2[OUTPUTS] = 0.0;
        let bytes = pinned(&w1, &w2);
        let weights = write(&dir, "split.weights", &bytes);
        let stats_text = stats(&[(j, mean, std)]);
        let stats_path = write(&dir, "normalization.stats", stats_text.as_bytes());
        let mut predictor = Predictor::load(&weights).unwrap();
        let table = read_stats(&stats_text).unwrap();
        let expected_clamped: Vec<usize> = features
            .iter()
            .map(|raw| {
                predictor
                    .predict("features/1", &table.normalize(raw))
                    .unwrap()
                    .clamped
            })
            .collect();
        let clamped_clips = expected_clamped.iter().filter(|c| **c > 0).count();
        assert_eq!(clamped_clips, count - cut_rank, "{expected_clamped:?}");
        assert!(clamped_clips > 0 && clamped_clips < count);

        let out = dir.join("bench");
        let examples = count.to_string();
        let output = benchmark(
            &out,
            &[
                "--examples",
                &examples,
                "--seed",
                "5",
                "--max-renders",
                "2",
                "--max-restarts",
                "1",
                "--kind",
                "ml-init",
                "--weights",
                s(&weights),
                "--normalization",
                s(&stats_path),
            ],
        );
        assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
        let clips = clip_lines(&fs::read_to_string(out.join("clips.txt")).unwrap());
        assert_eq!(clips.len(), count);
        for (k, fields) in clips.iter().enumerate() {
            assert!(STOP_REASONS.contains(&fields[8].as_str()), "{fields:?}");
            assert!(STOP_REASONS.contains(&fields[13].as_str()), "{fields:?}");
            let clamped: usize = fields[14].parse().expect("an integer");
            assert!(clamped <= 8, "{fields:?}");
            assert_eq!(clamped, expected_clamped[k], "clip {k}: {fields:?}");
        }
        let real = |i: usize| -> String {
            lower_median(
                &clips
                    .iter()
                    .map(|f| f[i].parse::<f64>().unwrap())
                    .collect::<Vec<_>>(),
            )
            .to_string()
        };
        let count_of = |i: usize| -> String {
            lower_median(
                &clips
                    .iter()
                    .map(|f| f[i].parse::<i64>().unwrap())
                    .collect::<Vec<_>>(),
            )
            .to_string()
        };
        let file = fs::read_to_string(out.join("benchmark.txt")).unwrap();
        let recs = records(&file);
        for (key, expected) in [
            ("default_median_loss", real(4)),
            ("default_search_median_loss", real(5)),
            ("raw_regressor_median_loss", real(9)),
            ("regressor_search_median_loss", real(10)),
            ("default_search_median_evaluations", count_of(6)),
            ("regressor_search_median_evaluations", count_of(11)),
        ] {
            assert_eq!(value(&recs, key), expected, "{count} clips: {key}");
        }
        assert_eq!(
            value(&recs, "clamped_predictions"),
            format!("{clamped_clips} of {count}")
        );
        let lines: Vec<String> = recs
            .iter()
            .filter(|(k, _)| k == "listening_example")
            .map(|(_, v)| v.clone())
            .collect();
        let expected_lines: Vec<String> = (0..3)
            .flat_map(|k| FIVE.iter().map(move |f| format!("examples/{k}/{f}")))
            .collect();
        assert_eq!(lines, expected_lines, "{count} clips");
        let dirs: Vec<String> = listing(&out.join("examples"))
            .into_iter()
            .filter(|p| !p.contains('/'))
            .collect();
        assert_eq!(dirs, ["0", "1", "2"], "{count} clips");
        for k in 0..3 {
            let mut files = listing(&out.join("examples").join(k.to_string()));
            files.sort();
            let mut five: Vec<String> = FIVE.iter().map(|f| (*f).to_owned()).collect();
            five.sort();
            assert_eq!(files, five);
        }
        assert_eq!(
            text(&output.stdout),
            file[file.find("default_median_loss ").unwrap()..]
        );
    }
}

/// A clip refusal mid-run (module documentation, step 5; Da8-3): weights whose prediction is
/// NaN for clip 2 of 3 only (hidden unit 0 overflows to +inf on that clip's feature `j`, and
/// `inf × 0` is NaN: `PredictError::NonFinitePrediction`, a `PredictionFailed` named
/// `NoValidCandidate`). The same weights complete a holdout of 2 (the same first two clips, ten
/// `listening_example` lines); on 3 the benchmark is refused `clip 2: …`, exit 1, one line, and
/// no `--out` directory is left.
#[test]
fn a_prediction_refused_at_clip_2_of_3_refuses_the_benchmark_and_writes_nothing() {
    let dir = scratch("clip-2");
    let features = holdout_features(9, 3);
    // Feature j where clip 2 lies outside clips 0 and 1; the side decides the weight's sign.
    let (j, sign, mean, std) = (1..N)
        .find_map(|j| {
            let (a, b, c) = (features[0][j], features[1][j], features[2][j]);
            let (low, high) = (a.min(b), a.max(b));
            if c > high {
                Some((j, 1.0f32, high, (c - high) / 2.0))
            } else if c < low {
                Some((j, -1.0f32, low, (low - c) / 2.0))
            } else {
                None
            }
        })
        .expect("a feature on which clip 2 lies outside clips 0 and 1");
    let (mut w1, w2) = target_weights();
    w1[j * HIDDEN] = sign * 3.0e38;
    let bytes = pinned(&w1, &w2);
    let weights = write(&dir, "trigger.weights", &bytes);
    let stats_text = stats(&[(j, mean, std)]);
    let stats_path = write(&dir, "normalization.stats", stats_text.as_bytes());
    // The construction: clip 2's prediction is refused, clips 0 and 1 are not.
    let mut predictor = Predictor::load(&weights).unwrap();
    let table = read_stats(&stats_text).unwrap();
    for (k, raw) in features.iter().enumerate() {
        let predicted = predictor.predict("features/1", &table.normalize(raw));
        assert_eq!(predicted.is_err(), k == 2, "clip {k}: {predicted:?}");
    }
    let flags = |examples: &'static str| -> Vec<String> {
        [
            "--examples",
            examples,
            "--seed",
            "9",
            "--max-renders",
            "2",
            "--max-restarts",
            "1",
            "--kind",
            "ml-init",
            "--weights",
            s(&weights),
            "--normalization",
            s(&stats_path),
        ]
        .iter()
        .map(|f| (*f).to_owned())
        .collect()
    };
    let two = dir.join("two");
    let f2 = flags("2");
    let output = benchmark(&two, &f2.iter().map(String::as_str).collect::<Vec<_>>());
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let file = fs::read_to_string(two.join("benchmark.txt")).unwrap();
    assert_eq!(
        records(&file)
            .iter()
            .filter(|(k, _)| k == "listening_example")
            .count(),
        10
    );
    let three = dir.join("three");
    let f3 = flags("3");
    let line = refused_line(&benchmark(
        &three,
        &f3.iter().map(String::as_str).collect::<Vec<_>>(),
    ));
    assert!(
        line.starts_with("patchback: benchmark refused: clip 2: patchback.fit.NoValidCandidate: "),
        "{line}"
    );
    assert!(line.contains("is NaN"), "{line}");
    assert!(!three.exists(), "a refused benchmark leaves no --out");
    eprintln!("feature {j}: {line}");
}

/// A binding `--max-wall-time-ms` (1 ms): both searches stop on the time bound after their first
/// candidate (`search.rs`, Stopping), so on every clip `stop_reason` and `seeded_stop_reason`
/// are `TimeBudgetExhausted`, each search counted 1 evaluation and 1 restart, the search's best
/// loss is the default arm's loss and the seeded search's best loss the raw arm's, bit for bit.
#[test]
fn a_binding_wall_time_bound_stops_both_searches_on_their_first_candidate() {
    let dir = scratch("wall");
    let (w1, w2) = target_weights();
    let weights = write(&dir, "targets.weights", &pinned(&w1, &w2));
    let stats_path = write(&dir, "normalization.stats", stats(&[]).as_bytes());
    let out = dir.join("bench");
    let output = benchmark(
        &out,
        &[
            "--examples",
            "2",
            "--seed",
            "7",
            "--max-renders",
            "50",
            "--max-restarts",
            "3",
            "--max-wall-time-ms",
            "1",
            "--kind",
            "ml-init",
            "--weights",
            s(&weights),
            "--normalization",
            s(&stats_path),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let clips = clip_lines(&fs::read_to_string(out.join("clips.txt")).unwrap());
    for fields in &clips {
        assert_eq!(fields[8], "TimeBudgetExhausted", "{fields:?}");
        assert_eq!(fields[13], "TimeBudgetExhausted", "{fields:?}");
        assert_eq!(
            (fields[6].as_str(), fields[7].as_str()),
            ("1", "1"),
            "{fields:?}"
        );
        assert_eq!(
            (fields[11].as_str(), fields[12].as_str()),
            ("1", "1"),
            "{fields:?}"
        );
        assert_eq!(fields[5], fields[4], "best = default: {fields:?}");
        assert_eq!(fields[10], fields[9], "seeded best = raw: {fields:?}");
    }
    let file = fs::read_to_string(out.join("benchmark.txt")).unwrap();
    assert!(file.contains("\nmax_wall_time_ms 1\n"), "{file}");
}

/// The input files' bounds and kinds (Da12-3, step 0): a `--normalization` one byte longer than
/// `MAX_STATS_BYTES` is refused `NormalizationStatsNotFound` for its size; one of exactly
/// `MAX_STATS_BYTES` is read and refused by `read_stats` instead; a directory as `--weights` is
/// `UnknownWeightArtifact`, an empty file `CorruptWeightArtifact`. Exit 1, nothing written.
#[test]
fn the_input_files_bounds_and_kinds_are_refused_as_documented() {
    let dir = scratch("inputs");
    let (w1, w2) = target_weights();
    let weights = write(&dir, "targets.weights", &pinned(&w1, &w2));
    let stats_path = write(&dir, "normalization.stats", stats(&[]).as_bytes());
    let over = write(&dir, "over.stats", &vec![b'#'; MAX_STATS_BYTES + 1]);
    let at = write(&dir, "at.stats", &vec![b'#'; MAX_STATS_BYTES]);
    let empty = write(&dir, "empty.weights", b"");
    let directory = dir.join("a-directory");
    fs::create_dir_all(&directory).unwrap();
    let out = dir.join("bench");
    let run = |w: &Path, n: &Path| {
        refused_line(&benchmark(
            &out,
            &[
                "--examples",
                "1",
                "--kind",
                "ml-init",
                "--weights",
                s(w),
                "--normalization",
                s(n),
            ],
        ))
    };
    let line = run(&weights, &over);
    assert_eq!(
        line,
        format!(
            "patchback: benchmark refused: patchback.analysis.NormalizationStatsNotFound: {over:?}: the file is larger than the 1048576 bytes a benchmark reads"
        )
    );
    let line = run(&weights, &at);
    assert!(
        line.starts_with(&format!(
            "patchback: benchmark refused: patchback.analysis.NormalizationStatsNotFound: {at:?}: "
        )),
        "{line}"
    );
    assert!(!line.contains("larger than"), "{line}");
    let line = run(&directory, &stats_path);
    assert!(
        line.starts_with(&format!(
            "patchback: benchmark refused: patchback.model.UnknownWeightArtifact: no weight artifact at {directory:?}"
        )),
        "{line}"
    );
    let line = run(&empty, &stats_path);
    assert!(
        line.starts_with(&format!(
            "patchback: benchmark refused: patchback.model.CorruptWeightArtifact: {empty:?}: "
        )),
        "{line}"
    );
    assert!(!out.exists());
}

/// `patchback benchmark --help`: exit 0 on stdout, naming `--kind` with its two values,
/// `--weights` and `--normalization`.
#[test]
fn the_help_names_the_kind_and_the_input_flags() {
    let output = Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(["benchmark", "--help"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let help = text(&output.stdout);
    for needle in [
        "--kind <KIND>",
        "engine-baseline",
        "ml-init",
        "--weights <WEIGHTS>",
        "--normalization <NORMALIZATION>",
    ] {
        assert!(help.contains(needle), "{needle}: {help}");
    }
}
