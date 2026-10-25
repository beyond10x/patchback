//! Adversary cases for story:fit-run-artifacts (wave a7, pass 2): the limiter warning of
//! Da7-15 against a render re-made from the written patch, the boundaries of the trace refusals
//! of Da7-16, every prefix of a real run's log through the stricter parser, and the patterns the
//! parser still accepts, pinned as they are (`characterise_…`).

use std::fs;
use std::path::{Path, PathBuf};

use patchback_audio::{AudioClip, read_wav};
use patchback_core::digest::sha256;
use patchback_core::parameter::Topology;
use patchback_core::patch::{compile, format_decimal, validate};
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_dsp::{RENDER_TAIL_FRAMES, RenderConfig, render, render_frame_count};
use patchback_fit::{
    Evaluation, EvaluationOutcome, FitBounds, FitConfig, LossComponents, Rejection, RunMeta,
    StopReason, TraceReason, default_point, fit, parse_trace, patch_at, write_results, write_trace,
};

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-fit-results-adversary2")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn meta() -> RunMeta {
    RunMeta {
        seed: 3,
        block_size_frames: 256,
        modelspec_version: "0.5.0".to_owned(),
        reference_sha256: sha256(b"reference"),
        notes_sha256: sha256(b"notes"),
    }
}

fn logged(
    sequence: u64,
    outcome: EvaluationOutcome,
    total: Option<f64>,
    best_after: Option<f64>,
) -> Evaluation {
    Evaluation {
        sequence,
        restart: 0,
        outcome,
        normalized: vec![0.5; Topology::subtractive_mono().contracts().len()],
        components: total.map(|total| LossComponents {
            log_spectral: total,
            envelope: 0.0,
            transient: 0.0,
            band_energy: 0.0,
        }),
        total,
        best_after,
        rejection: total.is_none().then_some(Rejection::NonFiniteLoss),
    }
}

fn written(log: &[Evaluation]) -> String {
    write_trace(
        &Topology::subtractive_mono(),
        log,
        StopReason::RestartsExhausted,
    )
    .unwrap()
}

/// `text` with field `field` of line `line` (1-based) replaced by `value`.
fn with_field(text: &str, line: usize, field: usize, value: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let mut fields: Vec<&str> = lines[line - 1].split(' ').collect();
    fields[field] = value;
    lines[line - 1] = fields.join(" ");
    lines.join("\n") + "\n"
}

/// The field index of `<best>` on a valid line and on a rejected one.
const VALID_BEST: usize = 11;
const REJECTED_BEST: usize = 7;

/// A small log: two rejected candidates, a first retained best of 0.5, a not-improved and a
/// rejected line holding it, an improvement to 0.25. Lines 3 to 8 of its trace.
fn small_log() -> Vec<Evaluation> {
    use EvaluationOutcome::{FirstRetained, Improved, NotImproved, Rejected};
    vec![
        logged(1, Rejected, None, None),
        logged(2, Rejected, None, None),
        logged(3, FirstRetained, Some(0.5), Some(0.5)),
        logged(4, NotImproved, Some(0.9), Some(0.5)),
        logged(5, Rejected, None, Some(0.5)),
        logged(6, Improved, Some(0.25), Some(0.25)),
    ]
}

/// Da7-16 boundaries of `BestLossRose`: a best one ulp above the previous is refused with both
/// values; an equal best, `-0.0` after `0.0` and `0.0` after `-0.0` are no rise; a rise on a
/// rejected line is refused like one on a valid line.
#[test]
fn adversary2_the_best_loss_rise_boundaries() {
    let text = written(&small_log());
    let above = f64::from_bits(0.5_f64.to_bits() + 1);
    assert!(above > 0.5);
    let error = parse_trace(&with_field(&text, 6, VALID_BEST, &format_decimal(above))).unwrap_err();
    assert_eq!(error.line, Some(6), "{error}");
    assert_eq!(
        error.reason,
        TraceReason::BestLossRose {
            previous: 0.5,
            best: above
        }
    );
    assert!(
        error.to_string().contains(&format_decimal(above)),
        "{error}"
    );

    let rose_on_rejected = with_field(&text, 7, REJECTED_BEST, "0.75");
    let error = parse_trace(&rose_on_rejected).unwrap_err();
    assert_eq!(error.line, Some(7), "{error}");
    assert_eq!(
        error.reason,
        TraceReason::BestLossRose {
            previous: 0.5,
            best: 0.75
        }
    );

    let mut zeros = small_log();
    for evaluation in &mut zeros[2..] {
        evaluation.best_after = Some(0.0);
        if evaluation.total.is_some() {
            evaluation.total = Some(0.0);
            evaluation.components = Some(LossComponents {
                log_spectral: 0.0,
                envelope: 0.0,
                transient: 0.0,
                band_energy: 0.0,
            });
        }
    }
    let zeros = written(&zeros);
    let negative_after_positive = with_field(&zeros, 6, VALID_BEST, "-0.0");
    let trace = parse_trace(&negative_after_positive).unwrap();
    assert_eq!(
        trace.records[3].best_after.map(f64::to_bits),
        Some((-0.0_f64).to_bits())
    );
    let positive_after_negative = with_field(&negative_after_positive, 7, REJECTED_BEST, "0.0");
    assert!(parse_trace(&positive_after_negative).is_ok());
}

/// Da7-16: any number of `-` bests may come first, from the very first line, and the first
/// numeric best after them is accepted whatever its value.
#[test]
fn adversary2_dashes_from_the_first_line_then_any_first_number() {
    use EvaluationOutcome::{FirstRetained, Rejected};
    let log = [
        logged(1, Rejected, None, None),
        logged(2, Rejected, None, None),
        logged(3, Rejected, None, None),
        logged(4, FirstRetained, Some(f64::MAX), Some(f64::MAX)),
    ];
    let trace = parse_trace(&written(&log)).unwrap();
    let bests: Vec<Option<f64>> = trace.records.iter().map(|r| r.best_after).collect();
    assert_eq!(bests, [None, None, None, Some(f64::MAX)]);
}

/// A non-finite or overflowing best is a malformed number, answered before any rise is
/// considered; the closing line carries no best at all.
#[test]
fn adversary2_a_non_finite_best_is_a_number_fault_and_completed_has_no_best() {
    let text = written(&small_log());
    for bad in ["inf", "NaN", "1e309", "-inf"] {
        let error = parse_trace(&with_field(&text, 6, VALID_BEST, bad)).unwrap_err();
        assert_eq!(error.line, Some(6), "{bad}: {error}");
        assert_eq!(
            error.reason,
            TraceReason::MalformedNumber {
                token: bad.to_owned()
            },
            "{bad}"
        );
    }
    let closing = text.lines().count();
    let with_best = text.replacen(
        "completed RestartsExhausted",
        "completed RestartsExhausted 0.9",
        1,
    );
    let error = parse_trace(&with_best).unwrap_err();
    assert_eq!(error.line, Some(closing), "{error}");
    assert!(
        matches!(error.reason, TraceReason::MalformedRecord { .. }),
        "{error:?}"
    );
}

/// The reference of `tests/results.rs`: the default patch with the cutoff at 0.25, two notes.
fn quiet_notes() -> NoteSequence {
    NoteSequence {
        format_version: 1,
        sample_rate_hz: 48_000,
        patch_id: "7f0c0000-0000-4000-8000-000000000a02".to_owned(),
        note_estimation_id: None,
        notes: vec![
            NoteEvent {
                start_sample: 0,
                end_sample: 6_000,
                midi_note: 45,
                velocity: 0.8,
            },
            NoteEvent {
                start_sample: 7_000,
                end_sample: 12_000,
                midi_note: 45,
                velocity: 0.5,
            },
        ],
    }
}

/// Da7-16 on real logs: every prefix of a real 20-render run's log (each the log of the same run
/// with that render budget) is written and parsed back by the stricter parser, the one-evaluation
/// run and a run whose last evaluation improved among them.
#[test]
fn adversary2_every_prefix_of_a_real_log_round_trips() {
    let topology = Topology::subtractive_mono();
    let mut point = default_point(&topology);
    point[topology.contract("filter_cutoff").unwrap().index] = 0.25;
    let plan = compile(&validate(&patch_at(&topology, &point).unwrap()).unwrap());
    let reference = AudioClip::from_mono(
        render(&plan, &quiet_notes(), &RenderConfig::default())
            .unwrap()
            .samples,
    )
    .unwrap();
    let outcome = fit(
        &reference,
        &quiet_notes(),
        &topology,
        &FitConfig {
            bounds: FitBounds {
                max_renders: 20,
                max_wall_time_ms: 600_000,
                max_restarts: 2,
                improvement_threshold: 1e-12,
            },
            seed: 42,
            render: RenderConfig::default(),
        },
    )
    .unwrap();
    let log = &outcome.log;
    assert_eq!(log.len(), 20, "precondition: the budget was spent");
    let mut improved_last = 0;
    for end in 1..=log.len() {
        let prefix = &log[..end];
        let text = write_trace(&topology, prefix, StopReason::RenderBudgetExhausted)
            .unwrap_or_else(|error| panic!("prefix {end}: {error}"));
        let trace = parse_trace(&text).unwrap();
        assert_eq!(trace.records.len(), end);
        for (record, evaluation) in trace.records.iter().zip(prefix) {
            assert_eq!(record.best_after, evaluation.best_after, "prefix {end}");
            assert_eq!(record.total, evaluation.total, "prefix {end}");
        }
        if prefix[end - 1].outcome == EvaluationOutcome::Improved {
            improved_last += 1;
        }
    }
    assert_eq!(log[0].outcome, EvaluationOutcome::FirstRetained);
    assert!(improved_last > 0, "precondition: some prefix ends improved");
}

/// A hot patch: output gain at 0 dB, drive at 24 dB, resonance and sustain at their tops.
fn hot_point(topology: &Topology) -> Vec<f64> {
    let mut point = default_point(topology);
    for path in ["output_gain", "drive", "resonance_q", "sustain"] {
        point[topology.contract(path).unwrap().index] = 1.0;
    }
    point
}

/// A quiet note, then after the reference ends a loud one.
fn late_loud_notes() -> NoteSequence {
    NoteSequence {
        format_version: 1,
        sample_rate_hz: 48_000,
        patch_id: "7f0c0000-0000-4000-8000-000000000a03".to_owned(),
        note_estimation_id: None,
        notes: vec![
            NoteEvent {
                start_sample: 0,
                end_sample: 12_000,
                midi_note: 45,
                velocity: 0.25,
            },
            NoteEvent {
                start_sample: 36_000,
                end_sample: 48_000,
                midi_note: 45,
                velocity: 1.0,
            },
        ],
    }
}

const REFERENCE_FRAMES: usize = 24_000;

/// Da7-15 against an independent render: a fit whose best render the limiter changed only after
/// the reference ends. The warning's `<n>` is the limited samples and `<m>` the frames of the
/// best patch's whole render, re-made here from the written `patch.patch` (the last note's end,
/// the release and the 4,800-frame tail), not the reference's length; `<n> <= <m>`; the report,
/// the results and the run agree; and `reconstructed.wav`, the reference's 24,000 frames, holds
/// no sample at the threshold (the documented gap: the run warns although the written clip was
/// never limited).
#[test]
fn adversary2_a_render_limited_only_after_the_reference_ends_still_warns() {
    let topology = Topology::subtractive_mono();
    let notes = late_loud_notes();
    let hot = compile(&validate(&patch_at(&topology, &hot_point(&topology)).unwrap()).unwrap());
    let full = render(&hot, &notes, &RenderConfig::default()).unwrap();
    assert!(
        full.report.limited,
        "precondition: the loud note is limited"
    );
    let mut samples = full.samples;
    samples.truncate(REFERENCE_FRAMES);
    assert!(
        samples.iter().all(|sample| sample.abs() < 1.0),
        "precondition: the quiet note is not limited"
    );
    let reference = AudioClip::from_mono(samples).unwrap();
    let outcome = fit(
        &reference,
        &notes,
        &topology,
        &FitConfig {
            bounds: FitBounds {
                max_renders: 20,
                max_wall_time_ms: 600_000,
                max_restarts: 1,
                improvement_threshold: 1e-9,
            },
            seed: 3,
            render: RenderConfig::default(),
        },
    )
    .unwrap();
    assert!(
        outcome.render_report.limited,
        "precondition: the best render is limited ({:?})",
        outcome.normalized
    );
    let out = scratch("late-limit");
    let results = write_results(&out, &outcome, &notes, &meta()).unwrap();

    let written = patchback_core::patch::load_patch_file(out.join("patch.patch")).unwrap();
    let plan = compile(&validate(written.patch()).unwrap());
    let again = render(&plan, &notes, &RenderConfig::default()).unwrap();
    let m = render_frame_count(&plan, &notes);
    assert_eq!(u64::try_from(again.samples.len()).unwrap(), m);
    assert!(m >= 48_000 + RENDER_TAIL_FRAMES, "{m}");
    assert_ne!(usize::try_from(m).unwrap(), REFERENCE_FRAMES);
    let n = again.report.limited_samples;
    assert!((1..=m).contains(&n), "{n} of {m}");
    assert_eq!(
        results.warnings,
        [format!("limiter engaged on {n} of {m} rendered samples")]
    );
    let report = fs::read_to_string(out.join("report.txt")).unwrap();
    let warning_lines: Vec<&str> = report
        .lines()
        .filter_map(|line| line.strip_prefix("warning "))
        .collect();
    assert_eq!(warning_lines, results.warnings);
    assert!(report.ends_with(&format!("warning {}\n", results.warnings[0])));

    let wav = read_wav(&out.join("reconstructed.wav")).unwrap();
    assert_eq!(wav.frame_count(), REFERENCE_FRAMES);
    let at_threshold = wav.samples().iter().filter(|s| s.abs() >= 1.0).count();
    assert_eq!(at_threshold, 0, "the written clip holds no limited sample");
}

/// The grammar (`trace.rs`): `<best>` is "the run's best loss after it, or `-` while no
/// candidate has been retained"; `TraceFrame.best_loss` is "absent until a valid candidate is
/// retained". A `first-retained` line retains a candidate, so its best is a number: a
/// `first-retained` line with `-`, followed by numeric bests, is a trace no run writes.
#[test]
fn adversary2_a_first_retained_line_without_a_best_is_refused() {
    let text = written(&small_log());
    let absent = with_field(&text, 5, VALID_BEST, "-");
    let parsed = parse_trace(&absent);
    assert!(
        parsed.is_err(),
        "accepted `first-retained` with best `-`: {:?}",
        parsed.map(|t| (t.records[2].outcome, t.records[2].best_after))
    );
}

/// The same rule from the other side: a numeric best on a line before any candidate was
/// retained (here the two `rejected` lines before the `first-retained` one, both 0.75, so no
/// other rule is broken) names a best loss no candidate scored.
#[test]
fn adversary2_a_numeric_best_before_any_retained_candidate_is_refused() {
    let text = written(&small_log());
    let early = with_field(
        &with_field(&text, 3, REJECTED_BEST, "0.75"),
        4,
        REJECTED_BEST,
        "0.75",
    );
    let parsed = parse_trace(&early);
    assert!(
        parsed.is_err(),
        "accepted best 0.75 on the rejected lines 1 and 2: {:?}",
        parsed.map(|t| (t.records[0].outcome, t.records[0].best_after))
    );
}

/// Still accepted (correction report, "Still accepted"): a restart number that skips and then
/// falls. No run writes it.
#[test]
fn characterise_a_restart_number_that_falls_or_skips_is_accepted() {
    let text = written(&small_log());
    let skipped = with_field(&text, 5, 3, "5");
    let fallen = with_field(&skipped, 6, 3, "1");
    let trace = parse_trace(&fallen).unwrap();
    let restarts: Vec<u32> = trace.records.iter().map(|r| r.restart).collect();
    assert_eq!(restarts, [0, 0, 5, 1, 0, 0]);
}

/// Still accepted: `first-retained` on a line that is not the first numeric best.
#[test]
fn characterise_first_retained_after_the_first_numeric_best_is_accepted() {
    let text = written(&small_log());
    let again = with_field(&text, 6, 4, "first-retained");
    let trace = parse_trace(&again).unwrap();
    assert_eq!(trace.records[3].outcome, EvaluationOutcome::FirstRetained);
    assert_eq!(trace.records[2].outcome, EvaluationOutcome::FirstRetained);
}

/// Still accepted: `improved` whose best is not its total.
#[test]
fn characterise_improved_with_a_best_other_than_its_total_is_accepted() {
    let text = written(&small_log());
    let apart = with_field(&text, 8, VALID_BEST, "0.375");
    let trace = parse_trace(&apart).unwrap();
    assert_eq!(trace.records[5].outcome, EvaluationOutcome::Improved);
    assert_eq!(trace.records[5].total, Some(0.25));
    assert_eq!(trace.records[5].best_after, Some(0.375));
}

/// Still accepted: `not-improved` or `rejected` whose best changed (fell; a rise is refused).
#[test]
fn characterise_not_improved_or_rejected_with_a_changed_best_is_accepted() {
    let text = written(&small_log());
    let not_improved = with_field(&text, 6, VALID_BEST, "0.4");
    let rejected = with_field(&not_improved, 7, REJECTED_BEST, "0.3");
    let trace = parse_trace(&rejected).unwrap();
    assert_eq!(trace.records[3].outcome, EvaluationOutcome::NotImproved);
    assert_eq!(trace.records[3].best_after, Some(0.4));
    assert_eq!(trace.records[4].outcome, EvaluationOutcome::Rejected);
    assert_eq!(trace.records[4].best_after, Some(0.3));
}
