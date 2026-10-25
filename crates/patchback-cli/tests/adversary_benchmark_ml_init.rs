//! Adversary cases for `patchback benchmark --kind ml-init` (story:ml-init-benchmark, wave a12,
//! pass 1). Each case drives the verb, or the library path the verb takes for a holdout clip,
//! against a statement of the unit's own module documentation or of a decision of
//! approval-record:wave-a12:
//!
//! - Da12-7 and Da12-15: can a generated holdout clip reach the clip refusal of a refused
//!   analysis, or a refused seeded fit? The holdout walk of the documented stream, each admitted
//!   draw rendered as the verb renders it and given the two calls the verb makes for arms 4 and 3.
//! - Da12-3 and the module documentation's determinism line: the weights and statistics files
//!   replaced while the run is under way change nothing; two runs of the same flags are byte-equal
//!   but `wall_time_ms`; a holdout larger than `LISTENING_EXAMPLE_COUNT`, with four clips so the
//!   lower median is not the minimum.
//! - Da12-16: on weights that clamp, arm 3's `predicted.wav` compares back to its `raw_loss` and is
//!   the reference's length; a holdout of one clip.
//! - Step 0 before everything: an input refusal wins over an existing `<out>` file and over
//!   `--examples 0`, leaves the file untouched and takes no holdout's rendering time.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use patchback_analysis::{AnalysisConfig, FEATURES_1_VALUE_COUNT, analyze_clip, read_stats};
use patchback_audio::AudioClip;
use patchback_core::digest::{hex, sha256};
use patchback_core::parameter::Topology;
use patchback_core::patch::{compile, format_decimal, validate};
use patchback_dsp::{RenderConfig, render};
use patchback_fit::{
    FitBounds, FitConfig, FitRequest, SeedInputs, fit, fit_request, reference_refusal,
};
use patchback_model::{ModelSpecPin, Predictor, W1_ELEMENTS, W1_SHAPE, W2_SHAPE};

const HIDDEN: usize = W1_SHAPE[1];
const OUTPUTS: usize = W2_SHAPE[1];

/// Eight targets inside [0, 1] (the implementor's `TARGETS`).
const TARGETS: [f32; OUTPUTS] = [0.2, 0.3, 0.4, 0.6, 0.7, 0.8, 0.35, 0.65];
/// Three targets outside [0, 1] (the implementor's `CLAMPED_TARGETS`).
const CLAMPED_TARGETS: [f32; OUTPUTS] = [1.5, -0.5, 2.0, 0.5, 0.25, 0.75, 0.4, 0.6];

/// `HOLDOUT_DOMAIN` of the verb (Da8-46).
const HOLDOUT_DOMAIN: u64 = 0x686f_6c64_6f75_7431;

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-adversary-ml-init")
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

fn args(out: &Path, flags: &[&str]) -> Vec<String> {
    let mut args = vec![
        "benchmark".to_owned(),
        "--out".to_owned(),
        out.to_str().unwrap().to_owned(),
    ];
    args.extend(flags.iter().map(|s| (*s).to_owned()));
    args
}

fn patchback(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(args)
        .output()
        .expect("the patchback binary runs")
}

fn artifact_bytes(targets: &[f32; OUTPUTS]) -> Vec<u8> {
    let mut w1 = vec![0.0f32; W1_ELEMENTS];
    let mut w2 = vec![0.0f32; HIDDEN * OUTPUTS];
    for (k, target) in targets.iter().enumerate() {
        w1[k] = 1.0;
        w2[k * OUTPUTS + k] = *target;
    }
    let payload: Vec<u8> = w1.iter().chain(&w2).flat_map(|v| v.to_le_bytes()).collect();
    let pin = ModelSpecPin::repository().unwrap();
    let header = format!(
        "weights/1\nmodel PatchRegressor\nmodelspec {} {}\nfeatures features/1\nw1 32 64\nw2 64 8\nsha256 {}\n\n",
        pin.version(),
        pin.commit(),
        hex(&sha256(&payload))
    );
    [header.as_bytes(), &payload].concat()
}

fn wide_stats() -> String {
    let mut text = "patchback-normalization-stats 1\nfeature_contract features/1\nsplit train\nrows 16\nstd population\n".to_owned();
    for i in 0..FEATURES_1_VALUE_COUNT {
        text.push_str(&format!(
            "feature {i} {} {}\n",
            format_decimal(-1e6),
            format_decimal(1e6)
        ));
    }
    text
}

fn records(text: &str) -> Vec<(String, String)> {
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

/// The fields of each `clip` line of a `clips.txt`.
fn clip_fields(clips: &str) -> Vec<Vec<String>> {
    clips
        .lines()
        .skip(2)
        .map(|line| line.split(' ').map(str::to_owned).collect())
        .collect()
}

fn lower_median_text(values: &[f64]) -> String {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sorted[(sorted.len() - 1) / 2].to_string()
}

fn lower_median_count(values: &[i64]) -> String {
    let mut sorted = values.to_vec();
    sorted.sort();
    sorted[(sorted.len() - 1) / 2].to_string()
}

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

/// Every file under `dir` with its bytes, relative paths sorted; `benchmark.txt`'s
/// `wall_time_ms` line removed.
fn tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, into: &mut Vec<(String, Vec<u8>)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if path.is_dir() {
                into.push((name.clone() + "/", Vec::new()));
                walk(root, &path, into);
            } else {
                let mut bytes = fs::read(&path).unwrap();
                if name == "benchmark.txt" {
                    bytes = text(&bytes)
                        .lines()
                        .filter(|line| !line.starts_with("wall_time_ms "))
                        .map(|line| format!("{line}\n"))
                        .collect::<String>()
                        .into_bytes();
                }
                into.push((name, bytes));
            }
        }
    }
    let mut all = Vec::new();
    walk(dir, dir, &mut all);
    all.sort();
    all
}

/// Da12-7, Da12-15: every holdout clip the documented stream admits, rendered as the verb renders
/// it (`render_reference`, at the default block size and at block size 7), is analysed by
/// `analyze_clip` with `AnalysisConfig::default()` (arm 4's analysis) and its regressor-seeded fit
/// completes (`fit_request`, one render). The dataset pipeline admits a draw only after
/// `analyze_and_extract` of `features/1` over the whole render, whose analysis is
/// `AnalysisConfig::default()`'s, so a refused analysis is a diagnostic draw and skipped; this walks
/// 40 admitted clips over five seeds to measure whether either clip refusal is reached.
#[test]
fn no_admitted_holdout_clip_reaches_the_analysis_or_seeded_fit_refusal() {
    let dir = scratch("reachability");
    let weights = write(&dir, "regressor.weights", &artifact_bytes(&TARGETS));
    let stats = wide_stats();
    let seed = SeedInputs {
        predictor: Predictor::load(&weights).unwrap(),
        normalization: read_stats(&stats).unwrap(),
        normalization_sha256: sha256(stats.as_bytes()),
    };
    let topology = Topology::subtractive_mono();
    let pipeline = patchback_dataset::Pipeline::new();
    let config = FitConfig {
        bounds: FitBounds {
            max_renders: 1,
            max_wall_time_ms: 600_000,
            max_restarts: 1,
            improvement_threshold: 1e-4,
        },
        seed: 0,
        render: RenderConfig::default(),
    };
    let mut refused = Vec::new();
    let mut admitted = 0;
    for holdout_seed in [0_u64, 1, 7, 42, 1234] {
        let mut stream = patchback_core::prng::SplitMix64::new(holdout_seed ^ HOLDOUT_DOMAIN);
        let mut clips = 0;
        while clips < 8 {
            let patch_seed = stream.next_u64();
            let draw = patchback_dataset::PatchDraw::from_seed(patch_seed);
            let patchback_dataset::Rendering::Sample(_) = pipeline.run(&draw).unwrap() else {
                continue;
            };
            let fit_seed = stream.next_u64();
            clips += 1;
            admitted += 1;
            let plan = compile(&validate(&draw.patch(&topology)).unwrap());
            let notes = draw.notes();
            let reference =
                AudioClip::from_mono(render(&plan, &notes, &config.render).unwrap().samples)
                    .unwrap();
            let block_7 = render(
                &plan,
                &notes,
                &RenderConfig {
                    block_size_frames: 7,
                },
            )
            .unwrap()
            .samples;
            if block_7 != reference.samples() {
                refused.push(format!(
                    "seed {holdout_seed} patch {patch_seed}: block size 7 renders other samples"
                ));
            }
            let analysis = match analyze_clip(&reference, &AnalysisConfig::default()) {
                Ok(analysis) => analysis,
                Err(error) => {
                    refused.push(format!(
                        "seed {holdout_seed} patch {patch_seed}: analysis refused: {}",
                        reference_refusal(error)
                    ));
                    continue;
                }
            };
            let outcome = fit_request(FitRequest {
                reference: &reference,
                notes: &notes,
                topology: &topology,
                config: &FitConfig {
                    seed: fit_seed,
                    ..config
                },
                analysis: Some(&analysis),
                seed: Some(seed.clone()),
            });
            match outcome {
                Ok(outcome) if outcome.run.best_loss().is_some() && outcome.seeded.is_some() => {}
                Ok(_) => refused.push(format!(
                    "seed {holdout_seed} patch {patch_seed}: seeded fit holds no valid start"
                )),
                Err(error) => refused.push(format!(
                    "seed {holdout_seed} patch {patch_seed}: seeded fit refused: {} {error}",
                    error.domain_error()
                )),
            }
        }
    }
    assert_eq!(admitted, 40);
    assert!(refused.is_empty(), "{refused:#?}");
}

/// Da12-3 and determinism: the same ML-init flags twice, the second run's `--weights` and
/// `--normalization` files replaced (other weights that clamp, statistics that do not read) half a
/// second after it starts, while it is still under way. Both complete with the same files byte for
/// byte, `benchmark.txt` but `wall_time_ms`; the second's records name the files as they were when
/// read. Four clips: `LISTENING_EXAMPLE_COUNT` (3) example directories, 15 `listening_example`
/// lines, and the four medians the lower medians recomputed from `clips.txt` (index 1 of 4, not the
/// minimum).
#[test]
fn inputs_replaced_mid_run_change_nothing_and_reruns_are_byte_equal() {
    let dir = scratch("read-once");
    let artifact = artifact_bytes(&TARGETS);
    let stats = wide_stats();
    let flags = |out: &Path, weights: &Path, normalization: &Path| {
        args(
            out,
            &[
                "--examples",
                "4",
                "--seed",
                "11",
                "--max-renders",
                "3",
                "--max-restarts",
                "1",
                "--kind",
                "ml-init",
                "--weights",
                weights.to_str().unwrap(),
                "--normalization",
                normalization.to_str().unwrap(),
            ],
        )
    };
    let weights_a = write(&dir, "a.weights", &artifact);
    let stats_a = write(&dir, "a.stats", stats.as_bytes());
    let out_a = dir.join("a");
    let first = patchback(&flags(&out_a, &weights_a, &stats_a));
    assert_eq!(first.status.code(), Some(0), "{}", text(&first.stderr));

    let weights_b = write(&dir, "b.weights", &artifact);
    let stats_b = write(&dir, "b.stats", stats.as_bytes());
    let out_b = dir.join("b");
    let mut child = Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(flags(&out_b, &weights_b, &stats_b))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        child.try_wait().unwrap().is_none(),
        "the run ended before its inputs were replaced: the case measured nothing"
    );
    fs::write(&weights_b, artifact_bytes(&CLAMPED_TARGETS)).unwrap();
    fs::write(&stats_b, b"not statistics\n").unwrap();
    let second = child.wait_with_output().unwrap();
    assert_eq!(second.status.code(), Some(0), "{}", text(&second.stderr));
    assert!(
        out_b.join("benchmark.txt").exists(),
        "the run wrote nothing"
    );
    assert_eq!(tree(&out_a), tree(&out_b));

    let without_wall = |bytes: &[u8]| -> String {
        text(bytes)
            .lines()
            .filter(|line| !line.starts_with("wall_time_ms "))
            .map(|line| format!("{line}\n"))
            .collect()
    };
    assert_eq!(without_wall(&first.stdout), without_wall(&second.stdout));

    let file = fs::read_to_string(out_a.join("benchmark.txt")).unwrap();
    let recs = records(&file);
    assert_eq!(
        value(&recs, "weights"),
        format!(
            "sha256 {}",
            hex(&sha256(
                &artifact[artifact.len() - (W1_ELEMENTS + HIDDEN * OUTPUTS) * 4..]
            ))
        )
    );
    assert_eq!(
        value(&recs, "normalization"),
        format!("sha256 {}", hex(&sha256(stats.as_bytes())))
    );
    let clips = clip_fields(&fs::read_to_string(out_a.join("clips.txt")).unwrap());
    assert_eq!(clips.len(), 4);
    let real = |i: usize| -> Vec<f64> { clips.iter().map(|c| c[i].parse().unwrap()).collect() };
    let count = |i: usize| -> Vec<i64> { clips.iter().map(|c| c[i].parse().unwrap()).collect() };
    for (key, expected) in [
        ("default_median_loss", lower_median_text(&real(4))),
        ("default_search_median_loss", lower_median_text(&real(5))),
        ("raw_regressor_median_loss", lower_median_text(&real(9))),
        ("regressor_search_median_loss", lower_median_text(&real(10))),
        (
            "default_search_median_evaluations",
            lower_median_count(&count(6)),
        ),
        (
            "regressor_search_median_evaluations",
            lower_median_count(&count(11)),
        ),
        ("clamped_predictions", "0 of 4".to_owned()),
    ] {
        assert_eq!(value(&recs, key), expected, "{key}");
    }
    let listening: Vec<&str> = recs
        .iter()
        .filter(|(k, _)| k == "listening_example")
        .map(|(_, v)| v.as_str())
        .collect();
    let mut expected = Vec::new();
    for k in 0..3 {
        for name in [
            "reference.wav",
            "default.wav",
            "fit.wav",
            "predicted.wav",
            "seeded.wav",
        ] {
            expected.push(format!("examples/{k}/{name}"));
        }
    }
    assert_eq!(listening, expected);
    assert!(!out_a.join("examples/3").exists());
    assert_eq!(
        text(&first.stdout),
        file[file.find("default_median_loss ").unwrap()..]
    );
}

/// Da12-16 on weights that clamp, and a holdout of one clip: `predicted.wav` is the reference's
/// length and `patchback compare` of it gives the clip's `raw_loss`; `seeded.wav` its
/// `seeded_best_loss`; the seeded fit's restart 0 is arm 3's point, so `seeded_best_loss <=
/// raw_loss`; the medians of one clip are its values; `clamped_predictions 1 of 1`.
#[test]
fn a_clamped_prediction_compares_back_on_a_holdout_of_one() {
    let dir = scratch("clamped-one");
    let weights = write(&dir, "clamped.weights", &artifact_bytes(&CLAMPED_TARGETS));
    let stats = write(&dir, "wide.stats", wide_stats().as_bytes());
    let out = dir.join("bench");
    let output = patchback(&args(
        &out,
        &[
            "--examples",
            "1",
            "--seed",
            "5",
            "--max-renders",
            "3",
            "--max-restarts",
            "1",
            "--kind",
            "ml-init",
            "--weights",
            weights.to_str().unwrap(),
            "--normalization",
            stats.to_str().unwrap(),
        ],
    ));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let clips = clip_fields(&fs::read_to_string(out.join("clips.txt")).unwrap());
    assert_eq!(clips.len(), 1);
    let clip = &clips[0];
    assert_eq!(clip[14], "3", "{clip:?}");
    let raw: f64 = clip[9].parse().unwrap();
    let seeded: f64 = clip[10].parse().unwrap();
    assert!(seeded <= raw, "{clip:?}");
    let example = out.join("examples/0");
    let reference = example.join("reference.wav");
    let reference_frames = patchback_audio::decode(&fs::read(&reference).unwrap())
        .unwrap()
        .frame_count();
    for (file, loss) in [("predicted.wav", &clip[9]), ("seeded.wav", &clip[10])] {
        let candidate = example.join(file);
        let frames = patchback_audio::decode(&fs::read(&candidate).unwrap())
            .unwrap()
            .frame_count();
        assert_eq!(frames, reference_frames, "{file}");
        assert_eq!(&compared_loss(&reference, &candidate), loss, "{file}");
    }
    let recs = records(&fs::read_to_string(out.join("benchmark.txt")).unwrap());
    for (key, field) in [
        ("default_median_loss", 4),
        ("default_search_median_loss", 5),
        ("default_search_median_evaluations", 6),
        ("raw_regressor_median_loss", 9),
        ("regressor_search_median_loss", 10),
        ("regressor_search_median_evaluations", 11),
    ] {
        assert_eq!(value(&recs, key), clip[field], "{key}");
    }
    assert_eq!(value(&recs, "clamped_predictions"), "1 of 1");
}

/// Step 0 runs before everything (module documentation, "The run"): a corrupt artifact is refused
/// `CorruptWeightArtifact` before `InvalidHoldoutSize` (`--examples 0`) and before an existing
/// `<out>/benchmark.txt`, which is left as it was; and before any clip, so a holdout of 200 clips
/// is refused in well under one clip's rendering time. Statistics that `read_stats` refuses (31
/// feature rows) are refused `NormalizationStatsNotFound` with `read_stats`'s own message, as
/// `patchback fit` words it.
#[test]
fn input_refusals_come_first_and_take_no_rendering_time() {
    let dir = scratch("refusal-order");
    let corrupt = write(&dir, "corrupt.weights", b"weights/1\nnot an artifact\n");
    let good = write(&dir, "good.weights", &artifact_bytes(&TARGETS));
    let stats = write(&dir, "wide.stats", wide_stats().as_bytes());
    let short_text: String = wide_stats()
        .lines()
        .filter(|line| !line.starts_with("feature 31 "))
        .map(|line| format!("{line}\n"))
        .collect();
    let short = write(&dir, "short.stats", short_text.as_bytes());
    let out = dir.join("bench");
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("benchmark.txt"), b"kept\n").unwrap();
    for examples in ["0", "200"] {
        let started = Instant::now();
        let output = patchback(&args(
            &out,
            &[
                "--examples",
                examples,
                "--kind",
                "ml-init",
                "--weights",
                corrupt.to_str().unwrap(),
                "--normalization",
                stats.to_str().unwrap(),
            ],
        ));
        let elapsed = started.elapsed();
        let stderr = text(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{stderr}");
        assert!(
            stderr.starts_with(
                "patchback: benchmark refused: patchback.model.CorruptWeightArtifact: "
            ),
            "{stderr}"
        );
        assert_eq!(stderr.lines().count(), 1, "{stderr}");
        assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
        assert_eq!(fs::read(out.join("benchmark.txt")).unwrap(), b"kept\n");
        assert_eq!(fs::read_dir(&out).unwrap().count(), 1);
    }
    let output = patchback(&args(
        &out,
        &[
            "--examples",
            "200",
            "--kind",
            "ml-init",
            "--weights",
            good.to_str().unwrap(),
            "--normalization",
            short.to_str().unwrap(),
        ],
    ));
    let reason = read_stats(&short_text).unwrap_err();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        text(&output.stderr),
        format!(
            "patchback: benchmark refused: patchback.analysis.NormalizationStatsNotFound: \
             {short:?}: {reason}\n"
        )
    );
    assert_eq!(fs::read(out.join("benchmark.txt")).unwrap(), b"kept\n");
}

/// Da12-4: arm 4 is `fit_request` with the clip's `fit_seed`, so its restarts 1 and later start
/// where arm 2's do. Every other test runs `--max-restarts 1`, where restart 1 never begins and
/// arm 4's seed is unobservable. Here `--max-restarts 2` with an improvement threshold no sweep
/// meets ends restart 0 after one sweep and lets restart 1 begin. Each clip's seeded columns are
/// those of `fit_request` with the reference read back from `reference.wav`, the clip's notes
/// and the clip's `fit_seed`, and that replicate's restart 1 starts where `fit`'s restart 1 starts.
/// The replicate with another seed differs, so the case tells the clip's seed from another.
#[test]
fn the_seeded_arm_runs_with_the_clips_fit_seed_and_pairs_restart_1() {
    let dir = scratch("paired");
    let weights = write(&dir, "regressor.weights", &artifact_bytes(&TARGETS));
    let stats = wide_stats();
    let stats_path = write(&dir, "wide.stats", stats.as_bytes());
    let out = dir.join("bench");
    let output = patchback(&args(
        &out,
        &[
            "--examples",
            "1",
            "--seed",
            "7",
            "--max-renders",
            "40",
            "--max-restarts",
            "2",
            "--improvement-threshold",
            "1000000000",
            "--kind",
            "ml-init",
            "--weights",
            weights.to_str().unwrap(),
            "--normalization",
            stats_path.to_str().unwrap(),
        ],
    ));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let clips = clip_fields(&fs::read_to_string(out.join("clips.txt")).unwrap());
    assert_eq!(clips.len(), 1);
    let topology = Topology::subtractive_mono();
    let seed = SeedInputs {
        predictor: Predictor::load(&weights).unwrap(),
        normalization: read_stats(&stats).unwrap(),
        normalization_sha256: sha256(stats.as_bytes()),
    };
    for (k, clip) in clips.iter().enumerate() {
        assert_eq!(clip[12], "2", "restart 1 began: {clip:?}");
        let patch_seed: u64 = clip[2].parse().unwrap();
        let fit_seed: u64 = clip[3].parse().unwrap();
        let reference = patchback_audio::decode(
            &fs::read(
                out.join("examples")
                    .join(k.to_string())
                    .join("reference.wav"),
            )
            .unwrap(),
        )
        .unwrap();
        let notes = patchback_dataset::PatchDraw::from_seed(patch_seed).notes();
        let analysis = analyze_clip(&reference, &AnalysisConfig::default()).unwrap();
        let config = |seed: u64| FitConfig {
            bounds: FitBounds {
                max_renders: 40,
                max_wall_time_ms: 60_000,
                max_restarts: 2,
                improvement_threshold: 1e9,
            },
            seed,
            render: RenderConfig::default(),
        };
        let seeded = |fit_seed: u64| {
            fit_request(FitRequest {
                reference: &reference,
                notes: &notes,
                topology: &topology,
                config: &config(fit_seed),
                analysis: Some(&analysis),
                seed: Some(seed.clone()),
            })
            .unwrap()
        };
        let columns = |o: &patchback_fit::FitOutcome| -> Vec<String> {
            vec![
                o.run.best_loss().unwrap().to_string(),
                o.run.evaluations().to_string(),
                o.restarts.to_string(),
                o.run.stop_reason().unwrap().name().to_owned(),
            ]
        };
        let replicate = seeded(fit_seed);
        assert_eq!(columns(&replicate), clip[10..14].to_vec(), "clip {k}");
        let other = seeded(fit_seed ^ 1);
        let restart_1 = |o: &patchback_fit::FitOutcome| -> Vec<f64> {
            o.log
                .iter()
                .find(|e| e.restart == 1)
                .expect("restart 1 began")
                .normalized
                .clone()
        };
        assert_ne!(
            restart_1(&other),
            restart_1(&replicate),
            "another seed starts restart 1 elsewhere"
        );
        assert_ne!(
            columns(&other),
            clip[10..14].to_vec(),
            "clip {k}: another seed gives other seeded columns, so the case tells the seeds apart"
        );
        let default_started = fit(&reference, &notes, &topology, &config(fit_seed)).unwrap();
        assert_eq!(
            restart_1(&replicate),
            restart_1(&default_started),
            "clip {k}"
        );
    }
}
