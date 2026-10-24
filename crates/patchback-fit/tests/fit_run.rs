//! The acceptance scenarios of story:bounded-search-fit (`spec/scenarios/bounded-search-fit-*.yaml`)
//! as tests of [`FitRun`], the state of `spec/domains/fit.yaml` `FitRun` with its commands, and of
//! [`fit`]'s refusals. The unknown-reference scenario is held through the binary
//! (`crates/patchback-cli/tests/fit.rs`), which knows clips and note sequences by path; its
//! unknown topology is held here.

use patchback_audio::AudioClip;
use patchback_core::parameter::{ParameterContract, Topology};
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_fit::{
    EvaluationOutcome, FailureReason, FitBounds, FitConfig, FitError, FitRun, FitState,
    LossComponents, SearchBound, StopReason, fit,
};

fn components(total: f64) -> LossComponents {
    LossComponents {
        log_spectral: total,
        envelope: total,
        transient: total,
        band_energy: total,
    }
}

/// The bounds of the scenarios: 2 000 renders, 600 000 ms, 4 restarts.
fn scenario_bounds() -> FitBounds {
    FitBounds {
        max_renders: 2_000,
        max_wall_time_ms: 600_000,
        max_restarts: 4,
        improvement_threshold: 0.0001,
    }
}

fn running(bounds: FitBounds) -> FitRun<u32> {
    let run = FitRun::start(bounds).expect("the bounds are positive");
    assert_eq!(run.state(), FitState::Running);
    assert_eq!(run.evaluations(), 0);
    assert_eq!(run.best_loss(), None);
    run
}

/// bounded-search-fit-retains-best: the first valid candidate is retained, a strictly lower loss
/// replaces it, a higher and an equal loss do not, and an invalid candidate is counted without
/// replacing it.
#[test]
fn retains_best_only_for_a_strictly_lower_loss() {
    let mut run = running(scenario_bounds());
    assert_eq!(
        run.evaluate(1, components(0.645), 0.645),
        Ok(EvaluationOutcome::FirstRetained)
    );
    assert_eq!(run.initial_loss(), Some(0.645));
    assert_eq!(
        run.evaluate(2, components(0.43), 0.43),
        Ok(EvaluationOutcome::Improved)
    );
    assert_eq!(run.best(), Some(&2));
    assert_eq!(
        run.evaluate(3, components(0.745), 0.745),
        Ok(EvaluationOutcome::NotImproved)
    );
    assert_eq!(
        run.evaluate(4, components(0.43), 0.43),
        Ok(EvaluationOutcome::NotImproved)
    );
    assert_eq!(
        run.best(),
        Some(&2),
        "an equal loss does not replace the best"
    );
    assert_eq!(run.reject(), Ok(EvaluationOutcome::Rejected));
    assert_eq!(
        run.best(),
        Some(&2),
        "an invalid candidate does not replace it"
    );
    assert_eq!(
        run.evaluate(6, components(0.415), 0.415),
        Ok(EvaluationOutcome::Improved)
    );
    assert_eq!(run.evaluations(), 6);
    assert_eq!(run.initial_loss(), Some(0.645));
    assert_eq!(run.best_loss(), Some(0.415));
    assert_eq!(run.best(), Some(&6));
    assert_eq!(run.best_components(), Some(&components(0.415)));
    assert_eq!(run.state(), FitState::Running);
}

/// DESIGN.md § 9.2 step 5: a numerically invalid loss offered as a valid candidate is counted as
/// rejected and never replaces the best.
#[test]
fn a_non_finite_loss_is_counted_as_rejected_and_never_retained() {
    let mut run = running(scenario_bounds());
    assert_eq!(
        run.evaluate(1, components(f64::NAN), f64::NAN),
        Ok(EvaluationOutcome::Rejected)
    );
    assert_eq!(run.best_loss(), None);
    assert_eq!(run.initial_loss(), None);
    run.evaluate(2, components(0.5), 0.5).unwrap();
    for (id, total, parts) in [
        (3, f64::NEG_INFINITY, components(0.1)),
        (4, f64::INFINITY, components(0.1)),
        (5, 0.1, components(f64::NAN)),
        (6, -0.1, components(0.1)),
    ] {
        assert_eq!(
            run.evaluate(id, parts, total),
            Ok(EvaluationOutcome::Rejected),
            "{total}"
        );
    }
    assert_eq!(run.best(), Some(&2));
    assert_eq!(run.best_loss(), Some(0.5));
    assert_eq!(run.evaluations(), 6);
}

/// bounded-search-fit-stops-at-render-bound: a run bounded to two renders refuses a third, stops
/// with RenderBudgetExhausted and then refuses further work.
#[test]
fn stops_at_the_render_bound_and_refuses_work_once_completed() {
    let mut run = running(FitBounds {
        max_renders: 2,
        max_restarts: 1,
        ..scenario_bounds()
    });
    assert_eq!(
        run.evaluate(1, components(0.645), 0.645),
        Ok(EvaluationOutcome::FirstRetained)
    );
    assert_eq!(
        run.evaluate(2, components(0.685), 0.685),
        Ok(EvaluationOutcome::NotImproved)
    );
    assert_eq!(
        run.evaluate(3, components(0.43), 0.43),
        Err(FitError::RenderBudgetExhausted)
    );
    assert_eq!(run.reject(), Err(FitError::RenderBudgetExhausted));
    assert_eq!(run.evaluations(), 2, "a refused candidate is not counted");
    assert_eq!(run.best_loss(), Some(0.645), "nor retained");
    assert_eq!(run.complete(StopReason::RenderBudgetExhausted, 12), Ok(()));
    assert_eq!(run.state(), FitState::Completed);
    assert_eq!(run.stop_reason(), Some(StopReason::RenderBudgetExhausted));
    assert_eq!(run.wall_time_ms(), Some(12));
    let not_running = Err(FitError::FitNotRunning {
        state: FitState::Completed,
    });
    assert_eq!(run.evaluate(4, components(0.43), 0.43), not_running);
    assert_eq!(run.reject(), not_running);
    assert_eq!(run.evaluations(), 2);
}

/// bounded-search-fit-ended-run-refused: completing or failing a run that already completed is
/// refused with FitNotRunning naming its state, and the run stays Completed.
#[test]
fn an_ended_run_refuses_a_second_ending() {
    let mut run = running(scenario_bounds());
    run.evaluate(1, components(0.645), 0.645).unwrap();
    assert_eq!(
        run.complete(StopReason::ImprovementBelowThreshold, 40),
        Ok(())
    );
    let not_running = Err(FitError::FitNotRunning {
        state: FitState::Completed,
    });
    assert_eq!(
        run.complete(StopReason::ImprovementBelowThreshold, 41),
        not_running
    );
    assert_eq!(run.fail(FailureReason::RendererFault, 42), not_running);
    assert_eq!(run.state(), FitState::Completed);
    assert_eq!(
        run.stop_reason(),
        Some(StopReason::ImprovementBelowThreshold)
    );
    assert_eq!(run.failure_reason(), None);
    assert_eq!(run.wall_time_ms(), Some(40));
}

/// CompleteFit `no-valid-candidate`: a running run without a valid candidate cannot complete; it
/// fails, and a failed run refuses every further command.
#[test]
fn a_run_without_a_valid_candidate_cannot_complete_and_fails() {
    let mut run = running(scenario_bounds());
    run.reject().unwrap();
    assert_eq!(
        run.complete(StopReason::RestartsExhausted, 5),
        Err(FitError::NoValidCandidate)
    );
    assert_eq!(run.state(), FitState::Running);
    assert_eq!(run.fail(FailureReason::NoValidCandidate, 6), Ok(()));
    assert_eq!(run.state(), FitState::Failed);
    assert_eq!(run.failure_reason(), Some(FailureReason::NoValidCandidate));
    assert_eq!(run.stop_reason(), None);
    let not_running = Some(FitError::FitNotRunning {
        state: FitState::Failed,
    });
    assert_eq!(
        run.complete(StopReason::RestartsExhausted, 7).err(),
        not_running
    );
    assert_eq!(
        run.fail(FailureReason::NoValidCandidate, 7).err(),
        not_running
    );
    assert_eq!(run.evaluate(9, components(0.1), 0.1).err(), not_running);
    assert_eq!(run.reject().err(), not_running);
    assert_eq!(run.evaluations(), 1);
}

/// bounded-search-fit-invalid-bounds: no render budget, no wall-time budget or no restart budget
/// is refused with InvalidSearchBound naming the bound, in the model's order; no run starts.
#[test]
fn invalid_bounds_are_refused_and_name_the_bound() {
    let cases = [
        (
            FitBounds {
                max_renders: 0,
                ..scenario_bounds()
            },
            SearchBound::RenderEvaluations,
            0,
        ),
        (
            FitBounds {
                max_wall_time_ms: 0,
                ..scenario_bounds()
            },
            SearchBound::WallTime,
            0,
        ),
        (
            FitBounds {
                max_restarts: -1,
                ..scenario_bounds()
            },
            SearchBound::Restarts,
            -1,
        ),
        (
            FitBounds {
                max_renders: -5,
                max_wall_time_ms: -6,
                max_restarts: 0,
                improvement_threshold: 0.0001,
            },
            SearchBound::RenderEvaluations,
            -5,
        ),
        (
            FitBounds {
                max_wall_time_ms: -6,
                max_restarts: 0,
                ..scenario_bounds()
            },
            SearchBound::WallTime,
            -6,
        ),
    ];
    let reference = AudioClip::from_mono(vec![0.0; 4_800]).unwrap();
    let notes = one_note(4_800);
    for (bounds, bound, value) in cases {
        let expected = FitError::InvalidSearchBound { bound, value };
        assert_eq!(bounds.check(), Err(expected.clone()));
        assert_eq!(FitRun::<u32>::start(bounds).err(), Some(expected.clone()));
        let config = FitConfig {
            bounds,
            ..FitConfig::default()
        };
        // The silent reference would be refused too; the bound is answered first.
        assert_eq!(
            fit(&reference, &notes, &Topology::subtractive_mono(), &config).err(),
            Some(expected)
        );
    }
}

fn one_note(end_sample: u64) -> NoteSequence {
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

/// bounded-search-fit-silence-and-pitch, the silent reference: refused with ReferenceSilent, no
/// run starts.
#[test]
fn a_silent_reference_is_refused_with_reference_silent() {
    let reference = AudioClip::from_mono(vec![0.0; 24_000]).unwrap();
    let error = fit(
        &reference,
        &one_note(12_000),
        &Topology::subtractive_mono(),
        &FitConfig::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, FitError::ReferenceSilent { .. }),
        "{error:?}"
    );
    assert_eq!(error.domain_error(), "patchback.fit.ReferenceSilent");
}

/// bounded-search-fit-silence-and-pitch, the pitch-ambiguous reference: white noise has no
/// fundamental; refused with PitchAmbiguous, no run starts.
#[test]
fn an_unpitched_reference_is_refused_with_pitch_ambiguous() {
    let mut state: u64 = 0x5eed;
    let noise: Vec<f32> = (0..24_000)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (0.8 * (((state >> 11) as f64 / (1u64 << 53) as f64) - 0.5)) as f32
        })
        .collect();
    let reference = AudioClip::from_mono(noise).unwrap();
    let error = fit(
        &reference,
        &one_note(12_000),
        &Topology::subtractive_mono(),
        &FitConfig::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, FitError::PitchAmbiguous { .. }),
        "{error:?}"
    );
    assert_eq!(error.domain_error(), "patchback.fit.PitchAmbiguous");
}

/// bounded-search-fit-unknown-references, the unknown topology: a topology the renderer does not
/// know is refused with TopologyNotFound before the reference is analysed; no run starts.
#[test]
fn an_unknown_topology_is_refused_with_topology_not_found() {
    let contracts: Vec<ParameterContract> = Topology::subtractive_mono().contracts().to_vec();
    let topology = Topology::new("subtractive-mono/9", contracts).unwrap();
    let silent = AudioClip::from_mono(vec![0.0; 24_000]).unwrap();
    let error = fit(&silent, &one_note(12_000), &topology, &FitConfig::default()).unwrap_err();
    assert_eq!(
        error,
        FitError::TopologyNotFound {
            topology: "subtractive-mono/9".to_owned()
        }
    );
    assert_eq!(error.domain_error(), "patchback.fit.TopologyNotFound");
}
