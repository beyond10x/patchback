//! Adversary cases for story:bounded-search-fit (wave a6, pass 1): the search's numeric
//! obligations and its documented rule, measured on references the engine renders.
//!
//! - The story's wall-time obligation (§ 12.3), as Da6-22 amends it: no render starts once the
//!   bound is reached, except the run's first candidate.
//! - The documented coordinate-search rule (search.rs module doc): a restart whose start cannot
//!   be improved spends exactly one start plus four sweeps of 16 candidates (steps 0.25, 0.125,
//!   0.0625, 0.03125) and ends.
//! - The `restarts` counter ("the restarts that began") against the log.
//! - § 2 invariant 5: the block size and the seed (for restart 0) do not change the run.
//! - D-a6-6: every point the search can visit validates, so `RendererFault` is unreachable.
//! - The Milestone A demo off the step grid, with another parameter and another seed.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use patchback_analysis::{AnalysisConfig, analyze_clip};
use patchback_audio::AudioClip;
use patchback_core::parameter::Topology;
use patchback_core::patch::{compile, validate};
use patchback_core::prng::{SplitMix64, Xoshiro256StarStar};
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_dsp::{RenderConfig, render};
use patchback_fit::{
    EvaluationOutcome, FitBounds, FitConfig, FitOutcome, FitState, StopReason, compare,
    default_point, fit, patch_at, restart_point,
};

/// One note of MIDI 45 at velocity 0.8, `end_sample` frames long, at 48 kHz.
fn notes(end_sample: u64) -> NoteSequence {
    NoteSequence {
        format_version: 1,
        sample_rate_hz: 48_000,
        patch_id: "7f0c0000-0000-4000-8000-000000000911".to_owned(),
        note_estimation_id: None,
        notes: vec![NoteEvent {
            start_sample: 0,
            end_sample,
            midi_note: 45,
            velocity: 0.8,
        }],
    }
}

/// The clip the engine renders from the normalized point `point` for `notes`.
fn rendered(point: &[f64], notes: &NoteSequence) -> AudioClip {
    let patch = patch_at(&Topology::subtractive_mono(), point).unwrap();
    let plan = compile(&validate(&patch).unwrap());
    let samples = render(&plan, notes, &RenderConfig::default())
        .unwrap()
        .samples;
    AudioClip::from_mono(samples).unwrap()
}

fn index(path: &str) -> usize {
    Topology::subtractive_mono().contract(path).unwrap().index
}

fn config(max_renders: i64, max_restarts: i64, improvement_threshold: f64) -> FitConfig {
    FitConfig {
        bounds: FitBounds {
            max_renders,
            max_wall_time_ms: 600_000,
            max_restarts,
            improvement_threshold,
        },
        ..FitConfig::default()
    }
}

fn run(reference: &AudioClip, notes: &NoteSequence, config: &FitConfig) -> FitOutcome {
    fit(reference, notes, &Topology::subtractive_mono(), config).expect("the fit completes")
}

/// The story's wall-time obligation as Da6-22 states it: no render starts once the wall time
/// since `fit` was entered has reached the bound, except the run's first candidate (the default
/// point of restart 0), which is always evaluated, so a time bound stops a run instead of failing
/// it. A bound of 1 ms on a 0.25 s and a 0.9 s reference: one evaluation, one restart,
/// `TimeBudgetExhausted`, the whole run's wall time at least the bound, the default point's patch.
#[test]
fn a_one_millisecond_bound_stops_after_the_first_candidate_with_its_patch() {
    let topology = Topology::subtractive_mono();
    let default = default_point(&topology);
    for end_sample in [12_000, 43_200] {
        let notes = notes(end_sample);
        let reference = rendered(&default, &notes);
        let mut config = config(2_000, 4, 0.0001);
        config.bounds.max_wall_time_ms = 1;
        let outcome = run(&reference, &notes, &config);
        let wall = outcome.run.wall_time_ms().unwrap();
        eprintln!(
            "note {end_sample} frames: bound 1 ms, wall_time_ms {wall}, evaluations {}, stop {:?}",
            outcome.run.evaluations(),
            outcome.run.stop_reason()
        );
        assert_eq!(outcome.run.evaluations(), 1, "note {end_sample} frames");
        assert_eq!(outcome.restarts, 1, "note {end_sample} frames");
        assert_eq!(
            outcome.run.stop_reason(),
            Some(StopReason::TimeBudgetExhausted),
            "note {end_sample} frames"
        );
        assert!(wall >= 1, "note {end_sample} frames: wall time {wall} ms");
        assert_eq!(outcome.normalized, default, "note {end_sample} frames");
        assert_eq!(
            outcome.patch,
            patch_at(&topology, &default).unwrap(),
            "note {end_sample} frames"
        );
    }
}

/// The documented rule on a reference the default patch renders exactly: the start's loss is 0,
/// so no candidate is strictly lower; each sweep tries 8 parameters x 2 directions (no clamp
/// from 0.5 at steps <= 0.25), the step halves after each, and the restart ends below MIN_STEP:
/// 1 + 4 x 16 = 65 evaluations, steps 0.25, 0.125, 0.0625 and 0.03125, then RestartsExhausted
/// with one restart.
#[test]
fn a_restart_that_cannot_improve_spends_one_start_and_four_sweeps() {
    let notes = notes(12_000);
    let start = default_point(&Topology::subtractive_mono());
    let reference = rendered(&start, &notes);
    let config = config(1_000, 1, 0.0001);
    let outcome = run(&reference, &notes, &config);
    assert_eq!(outcome.run.state(), FitState::Completed);
    assert_eq!(outcome.run.best_loss(), Some(0.0));
    assert_eq!(outcome.run.best(), Some(&1));
    assert_eq!(outcome.run.evaluations(), 65);
    assert_eq!(
        outcome.run.stop_reason(),
        Some(StopReason::RestartsExhausted)
    );
    let steps: BTreeSet<u64> = outcome.log[1..]
        .iter()
        .map(|e| {
            let moved: Vec<f64> = (0..8)
                .map(|i| (e.normalized[i] - start[i]).abs())
                .filter(|d| *d != 0.0)
                .collect();
            assert_eq!(moved.len(), 1, "evaluation {}", e.sequence);
            (moved[0] * 1024.0) as u64
        })
        .collect();
    assert_eq!(
        steps,
        BTreeSet::from([32, 64, 128, 256]),
        "steps in 1/1024 units"
    );
}

/// `FitOutcome::restarts` is "the restarts that began", and `patchback fit` prints it as
/// `restarts`. When the render budget runs out exactly where restart 0 ends, restart 1 renders
/// nothing; the count must not name a restart the log holds no evaluation of.
#[test]
fn the_restart_count_names_only_restarts_that_evaluated_a_candidate() {
    let notes = notes(12_000);
    let reference = rendered(&default_point(&Topology::subtractive_mono()), &notes);
    let config = config(65, 2, 0.0001);
    let outcome = run(&reference, &notes, &config);
    assert_eq!(
        outcome.run.stop_reason(),
        Some(StopReason::RenderBudgetExhausted)
    );
    assert_eq!(outcome.run.evaluations(), 65);
    let in_log: BTreeSet<u32> = outcome.log.iter().map(|e| e.restart).collect();
    assert_eq!(
        outcome.restarts as usize,
        in_log.len(),
        "restarts {} but the log holds evaluations of restarts {in_log:?}",
        outcome.restarts
    );
}

/// § 2 invariant 5 and the renderer's contract ("the block size changes how the work is cut and
/// never the samples"): block sizes 256, 7 and 4096 give the same log; a run of one restart is
/// the same whatever the seed (restart 0 starts at the default patch).
#[test]
fn neither_the_block_size_nor_the_seed_of_one_restart_changes_the_run() {
    let notes = notes(12_000);
    let mut point = default_point(&Topology::subtractive_mono());
    point[index("filter_cutoff")] = 0.25;
    let reference = rendered(&point, &notes);
    let base = config(12, 1, 0.0001);
    let first = run(&reference, &notes, &base);
    for (block, seed) in [(7, 0), (4_096, 0), (256, 99)] {
        let mut other = base;
        other.render.block_size_frames = block;
        other.seed = seed;
        let again = run(&reference, &notes, &other);
        assert_eq!(first.log, again.log, "block {block} seed {seed}");
        assert_eq!(first.patch, again.patch, "block {block} seed {seed}");
    }
}

/// D-a6-6: a patch validation refuses is a RendererFault, said to be unreachable. Every point the
/// search can visit (seeded restart points, the step grid of 1/32 from the default, the
/// corners per parameter) validates.
#[test]
fn every_point_the_search_can_visit_validates() {
    let topology = Topology::subtractive_mono();
    let mut points: Vec<Vec<f64>> = Vec::new();
    // Pass 2: the seeded starts the search uses (Da6-24, `restart_point`), not `seed ^ r`.
    for seed in 0..64u64 {
        for restart in 1..8u32 {
            points.push(restart_point(seed, restart, 8));
        }
    }
    for i in 0..8 {
        for k in 0..=32 {
            let mut point = default_point(&topology);
            point[i] = f64::from(k) / 32.0;
            points.push(point);
        }
    }
    for mask in 0..256u32 {
        points.push(
            (0..8)
                .map(|i| if mask & (1 << i) != 0 { 1.0 } else { 0.0 })
                .collect(),
        );
    }
    assert_eq!(points.len(), 448 + 264 + 256);
    for point in points {
        let patch = patch_at(&topology, &point).unwrap();
        assert!(validate(&patch).is_ok(), "{point:?}");
    }
}

/// The Milestone A demo (§ 13 A) off the step grid: resonance_q at 0.65 instead of 0.5 (not a
/// multiple of a step from the default), seed 3, two restarts. The fit ends below the default's
/// loss; before and after are printed for the report.
#[test]
fn the_demo_holds_off_the_step_grid_with_another_parameter_and_seed() {
    let notes = notes(12_000);
    let mut point = default_point(&Topology::subtractive_mono());
    point[index("resonance_q")] = 0.65;
    let reference = rendered(&point, &notes);
    let mut config = config(40, 2, 0.0001);
    config.seed = 3;
    let outcome = run(&reference, &notes, &config);
    let before = outcome.run.initial_loss().unwrap();
    let after = outcome.run.best_loss().unwrap();
    eprintln!(
        "demo resonance_q 0.65 seed 3: initial_loss {before} best_loss {after} evaluations {} \
         restarts {} stop {:?} best point {:?}",
        outcome.run.evaluations(),
        outcome.restarts,
        outcome.run.stop_reason(),
        outcome.normalized
    );
    assert!(after < before, "{after} is not below {before}");
}

/// Pass 2, Da6-24 against `patchback_core::prng` directly: restart `r >= 1` of seed `s` is
/// eight draws of `Xoshiro256StarStar::new(x)`, `x` the `r`-th output of `SplitMix64::new(s)`
/// (restart 1 takes the first). The first output of `SplitMix64::new(0)` is the published
/// vector 0xE220A8397B1DCDAF, so seed 0 restart 1 is pinned to a literal seed. Seeds 1 and 2 at
/// restarts 1 and 2 give four distinct points; seed 0 restart 1 differs from restart 2; a shorter
/// point is a prefix of a longer one (draw `i` is index `i`).
#[test]
fn restart_point_is_the_rth_splitmix_output_seeding_xoshiro() {
    let draws = |x: u64, len: usize| -> Vec<f64> {
        let mut prng = Xoshiro256StarStar::new(x);
        (0..len).map(|_| prng.next_f64()).collect()
    };
    assert_eq!(SplitMix64::new(0).next_u64(), 0xE220_A839_7B1D_CDAF);
    assert_eq!(restart_point(0, 1, 8), draws(0xE220_A839_7B1D_CDAF, 8));
    for seed in [0, 1, 2, 0x5eed, u64::MAX] {
        let mut mix = SplitMix64::new(seed);
        for restart in 1..=4u32 {
            let x = mix.next_u64();
            assert_eq!(
                restart_point(seed, restart, 8),
                draws(x, 8),
                "seed {seed} restart {restart}"
            );
        }
    }
    let four = [
        restart_point(1, 1, 8),
        restart_point(1, 2, 8),
        restart_point(2, 1, 8),
        restart_point(2, 2, 8),
    ];
    for i in 0..4 {
        for j in i + 1..4 {
            assert_ne!(four[i], four[j], "points {i} and {j}");
        }
    }
    assert_ne!(restart_point(0, 1, 8), restart_point(0, 2, 8));
    assert_eq!(restart_point(9, 3, 3)[..], restart_point(9, 3, 8)[..3]);
    assert!(restart_point(9, 3, 0).is_empty());
}

/// Pass 2, Da6-22: with a bound the second render cannot meet, the log is one entry, the run's
/// first candidate: sequence 1, restart 0, the default point, `FirstRetained`, retained as the
/// best; `restarts` is 1 whatever `max_restarts` allows.
#[test]
fn a_bound_the_second_render_cannot_meet_logs_only_the_first_candidate() {
    let notes = notes(12_000);
    let default = default_point(&Topology::subtractive_mono());
    let mut point = default.clone();
    point[index("filter_cutoff")] = 0.25;
    let reference = rendered(&point, &notes);
    for max_restarts in [1, 4] {
        let mut config = config(2_000, max_restarts, 0.0001);
        config.bounds.max_wall_time_ms = 1;
        let outcome = run(&reference, &notes, &config);
        assert_eq!(outcome.log.len(), 1, "max_restarts {max_restarts}");
        let only = &outcome.log[0];
        assert_eq!(only.sequence, 1);
        assert_eq!(only.restart, 0);
        assert_eq!(only.normalized, default);
        assert_eq!(only.outcome, EvaluationOutcome::FirstRetained);
        assert_eq!(outcome.run.best(), Some(&1));
        assert_eq!(outcome.restarts, 1, "max_restarts {max_restarts}");
        assert_eq!(
            outcome.run.stop_reason(),
            Some(StopReason::TimeBudgetExhausted)
        );
        assert!(outcome.run.best_loss().unwrap() > 0.0);
    }
}

/// Pass 2, Da6-29 (re-pinned in correction round 2) as `search.rs` `# Stopping` and `fit.rs`'s
/// `wall_time_ms` row state it: the recorded wall time may exceed the bound by at most the
/// reference analysis and one evaluation. The clock starts before the reference analysis, and
/// with a 1 ms bound the analysis and the first evaluation both run after the bound has passed.
/// One evaluation is measured here as the search performs it (patch, validate, compile, render,
/// pad, compare), after a warm-up, longest of three; the reference analysis once. The slack for
/// timing noise is one whole evaluation and 50 ms (D-a7-14). The adversary's first slack, an
/// eighth of an evaluation and 10 ms, failed on a loaded CI runner (release 0.5.0, run
/// 40141574944 attempt 1: wall 1668 ms, analysis 428 ms, one evaluation 1057 ms, allowed 1627 ms),
/// where the run's own analysis and evaluation took longer than the ones measured here. The test
/// still fails when the wall time exceeds the analysis and two evaluations.
#[test]
fn the_recorded_wall_time_exceeds_its_bound_by_at_most_the_analysis_and_one_evaluation() {
    let notes = notes(43_200);
    let topology = Topology::subtractive_mono();
    let default = default_point(&topology);
    let mut point = default.clone();
    point[index("filter_cutoff")] = 0.25;
    let reference = rendered(&point, &notes);
    let evaluate = || {
        let started = Instant::now();
        let patch = patch_at(&topology, &default).unwrap();
        let plan = compile(&validate(&patch).unwrap());
        let mut samples = render(&plan, &notes, &RenderConfig::default())
            .unwrap()
            .samples;
        samples.resize(reference.frame_count(), 0.0);
        let clip = AudioClip::from_mono(samples).unwrap();
        compare(&reference, &clip).unwrap();
        started.elapsed()
    };
    evaluate();
    let mut config = config(2_000, 4, 0.0001);
    config.bounds.max_wall_time_ms = 1;
    let outcome = run(&reference, &notes, &config);
    let wall = outcome.run.wall_time_ms().unwrap();
    let longest = (0..3).map(|_| evaluate()).max().unwrap_or(Duration::ZERO);
    let analysis = {
        let started = Instant::now();
        analyze_clip(&reference, &AnalysisConfig::default()).unwrap();
        started.elapsed()
    };
    // Timing noise between the run's analysis and first evaluation and those measured here: one
    // evaluation and 50 ms (D-a7-14).
    let longest_ms = longest.as_millis() as u64 + 1;
    let analysis_ms = analysis.as_millis() as u64 + 1;
    let allowed = analysis_ms + 2 * longest_ms + 50;
    eprintln!(
        "bound 1 ms: wall_time_ms {wall}, evaluations {}, longest evaluation {longest:?}, \
         reference analysis {analysis:?}, allowed overshoot {allowed} ms",
        outcome.run.evaluations()
    );
    assert_eq!(outcome.run.evaluations(), 1);
    assert!(
        wall - 1 <= allowed,
        "wall_time_ms {wall} exceeds the 1 ms bound by {} ms, more than the reference analysis \
         ({analysis_ms} ms) and two evaluations ({longest_ms} ms each), {allowed} ms with the \
         slack",
        wall - 1
    );
}
