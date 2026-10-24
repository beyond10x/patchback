//! The numeric obligations of story:bounded-search-fit (DESIGN.md § 9.2, § 12.1, § 12.3, § 2
//! invariant 5) on references the engine renders from known patches: the retained best loss never
//! rises along the log, render evaluations never exceed `max_renders`, the same seed and inputs
//! give the same run, a wall-time bound stops the run, every stop reason is reachable and agrees
//! with the counters, and the fit of a reference one parameter away from the default patch ends
//! below the default's loss (the Milestone A demo, DESIGN.md § 13 A, § 15).
//!
//! Notes are short (0.25 s) and budgets small, so each fit costs tens of renders.

use std::time::Instant;

use patchback_audio::AudioClip;
use patchback_core::parameter::Topology;
use patchback_core::patch::{NodeKind, compile, validate, write_patch};
use patchback_core::prng::{SplitMix64, Xoshiro256StarStar};
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_dsp::{RenderConfig, render};
use patchback_fit::{
    EvaluationOutcome, FitBounds, FitConfig, FitError, FitOutcome, FitState, INITIAL_STEP,
    StopReason, compare, default_point, fit, patch_at, restart_point,
};

/// One note of MIDI 45 at velocity 0.8, 0.25 s long.
fn notes_at(sample_rate_hz: u32) -> NoteSequence {
    NoteSequence {
        format_version: 1,
        sample_rate_hz,
        patch_id: "7f0c0000-0000-4000-8000-000000000911".to_owned(),
        note_estimation_id: None,
        notes: vec![NoteEvent {
            start_sample: 0,
            end_sample: 12_000,
            midi_note: 45,
            velocity: 0.8,
        }],
    }
}

fn notes() -> NoteSequence {
    notes_at(48_000)
}

/// The clip the engine renders from the normalized point `point`.
fn rendered(point: &[f64]) -> AudioClip {
    let topology = Topology::subtractive_mono();
    let patch = patch_at(&topology, point).unwrap();
    let plan = compile(&validate(&patch).unwrap());
    let samples = render(&plan, &notes(), &RenderConfig::default())
        .unwrap()
        .samples;
    AudioClip::from_mono(samples).unwrap()
}

/// The index of the parameter at `path` in the normalized vector.
fn index(path: &str) -> usize {
    Topology::subtractive_mono().contract(path).unwrap().index
}

/// The reference of the demo: the default patch with the filter cutoff at `p = 0.25` instead of
/// the default 0.5 (about 179 Hz instead of 800 Hz).
fn one_parameter_away() -> AudioClip {
    let mut point = default_point(&Topology::subtractive_mono());
    point[index("filter_cutoff")] = 0.25;
    rendered(&point)
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

fn run(reference: &AudioClip, config: &FitConfig) -> FitOutcome {
    fit(reference, &notes(), &Topology::subtractive_mono(), config).expect("the fit completes")
}

/// What every completed fit holds, whatever stopped it: the log has one entry per counted
/// evaluation, numbered from 1; the best loss after each never rises and is the minimum of the
/// valid losses so far; the counters respect every bound; the stop reason agrees with them.
fn assert_consistent(outcome: &FitOutcome, config: &FitConfig) {
    let run = &outcome.run;
    let bounds = &config.bounds;
    assert_eq!(run.state(), FitState::Completed);
    assert_eq!(run.evaluations() as usize, outcome.log.len());
    assert!(run.evaluations() <= bounds.max_renders);
    assert!(i64::from(outcome.restarts) <= bounds.max_restarts);
    assert!(outcome.restarts >= 1);
    let mut lowest: Option<f64> = None;
    for (n, evaluation) in outcome.log.iter().enumerate() {
        assert_eq!(evaluation.sequence, n as u64 + 1);
        match evaluation.outcome {
            EvaluationOutcome::Rejected => {
                assert!(evaluation.total.is_none() && evaluation.components.is_none());
                assert!(evaluation.rejection.is_some());
            }
            _ => {
                let total = evaluation.total.unwrap();
                assert!(total.is_finite() && total >= 0.0);
                let improves = lowest.is_none_or(|best| total < best);
                let expected = match (lowest, improves) {
                    (None, _) => EvaluationOutcome::FirstRetained,
                    (Some(_), true) => EvaluationOutcome::Improved,
                    (Some(_), false) => EvaluationOutcome::NotImproved,
                };
                assert_eq!(evaluation.outcome, expected, "evaluation {}", n + 1);
                if improves {
                    lowest = Some(total);
                }
            }
        }
        assert_eq!(evaluation.best_after, lowest, "evaluation {}", n + 1);
        if n > 0 {
            let before = outcome.log[n - 1].best_after;
            if let (Some(before), Some(after)) = (before, evaluation.best_after) {
                assert!(
                    after <= before,
                    "the best loss rose at evaluation {}",
                    n + 1
                );
            }
        }
        assert_eq!(evaluation.normalized.len(), 8);
        assert!(
            evaluation
                .normalized
                .iter()
                .all(|p| (0.0..=1.0).contains(p))
        );
    }
    assert_eq!(run.best_loss(), lowest);
    assert_eq!(run.initial_loss(), outcome.log[0].total);
    let best = run.best().copied().unwrap();
    let retained = &outcome.log[best as usize - 1];
    assert_eq!(retained.total, run.best_loss());
    assert_eq!(retained.normalized, outcome.normalized);
    assert_eq!(
        write_patch(&outcome.patch).unwrap(),
        write_patch(&patch_at(&Topology::subtractive_mono(), &outcome.normalized).unwrap())
            .unwrap()
    );
    match run.stop_reason().unwrap() {
        StopReason::RenderBudgetExhausted => {
            assert_eq!(run.evaluations(), bounds.max_renders);
        }
        StopReason::TimeBudgetExhausted => {
            assert!(run.wall_time_ms().unwrap() >= bounds.max_wall_time_ms as u64);
        }
        StopReason::RestartsExhausted => {
            assert_eq!(i64::from(outcome.restarts), bounds.max_restarts);
        }
        StopReason::ImprovementBelowThreshold => {
            assert!(
                outcome.restarts >= 2,
                "only a restart after the first ends the search"
            );
            let start_of_last = outcome
                .log
                .iter()
                .position(|e| e.restart == outcome.restarts - 1)
                .unwrap();
            let before = if start_of_last == 0 {
                None
            } else {
                outcome.log[start_of_last - 1].best_after
            };
            let gain = match (before, run.best_loss()) {
                (Some(before), Some(after)) => before - after,
                (None, Some(_)) => f64::INFINITY,
                _ => 0.0,
            };
            assert!(gain < bounds.improvement_threshold, "gain {gain}");
        }
    }
}

/// The default patch is the fixed chain with every parameter at its contract default.
#[test]
fn the_default_patch_is_the_fixed_chain_at_the_contract_defaults() {
    let topology = Topology::subtractive_mono();
    let point = default_point(&topology);
    assert_eq!(point, vec![0.5; 8]);
    let patch = patch_at(&topology, &point).unwrap();
    let validated = validate(&patch).expect("the default patch validates");
    assert_eq!(validated.patch().topology_id, "subtractive-mono/1");
    for contract in topology.contracts() {
        let value = patch
            .nodes
            .iter()
            .flat_map(|node| node.parameters.iter())
            .find(|parameter| parameter.name == contract.path)
            .unwrap()
            .value;
        assert_eq!(value, contract.default_physical(), "{}", contract.path);
    }
    let kinds: Vec<NodeKind> = patch.nodes.iter().map(|node| node.kind).collect();
    assert_eq!(kinds, NodeKind::MVP);
    // The ends of every range stay inside it, so every point of the unit cube validates.
    for corner in [0.0, 1.0] {
        validate(&patch_at(&topology, &[corner; 8]).unwrap()).expect("a corner validates");
    }
}

/// DESIGN.md § 13 A, § 15: the fit of a reference one parameter away from the default patch ends
/// with a lower loss than the default's, the first evaluation. Before and after are printed for
/// the report.
#[test]
fn the_fit_of_a_reference_one_parameter_away_lowers_the_loss() {
    let reference = one_parameter_away();
    let config = config(40, 1, 0.0001);
    let started = Instant::now();
    let outcome = run(&reference, &config);
    assert_consistent(&outcome, &config);
    let before = outcome.run.initial_loss().unwrap();
    let after = outcome.run.best_loss().unwrap();
    eprintln!(
        "demo: initial_loss {before} best_loss {after} evaluations {} stop {:?} wall {:?}",
        outcome.run.evaluations(),
        outcome.run.stop_reason(),
        started.elapsed()
    );
    assert_eq!(
        outcome.log[0].normalized,
        vec![0.5; 8],
        "it starts at the default"
    );
    assert!(after < before, "{after} is not below {before}");
    assert!(
        after < 0.5 * before,
        "{after} is not below half of {before}"
    );
}

/// The best render is the clip that was compared: comparing it with the reference again gives
/// the retained loss exactly.
#[test]
fn the_best_render_reproduces_the_best_loss() {
    let reference = one_parameter_away();
    let outcome = run(&reference, &config(6, 1, 0.0001));
    assert_eq!(outcome.render.frame_count(), reference.frame_count());
    let again = compare(&reference, &outcome.render).unwrap();
    assert_eq!(Some(again.total), outcome.run.best_loss());
    assert_eq!(Some(&again.components), outcome.run.best_components());
}

/// § 2 invariant 5: the same seed and inputs give the same log and the same patch, bit for bit;
/// another seed draws other restart points.
#[test]
fn the_same_seed_and_inputs_give_the_same_run() {
    let reference = one_parameter_away();
    let mut config = config(30, 3, 10.0);
    config.seed = 7;
    let first = run(&reference, &config);
    let second = run(&reference, &config);
    assert_consistent(&first, &config);
    assert_eq!(first.log, second.log);
    assert_eq!(first.restarts, second.restarts);
    assert_eq!(
        write_patch(&first.patch).unwrap(),
        write_patch(&second.patch).unwrap()
    );
    assert_eq!(first.render.samples(), second.render.samples());
    assert!(first.restarts >= 2, "the run reached a seeded restart");
    config.seed = 8;
    let other = run(&reference, &config);
    let restart_start = |outcome: &FitOutcome| {
        outcome
            .log
            .iter()
            .find(|e| e.restart == 1)
            .unwrap()
            .normalized
            .clone()
    };
    assert_ne!(restart_start(&first), restart_start(&other));
}

/// The documented start of restart `restart >= 1` (Da6-24), computed here independently of the
/// crate: the `restart`-th output of `SplitMix64::new(seed)` (numbered from 1) seeds
/// `Xoshiro256StarStar`, whose first eight `next_f64` draws are the point, in index order.
fn seeded_start(seed: u64, restart: u32) -> Vec<f64> {
    let mut mix = SplitMix64::new(seed);
    let mut restart_seed = 0;
    for _ in 0..restart {
        restart_seed = mix.next_u64();
    }
    let mut prng = Xoshiro256StarStar::new(restart_seed);
    (0..8).map(|_| prng.next_f64()).collect()
}

/// Restart 0 starts at the default patch; restart `r >= 1` at eight draws of
/// `Xoshiro256StarStar::new(<the r-th output of SplitMix64::new(seed)>)`, in index order.
#[test]
fn restarts_start_at_the_default_then_at_seeded_points() {
    let reference = one_parameter_away();
    let mut config = config(60, 3, 10.0);
    config.seed = 0x5eed;
    let outcome = run(&reference, &config);
    assert_consistent(&outcome, &config);
    assert!(outcome.restarts >= 2, "the run reached a seeded restart");
    for restart in 0..outcome.restarts {
        let first = outcome.log.iter().find(|e| e.restart == restart).unwrap();
        let expected = if restart == 0 {
            vec![0.5; 8]
        } else {
            seeded_start(config.seed, restart)
        };
        assert_eq!(first.normalized, expected, "restart {restart}");
    }
}

/// Da6-24 (adversary F6): under `seed ^ r`, seed 1 restart 2 and seed 2 restart 1 shared one
/// start. Seeds 1 and 2 at restarts 1 and 2 now start from four pairwise distinct points,
/// [`restart_point`] is the documented derivation, and the search starts restart 1 there.
#[test]
fn nearby_seeds_and_restarts_start_from_distinct_points() {
    let mut points = Vec::new();
    for seed in [1, 2] {
        for restart in [1, 2] {
            let point = restart_point(seed, restart, 8);
            assert_eq!(
                point,
                seeded_start(seed, restart),
                "seed {seed} restart {restart}"
            );
            points.push(((seed, restart), point));
        }
    }
    for (i, (a, p)) in points.iter().enumerate() {
        for (b, q) in &points[i + 1..] {
            assert_ne!(
                p, q,
                "seed and restart {a:?} and {b:?} start at the same point"
            );
        }
    }
    // Restart 0 is one start and one improving sweep of 16 (threshold 10 ends it), so the 18th
    // render is restart 1's start.
    let reference = one_parameter_away();
    for seed in [1, 2] {
        let mut config = config(18, 3, 10.0);
        config.seed = seed;
        let outcome = run(&reference, &config);
        assert_consistent(&outcome, &config);
        let start = outcome
            .log
            .iter()
            .find(|e| e.restart == 1)
            .expect("restart 1 began");
        assert_eq!(start.normalized, seeded_start(seed, 1), "seed {seed}");
    }
}

/// Coordinate search: each candidate after a restart's start differs from the point it was
/// searched from in one parameter, by the step (0.25 at first, halving), clamped to `[0, 1]`.
#[test]
fn candidates_move_one_parameter_by_the_step() {
    let reference = one_parameter_away();
    let outcome = run(&reference, &config(20, 1, 0.0001));
    let start = &outcome.log[0].normalized;
    let second = &outcome.log[1].normalized;
    let changed: Vec<usize> = (0..8).filter(|&i| start[i] != second[i]).collect();
    assert_eq!(changed, vec![0], "the first move is the first parameter");
    assert_eq!(second[0], start[0] + INITIAL_STEP);
    for pair in outcome.log.windows(2) {
        let moved = (0..8)
            .filter(|&i| pair[0].normalized[i] != pair[1].normalized[i])
            .count();
        assert!(moved <= 2, "a candidate moves one parameter from its base");
    }
}

/// § 12.3: a wall-time bound of 1 ms stops the run with TimeBudgetExhausted after its first
/// render, which every started run gets.
#[test]
fn a_one_millisecond_wall_time_bound_stops_with_time_budget_exhausted() {
    let reference = one_parameter_away();
    let mut config = config(2_000, 4, 0.0001);
    config.bounds.max_wall_time_ms = 1;
    let outcome = run(&reference, &config);
    assert_consistent(&outcome, &config);
    assert_eq!(
        outcome.run.stop_reason(),
        Some(StopReason::TimeBudgetExhausted)
    );
    assert_eq!(outcome.run.evaluations(), 1);
}

/// Every stop reason is reachable, and each agrees with the counters (`assert_consistent`).
#[test]
fn every_stop_reason_is_reachable_and_agrees_with_the_counters() {
    let reference = one_parameter_away();
    let mut timed = config(2_000, 4, 0.0001);
    timed.bounds.max_wall_time_ms = 1;
    let cases = [
        (config(3, 4, 0.0001), StopReason::RenderBudgetExhausted),
        (timed, StopReason::TimeBudgetExhausted),
        (config(500, 1, 10.0), StopReason::RestartsExhausted),
        (config(500, 4, 10.0), StopReason::ImprovementBelowThreshold),
    ];
    for (config, reason) in cases {
        let outcome = run(&reference, &config);
        assert_eq!(outcome.run.stop_reason(), Some(reason));
        assert_consistent(&outcome, &config);
    }
}

/// § 9.2 step 5: a run whose every render is refused (a note sequence at 44.1 kHz, which the
/// renderer refuses) counts each as rejected, retains nothing and fails with NoValidCandidate.
#[test]
fn a_run_whose_every_render_is_refused_fails_with_no_valid_candidate() {
    let reference = one_parameter_away();
    let error = fit(
        &reference,
        &notes_at(44_100),
        &Topology::subtractive_mono(),
        &config(100, 2, 0.0001),
    )
    .unwrap_err();
    assert_eq!(error, FitError::NoValidCandidate);
    assert_eq!(error.domain_error(), "patchback.fit.NoValidCandidate");
}
