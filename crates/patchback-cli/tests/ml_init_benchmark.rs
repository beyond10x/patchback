//! `patchback benchmark --kind ml-init --weights <path> --normalization <path>` through the binary
//! (story:ml-init-benchmark, Da12-2 to Da12-7): a small ML-init benchmark over a synthesized
//! holdout completes with exit 0 and writes `benchmark.txt` byte for byte as derived here from
//! the per-clip records, the inputs and the pin; `clips.txt` carries the four arms per clip, the
//! first two the engine baseline's own; five listening files per example clip compare back to
//! the recorded losses; the clamp record counts the clips whose prediction clamped, derived here
//! from the library's own predictor; the kind flag's refusals exit 1 or 2 with nothing written;
//! the engine baseline is unchanged by the flag. The model is held by
//! `crates/patchback-fit/tests/ml_init_benchmark.rs`; the baseline's own files by
//! `tests/benchmark.rs`, unchanged.
//!
//! Every artifact is written here from weights the test chooses (the `weights/1` bytes of
//! `website/docs/reference/files.md` § Exported weights, the a10 writer copied from
//! `tests/fit_seeded.rs`); every statistics file is a `normalization.stats` text written here.
//! The statistics are wide (every mean `-10^6`, every standard deviation `10^6`), so each clip's
//! first normalized feature is about 1 and the prediction about the chosen targets whatever the
//! clip; the clamp count is not assumed from that but recomputed from the clip's features.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

use patchback_analysis::{
    FEATURES_1_VALUE_COUNT, FeatureContractDefinition, FeatureRegistry, NoteSpan,
    RELEASE_OFFSET_FRAMES, RELEASE_WINDOW_FRAMES, SAMPLE_RATE_HZ, analyze_and_extract, read_stats,
};
use patchback_core::digest::{hex, sha256};
use patchback_core::patch::format_decimal;
use patchback_core::score::NoteSequence;
use patchback_model::{ModelSpecPin, Predictor, W1_ELEMENTS, W1_SHAPE, W2_SHAPE};

const HIDDEN: usize = W1_SHAPE[1];
const OUTPUTS: usize = W2_SHAPE[1];

/// Eight targets inside [0, 1], none at the default 0.5.
const TARGETS: [f32; OUTPUTS] = [0.2, 0.3, 0.4, 0.6, 0.7, 0.8, 0.35, 0.65];
/// Targets three of which fall outside [0, 1]: two above, one below.
const CLAMPED_TARGETS: [f32; OUTPUTS] = [1.5, -0.5, 2.0, 0.5, 0.25, 0.75, 0.4, 0.6];

/// The holdout stream's domain, `HOLDOUT_DOMAIN` of the verb (Da8-46).
const HOLDOUT_DOMAIN: u64 = 0x686f_6c64_6f75_7431;

/// The small run's flags after `--out <out>`, as `tests/benchmark.rs` runs the baseline.
const SMALL: &[&str] = &[
    "--examples",
    "2",
    "--seed",
    "7",
    "--max-renders",
    "12",
    "--max-restarts",
    "1",
];

/// Two renders a fit: for runs whose search quality does not matter.
const TWO_RENDERS: &[&str] = &[
    "--examples",
    "2",
    "--seed",
    "7",
    "--max-renders",
    "2",
    "--max-restarts",
    "1",
];

/// A fresh, empty directory for one test under Cargo's per-target scratch directory.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-ml-init-benchmark")
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

fn patchback(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(args)
        .output()
        .expect("the patchback binary runs")
}

/// `patchback benchmark --out <out> <flags> <extra>`.
fn benchmark(out: &Path, flags: &[&str], extra: &[&str]) -> Output {
    let mut args = vec![
        "benchmark".to_owned(),
        "--out".to_owned(),
        out.to_str().unwrap().to_owned(),
    ];
    args.extend(flags.iter().chain(extra).map(|s| (*s).to_owned()));
    patchback(&args)
}

/// The ML-init flags for `weights` and `stats`.
fn ml_init<'a>(weights: &'a Path, stats: &'a Path) -> [&'a str; 6] {
    [
        "--kind",
        "ml-init",
        "--weights",
        weights.to_str().unwrap(),
        "--normalization",
        stats.to_str().unwrap(),
    ]
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

/// Exit 1, nothing on stdout, one stderr line `patchback: benchmark refused: <error>: …` naming
/// `error`, which `spec/domains/<domain>.yaml` declares, and nothing at `out`.
fn assert_refused(output: &Output, domain: &str, error: &str, out: &Path) -> String {
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 1, "{stderr}");
    assert!(
        lines[0].starts_with(&format!("patchback: benchmark refused: {error}: ")),
        "{stderr}"
    );
    assert!(
        declared_errors(domain)
            .iter()
            .any(|declared| declared == error),
        "{error} is not an error of spec/domains/{domain}.yaml"
    );
    assert!(!out.exists(), "a refused benchmark writes nothing");
    lines[0].to_owned()
}

/// A usage error: exit 2, nothing on stdout, clap's `error:` on stderr, nothing at `out`.
fn assert_usage_error(output: &Output, out: &Path) -> String {
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    assert!(stderr.starts_with("error: "), "{stderr}");
    assert!(!out.exists(), "a usage error writes nothing");
    stderr
}

/// The bytes of a `weights/1` artifact whose output `k` is `targets[k]` times the first
/// normalized feature, for `features`, under the repository's ModelSpec pin (the a10 writer,
/// copied from `tests/fit_seeded.rs`).
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

/// The payload of a `weights/1` artifact: its last `32 × 64 + 64 × 8` float32 values.
fn payload(artifact: &[u8]) -> &[u8] {
    &artifact[artifact.len() - (W1_ELEMENTS + HIDDEN * OUTPUTS) * 4..]
}

/// A `normalization.stats` text of `version`: every mean `-10^6`, every deviation `10^6`.
fn wide_stats(version: &str) -> String {
    let mut text = format!(
        "patchback-normalization-stats 1\nfeature_contract {version}\nsplit train\nrows 16\nstd population\n"
    );
    for i in 0..FEATURES_1_VALUE_COUNT {
        text.push_str(&format!(
            "feature {i} {} {}\n",
            format_decimal(-1e6),
            format_decimal(1e6)
        ));
    }
    text
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

/// The columns line of an ML-init `clips.txt`: the engine baseline's eight, then the raw
/// regressor arm's loss and the seeded fit's best loss, evaluations, restarts, stop reason and
/// clamp count.
const ML_INIT_COLUMNS: &str = "columns index patch_seed fit_seed default_loss best_loss evaluations \
     restarts stop_reason raw_loss seeded_best_loss seeded_evaluations seeded_restarts \
     seeded_stop_reason clamped";

/// One `clip` line of an ML-init `clips.txt`.
#[derive(Debug)]
struct Clip {
    /// The engine baseline's eight fields, as its `clips.txt` writes them.
    baseline: String,
    patch_seed: u64,
    fit_seed: u64,
    default_loss: f64,
    best_loss: f64,
    evaluations: i64,
    raw_loss: f64,
    seeded_best_loss: f64,
    seeded_evaluations: i64,
    seeded_restarts: u32,
    seeded_stop_reason: String,
    clamped: usize,
}

fn clips(text: &str) -> Vec<Clip> {
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("patchback-benchmark-clips 1"));
    assert_eq!(lines.next(), Some(ML_INIT_COLUMNS));
    assert!(text.ends_with('\n'));
    lines
        .enumerate()
        .map(|(k, line)| {
            let fields: Vec<&str> = line.split(' ').collect();
            assert_eq!(fields.len(), 15, "{line:?}");
            assert_eq!(fields[0], "clip", "{line:?}");
            assert_eq!(fields[1], k.to_string(), "{line:?}");
            Clip {
                baseline: fields[..9].join(" "),
                patch_seed: fields[2].parse().unwrap(),
                fit_seed: fields[3].parse().unwrap(),
                default_loss: fields[4].parse().unwrap(),
                best_loss: fields[5].parse().unwrap(),
                evaluations: fields[6].parse().unwrap(),
                raw_loss: fields[9].parse().unwrap(),
                seeded_best_loss: fields[10].parse().unwrap(),
                seeded_evaluations: fields[11].parse().unwrap(),
                seeded_restarts: fields[12].parse().unwrap(),
                seeded_stop_reason: fields[13].to_owned(),
                clamped: fields[14].parse().unwrap(),
            }
        })
        .collect()
}

/// The lower median (Da8-4), recomputed here.
fn lower_median<T: Copy + PartialOrd>(values: &[T]) -> T {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sorted[(sorted.len() - 1) / 2]
}

/// The draw of patch seed `patch_seed`'s notes: one note from sample 0
/// (`patchback_dataset::PatchDraw`).
fn notes_of(patch_seed: u64) -> NoteSequence {
    patchback_dataset::PatchDraw::from_seed(patch_seed).notes()
}

/// How many of the eight outputs the predictor of `weights` clamps for the first note of
/// `notes` in `reference`, under `stats`: the note's own clip analysed as the seeded fit
/// analyses it (`crates/patchback-fit/src/search.rs`, step 6), normalized, predicted.
fn clamped_outputs(reference: &Path, notes: &NoteSequence, weights: &Path, stats: &str) -> usize {
    let clip = patchback_audio::decode(&fs::read(reference).unwrap()).unwrap();
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
    let raw: [f64; FEATURES_1_VALUE_COUNT] = raw.as_slice().try_into().unwrap();
    let normalized = read_stats(stats).unwrap().normalize(&raw);
    let prediction = Predictor::load(weights)
        .unwrap()
        .predict("features/1", &normalized)
        .unwrap();
    prediction.clamped
}

/// The holdout's seeds by the documented stream (Da8-46), as `tests/benchmark.rs` walks it.
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

/// `patchback compare`'s `loss` of `candidate` against `reference`.
fn compared_loss(reference: &Path, candidate: &Path) -> String {
    let output = patchback(&[
        "compare".to_owned(),
        "--reference".to_owned(),
        reference.to_str().unwrap().to_owned(),
        "--candidate".to_owned(),
        candidate.to_str().unwrap().to_owned(),
    ]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    value(&records(&text(&output.stdout)), "loss").to_owned()
}

/// The small ML-init run: exit 0, nothing on stderr; `benchmark.txt` byte for byte as derived
/// from `clips.txt`, the artifact's and the statistics' bytes and the ModelSpec pin, its one
/// varying record `wall_time_ms` read back and bounded by the run's own time; stdout is its
/// records from `default_median_loss` on; `clips.txt`'s first eight fields are the engine
/// baseline's of the same flags (arms 1 and 2 unchanged, Da12-4), its seeds the documented
/// stream; five listening files per clip, `predicted.wav` and `seeded.wav` comparing back to the
/// clip's raw loss and seeded best loss; no clip clamped.
#[test]
fn an_ml_init_benchmark_reports_the_four_arms_and_its_files_agree() {
    let dir = scratch("small");
    let artifact = artifact_bytes(&TARGETS, "features/1");
    let weights = write(&dir, "regressor.weights", &artifact);
    let stats = wide_stats("features/1");
    let stats_path = write(&dir, "normalization.stats", stats.as_bytes());
    let out = dir.join("bench");
    let started = Instant::now();
    let output = benchmark(&out, SMALL, &ml_init(&weights, &stats_path));
    let elapsed = started.elapsed();
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    eprintln!("benchmark --kind ml-init --examples 2: {elapsed:?}");

    let clips = clips(&fs::read_to_string(out.join("clips.txt")).unwrap());
    assert_eq!(clips.len(), 2);
    for ((patch_seed, fit_seed), clip) in holdout_seeds(7, 2).into_iter().zip(&clips) {
        assert_eq!((clip.patch_seed, clip.fit_seed), (patch_seed, fit_seed));
    }
    for (k, clip) in clips.iter().enumerate() {
        assert!(clip.evaluations >= 1 && clip.evaluations <= 12, "{clip:?}");
        assert!(
            clip.seeded_evaluations >= 1 && clip.seeded_evaluations <= 12,
            "{clip:?}"
        );
        assert_eq!(clip.seeded_restarts, 1, "{clip:?}");
        assert!(
            ["RenderBudgetExhausted", "RestartsExhausted"]
                .contains(&clip.seeded_stop_reason.as_str()),
            "{clip:?}"
        );
        // Restart 0 starts at the prediction and keeps only improvements.
        assert!(clip.seeded_best_loss <= clip.raw_loss, "{clip:?}");
        assert_eq!(clip.clamped, 0, "{clip:?}");
        let example = out.join("examples").join(k.to_string());
        let reference = example.join("reference.wav");
        assert_eq!(
            clamped_outputs(&reference, &notes_of(clip.patch_seed), &weights, &stats),
            clip.clamped
        );
        for (file, loss) in [
            ("default.wav", clip.default_loss),
            ("fit.wav", clip.best_loss),
            ("predicted.wav", clip.raw_loss),
            ("seeded.wav", clip.seeded_best_loss),
        ] {
            assert_eq!(
                compared_loss(&reference, &example.join(file)),
                loss.to_string(),
                "clip {k} {file}"
            );
        }
    }

    // Arms 1 and 2 are the engine baseline's, clip for clip.
    let baseline_out = dir.join("baseline");
    let baseline = benchmark(&baseline_out, SMALL, &[]);
    assert_eq!(
        baseline.status.code(),
        Some(0),
        "{}",
        text(&baseline.stderr)
    );
    let baseline_clips = fs::read_to_string(baseline_out.join("clips.txt")).unwrap();
    let baseline_lines: Vec<&str> = baseline_clips.lines().skip(2).collect();
    assert_eq!(
        baseline_lines,
        clips
            .iter()
            .map(|c| c.baseline.as_str())
            .collect::<Vec<_>>()
    );

    let mut examples = Vec::new();
    for k in 0..2 {
        for name in [
            "reference.wav",
            "default.wav",
            "fit.wav",
            "predicted.wav",
            "seeded.wav",
        ] {
            examples.push(format!("examples/{k}/{name}"));
        }
    }
    let mut expected_listing = vec![
        "benchmark.txt".to_owned(),
        "clips.txt".to_owned(),
        "examples".to_owned(),
        "examples/0".to_owned(),
        "examples/1".to_owned(),
    ];
    expected_listing.extend(examples.iter().cloned());
    expected_listing.sort();
    assert_eq!(listing(&out), expected_listing);

    let file = fs::read_to_string(out.join("benchmark.txt")).unwrap();
    let wall: u64 = value(&records(&file), "wall_time_ms").parse().unwrap();
    assert!(u128::from(wall) <= elapsed.as_millis(), "{wall} ms");
    let median = |f: fn(&Clip) -> f64| lower_median(&clips.iter().map(f).collect::<Vec<_>>());
    let median_count = |f: fn(&Clip) -> i64| lower_median(&clips.iter().map(f).collect::<Vec<_>>());
    let pin = ModelSpecPin::repository().unwrap();
    let mut expected = format!(
        "patchback-benchmark 1\npatchback {}\nkind ml-init\ntopology subtractive-mono/1\nseed 7\n\
         example_count 2\nsample_rate 48000\nblock_size 256\nmax_renders 12\n\
         max_wall_time_ms 60000\nmax_restarts 1\nimprovement_threshold 0.0001\n\
         prng xoshiro256**\nweights sha256 {}\nmodelspec {} {}\nnormalization sha256 {}\n\
         feature_contract features/1\n",
        env!("CARGO_PKG_VERSION"),
        hex(&sha256(payload(&artifact))),
        pin.version(),
        pin.commit(),
        hex(&sha256(stats.as_bytes())),
    );
    let result = format!(
        "default_median_loss {}\ndefault_search_median_loss {}\nraw_regressor_median_loss {}\n\
         regressor_search_median_loss {}\ndefault_search_median_evaluations {}\n\
         regressor_search_median_evaluations {}\nclamped_predictions {} of 2\nwall_time_ms {wall}\n{}",
        median(|c| c.default_loss),
        median(|c| c.best_loss),
        median(|c| c.raw_loss),
        median(|c| c.seeded_best_loss),
        median_count(|c| c.evaluations),
        median_count(|c| c.seeded_evaluations),
        clips.iter().filter(|c| c.clamped > 0).count(),
        examples
            .iter()
            .map(|path| format!("listening_example {path}\n"))
            .collect::<String>(),
    );
    expected.push_str(&result);
    assert_eq!(file, expected);
    assert_eq!(text(&output.stdout), result);
    eprintln!("{file}");
}

/// Weights whose prediction clamps three outputs on every clip: each clip's `clamped` column is
/// the predictor's own count for that clip, recomputed here from its features, and
/// `clamped_predictions` the clips with a count above 0.
#[test]
fn clamped_predictions_count_the_clips_whose_prediction_clamped() {
    let dir = scratch("clamped");
    let weights = write(
        &dir,
        "clamped.weights",
        &artifact_bytes(&CLAMPED_TARGETS, "features/1"),
    );
    let stats = wide_stats("features/1");
    let stats_path = write(&dir, "normalization.stats", stats.as_bytes());
    let out = dir.join("bench");
    let output = benchmark(&out, TWO_RENDERS, &ml_init(&weights, &stats_path));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let clips = clips(&fs::read_to_string(out.join("clips.txt")).unwrap());
    assert_eq!(clips.len(), 2);
    for (k, clip) in clips.iter().enumerate() {
        let reference = out
            .join("examples")
            .join(k.to_string())
            .join("reference.wav");
        let expected = clamped_outputs(&reference, &notes_of(clip.patch_seed), &weights, &stats);
        assert_eq!(clip.clamped, expected, "{clip:?}");
        assert_eq!(clip.clamped, 3, "1.5, -0.5 and 2.0 times about 1: {clip:?}");
    }
    let clamped = clips.iter().filter(|c| c.clamped > 0).count();
    let file = fs::read_to_string(out.join("benchmark.txt")).unwrap();
    assert_eq!(
        value(&records(&file), "clamped_predictions"),
        format!("{clamped} of 2")
    );
    assert_eq!(
        text(&output.stdout),
        file[file.find("default_median_loss ").unwrap()..]
    );
}

/// `--kind ml-init` without `--weights` and `--normalization` reaches `StartBenchmark`, which
/// refuses it `WeightArtifactRequired` (Da12-2): exit 1, nothing written.
#[test]
fn ml_init_without_a_weight_artifact_is_refused() {
    let dir = scratch("no-weights");
    let out = dir.join("bench");
    let line = assert_refused(
        &benchmark(&out, &["--kind", "ml-init", "--examples", "1"], &[]),
        "fit",
        "patchback.fit.WeightArtifactRequired",
        &out,
    );
    assert!(line.contains("\"subtractive-mono/1\""), "{line}");
}

/// `--weights` or `--normalization` with the engine baseline, by default or named, is a usage
/// error (2); so is one of the two without the other, and a kind that is neither.
#[test]
fn the_kind_and_file_flags_are_usage_checked() {
    let dir = scratch("usage");
    let weights = write(
        &dir,
        "regressor.weights",
        &artifact_bytes(&TARGETS, "features/1"),
    );
    let stats = write(
        &dir,
        "normalization.stats",
        wide_stats("features/1").as_bytes(),
    );
    let out = dir.join("bench");
    let (w, s) = (weights.to_str().unwrap(), stats.to_str().unwrap());
    for flags in [
        vec!["--weights", w, "--normalization", s],
        vec![
            "--kind",
            "engine-baseline",
            "--weights",
            w,
            "--normalization",
            s,
        ],
        vec!["--kind", "ml-init", "--weights", w],
        vec!["--kind", "ml-init", "--normalization", s],
        vec!["--weights", w],
        vec![
            "--kind",
            "ml-init-plus-search",
            "--weights",
            w,
            "--normalization",
            s,
        ],
    ] {
        let output = benchmark(&out, &["--examples", "1"], &flags);
        let stderr = assert_usage_error(&output, &out);
        assert!(
            stderr.contains("--weights")
                || stderr.contains("--normalization")
                || stderr.contains("--kind"),
            "{flags:?}: {stderr}"
        );
    }
}

/// The inputs are read and checked before any clip is rendered (Da12-3), and refused as
/// `patchback fit` refuses them (Da11-6): a missing artifact, a corrupt one, missing statistics,
/// and an artifact or statistics of another feature contract than `features/1`. Exit 1, one
/// line, nothing written.
#[test]
fn the_seed_inputs_are_refused_as_the_fit_refuses_them() {
    let dir = scratch("inputs");
    let good = write(
        &dir,
        "regressor.weights",
        &artifact_bytes(&TARGETS, "features/1"),
    );
    let stats = write(
        &dir,
        "normalization.stats",
        wide_stats("features/1").as_bytes(),
    );
    let corrupt = write(&dir, "corrupt.weights", b"weights/1\nnot an artifact\n");
    let features_2 = write(
        &dir,
        "features2.weights",
        &artifact_bytes(&TARGETS, "features/2"),
    );
    let stats_2 = write(&dir, "features2.stats", wide_stats("features/2").as_bytes());
    let missing = dir.join("absent");
    let out = dir.join("bench");
    for (weights, normalization, domain, error) in [
        (
            &missing,
            &stats,
            "model",
            "patchback.model.UnknownWeightArtifact",
        ),
        (
            &corrupt,
            &stats,
            "model",
            "patchback.model.CorruptWeightArtifact",
        ),
        (
            &good,
            &missing,
            "analysis",
            "patchback.analysis.NormalizationStatsNotFound",
        ),
        (
            &features_2,
            &stats,
            "fit",
            "patchback.fit.FeatureContractMismatch",
        ),
        (
            &good,
            &stats_2,
            "fit",
            "patchback.fit.FeatureContractMismatch",
        ),
    ] {
        let line = assert_refused(
            &benchmark(&out, SMALL, &ml_init(weights, normalization)),
            domain,
            error,
            &out,
        );
        if error == "patchback.fit.FeatureContractMismatch" {
            assert!(line.contains("features/2"), "{line}");
        }
    }
}

/// `--kind engine-baseline` named is the default: the same flags with and without it write the
/// same files, byte for byte, `benchmark.txt` but its `wall_time_ms` record; three listening
/// files per clip; no ML-init record.
#[test]
fn the_engine_baseline_is_the_default_kind_and_unchanged() {
    let dir = scratch("baseline");
    let default = dir.join("default");
    let named = dir.join("named");
    for (out, extra) in [
        (&default, &[][..]),
        (&named, &["--kind", "engine-baseline"][..]),
    ] {
        let output = benchmark(out, TWO_RENDERS, extra);
        assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    }
    assert_eq!(listing(&default), listing(&named));
    for name in listing(&default) {
        let path = default.join(&name);
        if path.is_dir() {
            continue;
        }
        let (a, b) = (
            fs::read(&path).unwrap(),
            fs::read(named.join(&name)).unwrap(),
        );
        if name == "benchmark.txt" {
            let without_wall = |bytes: &[u8]| -> Vec<String> {
                text(bytes)
                    .lines()
                    .filter(|line| !line.starts_with("wall_time_ms "))
                    .map(str::to_owned)
                    .collect()
            };
            assert_eq!(without_wall(&a), without_wall(&b));
            let file = text(&a);
            assert!(file.contains("\nkind engine-baseline\n"), "{file}");
            for key in [
                "raw_regressor_median_loss",
                "regressor_search_median_loss",
                "clamped_predictions",
                "weights",
                "feature_contract",
            ] {
                assert!(!file.contains(&format!("\n{key} ")), "{key}: {file}");
            }
        } else {
            assert_eq!(a, b, "{name}");
        }
    }
    let mut files: Vec<String> = fs::read_dir(default.join("examples/0"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(files, ["default.wav", "fit.wav", "reference.wav"]);
}
