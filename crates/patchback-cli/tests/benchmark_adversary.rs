//! Adversary cases for `patchback benchmark` (story:engine-baseline, wave a8, pass 1): the
//! holdout derivation recomputed from the module documentation, the draw bound, the
//! classification against the fit's own reference check, the block size, the `--out` rule at its
//! edges, the refusal order and the listening examples of every clip that gets them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_analysis::{AnalysisConfig, analyze_clip};
use patchback_audio::{AudioClip, read_wav};
use patchback_core::parameter::Topology;
use patchback_core::patch::{compile, validate};
use patchback_core::prng::SplitMix64;
use patchback_dataset::{MAX_DRAWS_PER_SAMPLE, PatchDraw, Pipeline, Rendering};
use patchback_dsp::{RenderConfig, render};

/// The holdout stream's domain as the verb documents it (`HOLDOUT_DOMAIN`, Da8-46): the stream
/// is `SplitMix64::new(seed ^ HOLDOUT_DOMAIN)`.
const HOLDOUT_DOMAIN: u64 = 0x686f_6c64_6f75_7431;

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-benchmark-adversary")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn benchmark(out: &Path, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .arg("benchmark")
        .arg("--out")
        .arg(out)
        .args(flags)
        .output()
        .unwrap()
}

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
            if path.is_dir() && !path.is_symlink() {
                walk(root, &path, into);
            }
        }
    }
    let mut all = Vec::new();
    walk(dir, dir, &mut all);
    all.sort();
    all
}

fn assert_refused(output: &Output, prefix: &str) {
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with(prefix),
        "expected {prefix:?}, got {stderr}"
    );
}

/// The holdout as the module documentation states it: `SplitMix64::new(seed ^ HOLDOUT_DOMAIN)`
/// read in order; a
/// patch seed per draw, a diagnostic draw skipped, an admitted draw followed by its fit seed; at
/// most `4 × count` draws. `Err(draws)` when the bound is reached first.
fn documented_holdout(seed: u64, count: usize) -> Result<Vec<(PatchDraw, u64)>, u64> {
    let pipeline = Pipeline::new();
    let max_draws = count as u64 * MAX_DRAWS_PER_SAMPLE;
    let mut stream = SplitMix64::new(seed ^ HOLDOUT_DOMAIN);
    let mut draws = 0;
    let mut holdout = Vec::new();
    while holdout.len() < count {
        if draws >= max_draws {
            return Err(draws);
        }
        let draw = PatchDraw::from_seed(stream.next_u64());
        draws += 1;
        match pipeline
            .run(&draw)
            .expect("the pipeline classifies the draw")
        {
            Rendering::Sample(_) => holdout.push((draw, stream.next_u64())),
            Rendering::Diagnostic(_) => {}
        }
    }
    Ok(holdout)
}

/// Clip `draw`'s reference as the documentation states it: rendered whole at `block`.
fn reference(draw: &PatchDraw, block: i64) -> AudioClip {
    let topology = Topology::subtractive_mono();
    let plan = compile(&validate(&draw.patch(&topology)).unwrap());
    let rendered = render(
        &plan,
        &draw.notes(),
        &RenderConfig {
            block_size_frames: block,
        },
    )
    .unwrap();
    AudioClip::from_mono(rendered.samples).unwrap()
}

/// Whether the first draw of `seed` is a diagnostic example.
fn first_draw_is_diagnostic(pipeline: &Pipeline, seed: u64) -> bool {
    let draw = PatchDraw::from_seed(SplitMix64::new(seed ^ HOLDOUT_DOMAIN).next_u64());
    matches!(pipeline.run(&draw), Ok(Rendering::Diagnostic(_)))
}

struct ClipLine {
    patch_seed: u64,
    fit_seed: u64,
    default_loss: f64,
    best_loss: f64,
    raw: Vec<String>,
}

fn clip_lines(out: &Path) -> Vec<ClipLine> {
    fs::read_to_string(out.join("clips.txt"))
        .unwrap()
        .lines()
        .skip(2)
        .map(|line| {
            let raw: Vec<String> = line.split(' ').map(str::to_owned).collect();
            ClipLine {
                patch_seed: raw[2].parse().unwrap(),
                fit_seed: raw[3].parse().unwrap(),
                default_loss: raw[4].parse().unwrap(),
                best_loss: raw[5].parse().unwrap(),
                raw,
            }
        })
        .collect()
}

/// approval-record:engine-baseline-holdout and the story's acceptance run the benchmark over a
/// holdout of `example_count` clips (200 in the target scenarios). Each clip the pipeline admits
/// is then fitted, and `patchback_fit::fit` first analyses the whole reference
/// (`check_reference`: silence, pitch); a clip it refuses ends the benchmark with exit 1 (Da8-3).
/// So every admitted clip must be one the fit's analysis accepts, or the benchmark of the
/// scenario's size fails on its own holdout. Checked over the first 32 clips of `--seed 0` (all
/// 200 held when this was written, in 76 s; 32 keep the suite short).
#[test]
fn every_admitted_holdout_clip_is_one_the_fit_accepts() {
    let holdout = documented_holdout(0, 32).expect("the holdout is drawn");
    let refused: Vec<String> = holdout
        .iter()
        .enumerate()
        .filter_map(|(k, (draw, _))| {
            analyze_clip(&reference(draw, 256), &AnalysisConfig::default())
                .err()
                .map(|error| {
                    format!(
                        "clip {k} (patch seed {}, MIDI {}, gate {} frames): {}: {error}",
                        draw.patch_seed,
                        draw.midi_note,
                        draw.gate_samples,
                        error.domain_error()
                    )
                })
        })
        .collect();
    assert!(
        refused.is_empty(),
        "{} of 32 admitted clips are refused by the fit's reference analysis; first: {}",
        refused.len(),
        refused[0]
    );
}

/// `--seed` whose first draw is a diagnostic example: `clips.txt`'s seeds are exactly the
/// documented stream (the skipped draw uses up one output), every listening example's reference
/// is the documented reference, `fit.wav` and `default.wav` have its frame count, and `compare`
/// of each pair reproduces the recorded loss bit for bit, for clips 0, 1 and 2. Every real in
/// `clips.txt` is the shortest text of its value.
#[test]
fn the_holdout_and_the_listening_examples_are_the_documented_ones() {
    let pipeline = Pipeline::new();
    let seed = (0..256)
        .find(|&s| first_draw_is_diagnostic(&pipeline, s) && documented_holdout(s, 3).is_ok())
        .expect("a seed whose first draw is a diagnostic example");
    let expected = documented_holdout(seed, 3).unwrap();

    let dir = scratch("documented");
    let out = dir.join("bench");
    let seed_text = seed.to_string();
    let output = benchmark(
        &out,
        &[
            "--examples",
            "3",
            "--seed",
            &seed_text,
            "--max-renders",
            "2",
            "--max-restarts",
            "1",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let clips = clip_lines(&out);
    assert_eq!(clips.len(), 3);
    for (k, (clip, (draw, fit_seed))) in clips.iter().zip(&expected).enumerate() {
        assert_eq!(
            (clip.patch_seed, clip.fit_seed),
            (draw.patch_seed, *fit_seed),
            "clip {k} of seed {seed}"
        );
        for field in &clip.raw[4..6] {
            let value: f64 = field.parse().unwrap();
            assert_eq!(&value.to_string(), field, "clip {k}");
        }
        let dir = out.join("examples").join(k.to_string());
        let recorded = read_wav(&dir.join("reference.wav")).unwrap();
        assert_eq!(
            recorded.samples(),
            reference(draw, 256).samples(),
            "clip {k}"
        );
        for (name, loss) in [
            ("fit.wav", clip.best_loss),
            ("default.wav", clip.default_loss),
        ] {
            let candidate = read_wav(&dir.join(name)).unwrap();
            assert_eq!(
                candidate.frame_count(),
                recorded.frame_count(),
                "clip {k} {name}"
            );
            let compared = patchback_fit::compare(&recorded, &candidate).unwrap();
            assert_eq!(compared.total.to_bits(), loss.to_bits(), "clip {k} {name}");
        }
    }
}

/// `MAX_DRAWS_PER_SAMPLE` is reachable: a seed whose first four draws are diagnostic examples
/// cannot draw a holdout of one clip. Exit 1, the documented refusal, nothing written.
#[test]
fn a_holdout_whose_draws_run_out_is_refused_and_writes_nothing() {
    let seed = (0..4096)
        .find(|&s| documented_holdout(s, 1).is_err())
        .expect("a seed whose first four draws are diagnostic examples");
    let dir = scratch("exhausted");
    let out = dir.join("bench");
    let output = benchmark(
        &out,
        &[
            "--examples",
            "1",
            "--seed",
            &seed.to_string(),
            "--max-renders",
            "1",
            "--max-restarts",
            "1",
        ],
    );
    assert_refused(
        &output,
        "patchback: benchmark refused: holdout exhausted: 4 patch draws gave 0 of the 1 clips",
    );
    assert!(!out.exists());
}

/// The block size changes how the render is cut, never the samples (`patchback_dsp::render`): two
/// runs at 256 and 100 frames write the same `clips.txt` and WAVs, and `benchmark.txt` differs
/// only in `block_size` and `wall_time_ms`. A one-clip holdout has one example directory and
/// three `listening_example` records.
#[test]
fn the_block_size_changes_no_sample_and_no_loss() {
    let dir = scratch("block-size");
    let flags = |block: &'static str| {
        [
            "--examples",
            "1",
            "--seed",
            "7",
            "--max-renders",
            "2",
            "--max-restarts",
            "1",
            "--block-size",
            block,
        ]
    };
    let a = dir.join("a");
    let b = dir.join("b");
    for (out, block) in [(&a, "256"), (&b, "100")] {
        let output = benchmark(out, &flags(block));
        assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    }
    assert_eq!(listing(&a), listing(&b));
    assert_eq!(
        fs::read_dir(a.join("examples"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>(),
        ["0"]
    );
    for name in listing(&a) {
        if name != "benchmark.txt" && !a.join(&name).is_dir() {
            assert_eq!(
                fs::read(a.join(&name)).unwrap(),
                fs::read(b.join(&name)).unwrap(),
                "{name}"
            );
        }
    }
    let lines = |out: &Path| -> Vec<String> {
        fs::read_to_string(out.join("benchmark.txt"))
            .unwrap()
            .lines()
            .filter(|l| !l.starts_with("wall_time_ms ") && !l.starts_with("block_size "))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(lines(&a), lines(&b));
    assert_eq!(
        lines(&a)
            .iter()
            .filter(|l| l.starts_with("listening_example "))
            .count(),
        3
    );
    assert!(
        fs::read_to_string(b.join("benchmark.txt"))
            .unwrap()
            .contains("\nblock_size 100\n")
    );
}

/// `examples` in any form is refused before any work: a file, an empty directory, a directory
/// holding `0/`, a dangling symbolic link. The directory is left as it was.
#[test]
fn an_examples_entry_of_any_kind_is_refused() {
    let flags = [
        "--examples",
        "1",
        "--max-renders",
        "1",
        "--max-restarts",
        "1",
    ];
    for case in ["file", "empty", "nested", "dangling"] {
        let dir = scratch(&format!("examples-{case}"));
        let out = dir.join("bench");
        fs::create_dir(&out).unwrap();
        let examples = out.join("examples");
        match case {
            "file" => fs::write(&examples, "x").unwrap(),
            "empty" => fs::create_dir(&examples).unwrap(),
            "nested" => fs::create_dir_all(examples.join("0")).unwrap(),
            _ => std::os::unix::fs::symlink(dir.join("nowhere"), &examples).unwrap(),
        }
        let before = listing(&out);
        let output = benchmark(&out, &flags);
        assert_refused(&output, "patchback: benchmark refused: cannot write ");
        assert!(text(&output.stderr).contains("examples"), "{case}");
        assert_eq!(listing(&out), before, "{case}");
    }
}

/// A failed write removes what the run wrote and nothing else: an `--out` that existed stays,
/// with the unrelated file it held.
#[cfg(unix)]
#[test]
fn a_failed_write_keeps_an_existing_out_and_its_unrelated_file() {
    let dir = scratch("failed-write-existing");
    let out = dir.join("bench");
    fs::create_dir(&out).unwrap();
    fs::write(out.join("notes.txt"), "mine\n").unwrap();
    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg("ulimit -f 16 && trap '' XFSZ && exec \"$0\" \"$@\"")
        .arg(env!("CARGO_BIN_EXE_patchback"))
        .arg("benchmark")
        .arg("--out")
        .arg(&out)
        .args([
            "--examples",
            "1",
            "--seed",
            "7",
            "--max-renders",
            "2",
            "--max-restarts",
            "1",
        ])
        .output()
        .unwrap();
    assert_refused(&output, "patchback: benchmark refused: cannot write ");
    assert_eq!(listing(&out), ["notes.txt"]);
    assert_eq!(fs::read_to_string(out.join("notes.txt")).unwrap(), "mine\n");
}

/// The refusals in the documented order (holdout size, topology, bounds, `--out`), each exit 1
/// with nothing created; a threshold that is no finite non-negative decimal is a usage error.
#[test]
fn the_refusals_answer_in_the_documented_order() {
    let dir = scratch("order");
    let file = dir.join("a-file");
    fs::write(&file, "x").unwrap();
    let out = dir.join("bench");
    for (flags, prefix) in [
        (
            &[
                "--examples",
                "0",
                "--topology",
                "no-such/1",
                "--max-renders",
                "0",
            ][..],
            "patchback.fit.InvalidHoldoutSize: ",
        ),
        (
            &[
                "--examples",
                "1",
                "--topology",
                "no-such/1",
                "--max-renders",
                "0",
            ][..],
            "patchback.fit.TopologyNotFound: ",
        ),
        (
            &["--examples", "1", "--max-wall-time-ms", "0"][..],
            "patchback.fit.InvalidSearchBound: ",
        ),
        (
            &["--examples", "1", "--max-restarts", "0"][..],
            "patchback.fit.InvalidSearchBound: ",
        ),
    ] {
        assert_refused(
            &benchmark(&out, flags),
            &format!("patchback: benchmark refused: {prefix}"),
        );
        assert!(!out.exists(), "{flags:?}");
    }
    assert_refused(
        &benchmark(&file, &["--examples", "1", "--max-renders", "0"]),
        "patchback: benchmark refused: patchback.fit.InvalidSearchBound: ",
    );
    for threshold in ["-1", "nan", "inf"] {
        let output = benchmark(
            &out,
            &["--examples", "1", "--improvement-threshold", threshold],
        );
        assert_eq!(output.status.code(), Some(2), "{threshold}");
        assert!(!out.exists());
    }
}

/// Pass 2. The verb's draw bound at both edges (`draws >= 4 × count` refuses before a draw): a
/// holdout of one clip whose fourth draw is the first admitted one is drawn, and its clip is that
/// fourth output; one whose first four draws are diagnostic examples and whose fifth would be
/// admitted is refused, so an off-by-one bound (`>`) does not pass unseen.
#[test]
fn the_draw_bound_admits_exactly_four_draws_a_clip() {
    let pipeline = Pipeline::new();
    let kinds = |seed: u64| -> Vec<bool> {
        let mut stream = SplitMix64::new(seed ^ HOLDOUT_DOMAIN);
        (0..5)
            .map(|_| {
                let draw = PatchDraw::from_seed(stream.next_u64());
                matches!(pipeline.run(&draw), Ok(Rendering::Sample(_)))
            })
            .collect()
    };
    let mut fourth = None;
    let mut fifth = None;
    for seed in 0..8192_u64 {
        let admitted = kinds(seed);
        if fourth.is_none() && admitted[..4] == [false, false, false, true] {
            fourth = Some(seed);
        }
        if fifth.is_none() && admitted == [false, false, false, false, true] {
            fifth = Some(seed);
        }
        if fourth.is_some() && fifth.is_some() {
            break;
        }
    }
    let (fourth, fifth) = (fourth.unwrap(), fifth.unwrap());
    let flags = |seed: &str| {
        [
            "--examples",
            "1",
            "--seed",
            seed,
            "--max-renders",
            "1",
            "--max-restarts",
            "1",
        ]
        .map(str::to_owned)
    };
    let dir = scratch("draw-bound");
    let out = dir.join("fourth");
    let seed = fourth.to_string();
    let output = benchmark(&out, &flags(&seed).each_ref().map(String::as_str));
    assert_eq!(
        output.status.code(),
        Some(0),
        "seed {fourth}: {}",
        text(&output.stderr)
    );
    let mut stream = SplitMix64::new(fourth ^ HOLDOUT_DOMAIN);
    let outputs: Vec<u64> = (0..5).map(|_| stream.next_u64()).collect();
    let clips = clip_lines(&out);
    assert_eq!(
        (clips[0].patch_seed, clips[0].fit_seed),
        (outputs[3], outputs[4])
    );

    let out = dir.join("fifth");
    let seed = fifth.to_string();
    assert_refused(
        &benchmark(&out, &flags(&seed).each_ref().map(String::as_str)),
        "patchback: benchmark refused: holdout exhausted: 4 patch draws gave 0 of the 1 clips",
    );
    assert!(!out.exists());
}

/// Pass 2. Every other run of the suite has `--max-restarts 1`, so only restart 0 runs, it starts
/// at the default point, and the fit seed `clips.txt` records changes no output: a verb that
/// passed any other seed to `fit` would pass them all. Here two restarts run (a threshold no
/// sweep reaches ends restart 0 after its sweep of 16 renders; the 17th is restart 1's start)
/// and clip 0's record must be the library fit seeded with its recorded fit seed, bit for bit.
///
/// Da8-46: the holdout stream is `SplitMix64::new(--seed ^ HOLDOUT_DOMAIN)`, so a fit seeded
/// with `--seed` no longer starts restart 1 at clip 0's own patch, and at most seeds restart 1's
/// one render loses to restart 0 whatever its seed, so the record shows no seed. `--seed 83` is
/// the first seed where it does (a search from 0 while re-pinning; 122 and 200 are the next):
/// restart 1 of the recorded fit seed runs and its start is the best, evaluation 17. The case
/// asserts that both plausible wrong wirings, a fit seeded `--seed` (83) or `FitConfig`'s seed
/// 0 left in place, record a different best loss, evaluation count or render, so it is red on
/// either.
#[test]
fn the_fit_is_seeded_with_the_recorded_fit_seed() {
    use patchback_fit::{FitBounds, FitConfig, fit};
    let dir = scratch("fit-seed");
    let out = dir.join("bench");
    let output = benchmark(
        &out,
        &[
            "--examples",
            "1",
            "--seed",
            "83",
            "--max-renders",
            "17",
            "--max-restarts",
            "2",
            "--improvement-threshold",
            "1000000000",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let clip = &clip_lines(&out)[0];
    let (draw, fit_seed) = documented_holdout(83, 1).unwrap().remove(0);
    assert_eq!(
        (clip.patch_seed, clip.fit_seed),
        (draw.patch_seed, fit_seed)
    );

    let topology = Topology::subtractive_mono();
    let reference = reference(&draw, 256);
    let seeded = |seed: u64| {
        fit(
            &reference,
            &draw.notes(),
            &topology,
            &FitConfig {
                bounds: FitBounds {
                    max_renders: 17,
                    max_wall_time_ms: 60_000,
                    max_restarts: 2,
                    improvement_threshold: 1e9,
                },
                seed,
                render: RenderConfig::default(),
            },
        )
        .unwrap()
    };
    let expected = seeded(fit_seed);
    assert_eq!(expected.restarts, 2, "restart 1 ran");
    assert_eq!(
        clip.raw[5..].join(" "),
        format!(
            "{} {} {} {}",
            expected.run.best_loss().unwrap(),
            expected.run.evaluations(),
            expected.restarts,
            expected.run.stop_reason().unwrap().name()
        )
    );
    let fitted = read_wav(&out.join("examples/0/fit.wav")).unwrap();
    assert_eq!(fitted.samples(), expected.render.samples());

    for wrong_seed in [83, 0] {
        let wrong = seeded(wrong_seed);
        assert!(
            (
                wrong.run.best_loss(),
                wrong.run.evaluations(),
                wrong.render.samples()
            ) != (
                expected.run.best_loss(),
                expected.run.evaluations(),
                expected.render.samples()
            ),
            "a fit seeded {wrong_seed} records what the recorded fit seed does"
        );
    }
}
