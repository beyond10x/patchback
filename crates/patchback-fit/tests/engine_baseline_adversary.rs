//! Adversary cases for story:engine-baseline (wave a8, pass 1): the `Benchmark` value of
//! `spec/domains/fit.yaml` against what its scenarios and DESIGN.md § 12.2 leave implicit: a
//! refused command changes nothing, the verdict at the 20 % boundary, a result that is no
//! `Decimal`, the medians at their edges, and the default arm's refusals.

use patchback_core::parameter::Topology;
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_dsp::RenderConfig;
use patchback_fit::{
    BaselineResult, Benchmark, BenchmarkKind, BenchmarkState, DefaultArmError, FitError,
    TargetVerdict, default_arm, median, median_count, reduction_percent,
};

const DECLARED: &str = "subtractive-mono/1";

fn result(default: f64, search: f64, reduction: f64) -> BaselineResult {
    BaselineResult {
        default_median_loss: default,
        default_search_median_loss: search,
        median_loss_reduction_percent: reduction,
        default_search_median_evaluations: 10,
        wall_time_ms: 5,
        listening_examples: vec!["examples/0/reference.wav".to_owned()],
    }
}

/// `wrong-state` and `kind-mismatch` are refusals: the benchmark after the refused call equals
/// the benchmark before it, every field included.
#[test]
fn a_refused_record_changes_nothing() {
    let mut completed =
        Benchmark::start(BenchmarkKind::EngineBaseline, DECLARED, 1, 3, None).unwrap();
    completed
        .record_baseline_result(result(0.8, 0.7, 12.5))
        .unwrap();
    let before = completed.clone();
    assert_eq!(
        completed.record_baseline_result(result(0.9, 0.1, 88.0)),
        Err(FitError::BenchmarkStateConflict {
            state: BenchmarkState::Completed
        })
    );
    assert_eq!(completed, before);

    let mut ml = Benchmark::start(BenchmarkKind::MlInit, DECLARED, 1, 3, Some("w")).unwrap();
    let before = ml.clone();
    assert_eq!(
        ml.record_baseline_result(result(0.9, 0.1, 88.0)),
        Err(FitError::BenchmarkKindMismatch {
            kind: BenchmarkKind::MlInit
        })
    );
    assert_eq!(ml, before);
}

/// `StartBenchmark` sets `weight_artifact_id: input.weight_artifact_id` for either kind, and the
/// entity invariant is `example_count > 0` with no upper bound.
#[test]
fn a_start_keeps_every_input_at_its_extremes() {
    let bench = Benchmark::start(
        BenchmarkKind::EngineBaseline,
        DECLARED,
        u64::MAX,
        i64::MAX,
        Some("artifact"),
    )
    .unwrap();
    assert_eq!(bench.seed(), u64::MAX);
    assert_eq!(bench.example_count(), i64::MAX);
    assert_eq!(bench.weight_artifact_id(), Some("artifact"));
    assert_eq!(
        Benchmark::start(BenchmarkKind::EngineBaseline, DECLARED, 0, i64::MIN, None),
        Err(FitError::InvalidHoldoutSize {
            example_count: i64::MIN
        })
    );
    assert_eq!(
        Benchmark::start(BenchmarkKind::MlInit, DECLARED, 0, 1, Some("")).map(|b| b.state()),
        Ok(BenchmarkState::Running),
        "an empty artifact id is defined"
    );
}

/// The exact sign of `4 * default - 5 * search` for two positive doubles: `>= 0` exactly when
/// the true reduction `(default - search) / default` of those two doubles is at least 20 %.
fn exactly_at_least_twenty_percent(default: f64, search: f64) -> bool {
    fn parts(x: f64) -> (u128, i32) {
        let bits = x.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i32;
        let fraction = u128::from(bits & ((1 << 52) - 1));
        if exponent == 0 {
            (fraction, -1074)
        } else {
            (fraction | (1 << 52), exponent - 1075)
        }
    }
    let (md, ed) = parts(default);
    let (ms, es) = parts(search);
    let low = ed.min(es);
    let left = 4 * (md << (ed - low) as u32);
    let right = 5 * (ms << (es - low) as u32);
    left >= right
}

/// DESIGN.md § 12.2: the target is met at a reduction of "at least 20 %". Da8-35: the verdict is
/// decided on the recorded `f64` reduction, so the record and the verdict always agree. Over the
/// 6000 pairs closest to the boundary (default `k / 1000`, search the nearest double to
/// `0.8 × default` and its two neighbours): `reduction_percent` is the documented formula
/// `(default - search) / default * 100.0` bit for bit; the verdict of [`TargetVerdict::of`] and
/// the one `record_baseline_result` returns are `Met` exactly when that `f64` is `>= 20.0`, and
/// the benchmark records that same `f64`. How many pairs fall on the other side of 20 than exact
/// rational arithmetic is printed, not asserted: that difference is documented, not corrected.
#[test]
fn the_verdict_is_decided_on_the_recorded_f64_reduction() {
    let mut differ_from_exact = 0;
    let mut pairs = 0;
    for k in 1..=2000_u32 {
        let default = f64::from(k) / 1000.0;
        let near = default * 0.8;
        for search in [near.next_down(), near, near.next_up()] {
            pairs += 1;
            let computed = reduction_percent(default, search);
            let formula = (default - search) / default * 100.0;
            assert_eq!(
                computed.to_bits(),
                formula.to_bits(),
                "default {default:?}, search {search:?}"
            );
            let expected = if computed >= 20.0 {
                TargetVerdict::Met
            } else {
                TargetVerdict::NotMet
            };
            assert_eq!(
                TargetVerdict::of(computed),
                expected,
                "default {default:?}, search {search:?}: reduction {computed:?}"
            );
            let mut bench =
                Benchmark::start(BenchmarkKind::EngineBaseline, DECLARED, 1, 1, None).unwrap();
            assert_eq!(
                bench.record_baseline_result(result(default, search, computed)),
                Ok(expected),
                "default {default:?}, search {search:?}: reduction {computed:?}"
            );
            assert_eq!(
                bench.median_loss_reduction_percent().map(f64::to_bits),
                Some(computed.to_bits())
            );
            if (expected == TargetVerdict::Met) != exactly_at_least_twenty_percent(default, search)
            {
                differ_from_exact += 1;
            }
        }
    }
    assert_eq!(pairs, 6000);
    eprintln!("{differ_from_exact} of {pairs} pairs differ from exact arithmetic (Da8-35)");
}

/// Da8-36: `record_baseline_result` adds no refusal to the model's four outcomes. A reduction
/// that is no number (the NaN `reduction_percent` gives for a default median of 0, which the CLI
/// refuses before it records) is recorded as given, the verdict is `NotMet` and the benchmark is
/// Completed. What the model should answer is the specification owner's.
#[test]
fn a_reduction_that_is_no_number_is_recorded_and_completes_the_benchmark_not_met() {
    let reduction = reduction_percent(0.0, 0.0);
    assert!(reduction.is_nan());
    let mut bench = Benchmark::start(BenchmarkKind::EngineBaseline, DECLARED, 1, 1, None).unwrap();
    assert_eq!(
        bench.record_baseline_result(result(0.0, 0.0, reduction)),
        Ok(TargetVerdict::NotMet)
    );
    assert_eq!(bench.state(), BenchmarkState::Completed);
    assert_eq!(bench.target_verdict(), Some(TargetVerdict::NotMet));
    assert!(
        bench
            .median_loss_reduction_percent()
            .is_some_and(f64::is_nan),
        "{:?}",
        bench.median_loss_reduction_percent()
    );
}

/// The lower median at its edges: ties, an even count of unsorted values, negative zero against
/// zero, the integer extremes.
#[test]
fn the_lower_median_at_its_edges() {
    assert_eq!(median(&[0.5, 0.5]), Some(0.5));
    assert_eq!(median(&[0.9, 0.1, 0.7, 0.3]), Some(0.3));
    assert_eq!(median(&[0.9, 0.1, 0.7, 0.3, 0.5, 0.2]), Some(0.3));
    assert_eq!(median(&[3.0, 1.0]), Some(1.0), "not the mean 2.0");
    assert_eq!(median_count(&[i64::MAX, i64::MIN]), Some(i64::MIN));
    assert_eq!(median_count(&[5, 1, 5, 1]), Some(1));
    assert_eq!(median_count(&[3, 1, 2, 4, 5]), Some(3));
}

fn notes(sample_rate_hz: u32) -> NoteSequence {
    NoteSequence {
        format_version: patchback_core::score::FORMAT_VERSION,
        sample_rate_hz,
        patch_id: "7f0c0000-0000-4000-8000-000000000a82".to_owned(),
        note_estimation_id: None,
        notes: vec![NoteEvent {
            start_sample: 0,
            end_sample: 9_600,
            midi_note: 57,
            velocity: 1.0,
        }],
    }
}

/// `DefaultArmError` variants a library caller reaches: a refused render (block size 0, a
/// sequence at another rate) and an empty reference.
#[test]
fn the_default_arm_refuses_what_it_cannot_score() {
    let topology = Topology::subtractive_mono();
    let reference = patchback_audio::AudioClip::from_mono(vec![0.25; 4_800]).unwrap();
    assert!(matches!(
        default_arm(
            &reference,
            &notes(48_000),
            &topology,
            &RenderConfig {
                block_size_frames: 0
            }
        ),
        Err(DefaultArmError::Render(_))
    ));
    assert!(matches!(
        default_arm(
            &reference,
            &notes(44_100),
            &topology,
            &RenderConfig::default()
        ),
        Err(DefaultArmError::Render(_))
    ));
    let error = default_arm(
        &reference,
        &notes(44_100),
        &topology,
        &RenderConfig::default(),
    )
    .unwrap_err();
    assert!(error.domain_error().is_some(), "{error}");
}

/// The 6000 pairs of `the_verdict_is_decided_on_the_recorded_f64_reduction` near the boundary.
fn boundary_pairs() -> Vec<(f64, f64)> {
    (1..=2000_u32)
        .flat_map(|k| {
            let default = f64::from(k) / 1000.0;
            let near = default * 0.8;
            [near.next_down(), near, near.next_up()].map(|search| (default, search))
        })
        .collect()
}

/// Pass 2, Da8-35: the re-pinned case can fail on the mutants it names. Its pairs hold a
/// reduction of exactly `20.0` (so `of` or `record_baseline_result` comparing with `>` would give
/// `NotMet` where `>= 20.0` is `Met`), and every reordering of the formula a refactor might write
/// differs from `(default - search) / default * 100.0` in at least one pair's bits.
#[test]
fn the_boundary_pairs_tell_every_mutant_of_the_verdict_apart() {
    let pairs = boundary_pairs();
    let formula = |d: f64, s: f64| (d - s) / d * 100.0;
    let at_twenty = pairs
        .iter()
        .filter(|(d, s)| reduction_percent(*d, *s) == 20.0)
        .count();
    assert!(at_twenty > 0, "no pair computes exactly 20.0");
    type Formula = fn(f64, f64) -> f64;
    let reorderings: [(&str, Formula); 4] = [
        ("100 * (d - s) / d", |d, s| 100.0 * (d - s) / d),
        ("(1 - s / d) * 100", |d, s| (1.0 - s / d) * 100.0),
        ("(d - s) * 100 / d", |d, s| (d - s) * 100.0 / d),
        ("100 - s / d * 100", |d, s| 100.0 - s / d * 100.0),
    ];
    for (name, mutant) in reorderings {
        assert!(
            pairs
                .iter()
                .any(|(d, s)| mutant(*d, *s).to_bits() != formula(*d, *s).to_bits()),
            "{name} agrees with the formula on every pair"
        );
    }
}

/// Pass 2, Da8-36: a default median of `+0.0` or `-0.0` from the library, with a search median
/// above it. The reductions are `-inf` and `+inf`; each is recorded as computed and the benchmark
/// is Completed. As the method's precondition documents, `+inf` is `Met`: a negative-zero default
/// "meets" the target with no reduction defined. The CLI's refusal (`default_median == 0.0`)
/// catches both zeros, since `-0.0 == 0.0`.
#[test]
fn a_zero_default_median_of_either_sign_is_recorded_as_computed() {
    for (default, expected_reduction, verdict) in [
        (0.0_f64, f64::NEG_INFINITY, TargetVerdict::NotMet),
        (-0.0_f64, f64::INFINITY, TargetVerdict::Met),
    ] {
        let reduction = reduction_percent(default, 0.25);
        assert_eq!(reduction, expected_reduction, "default {default:?}");
        let mut bench =
            Benchmark::start(BenchmarkKind::EngineBaseline, DECLARED, 1, 1, None).unwrap();
        assert_eq!(
            bench.record_baseline_result(result(default, 0.25, reduction)),
            Ok(verdict)
        );
        assert_eq!(bench.state(), BenchmarkState::Completed);
        assert_eq!(
            bench.median_loss_reduction_percent().map(f64::to_bits),
            Some(reduction.to_bits())
        );
        assert_eq!(
            bench.default_median_loss().map(f64::to_bits),
            Some(default.to_bits())
        );
        assert!(
            default == 0.0,
            "the CLI's exact refusal catches {default:?}"
        );
    }
}
