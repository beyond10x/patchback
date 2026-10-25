//! Adversary cases for story:fit-run-artifacts (wave a7, pass 1): the trace grammar read from
//! its module documentation and `spec/domains/studio.yaml` `TraceFrame`, the decimal round trip,
//! the cleanup of a failed write, and the limiter warning of a real fit.

use std::fs;
use std::path::{Path, PathBuf};

use patchback_audio::{AudioClip, read_wav};
use patchback_core::digest::sha256;
use patchback_core::parameter::Topology;
use patchback_core::patch::{compile, validate};
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_dsp::{RenderConfig, render};
use patchback_fit::{
    Evaluation, EvaluationOutcome, FitBounds, FitConfig, LossComponents, Rejection, ResultsError,
    RunMeta, StopReason, default_point, fit, parse_trace, patch_at, write_results, write_trace,
};

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-fit-results-adversary")
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

fn valid(
    sequence: u64,
    outcome: EvaluationOutcome,
    components: [f64; 4],
    total: f64,
    best_after: f64,
    normalized: Vec<f64>,
) -> Evaluation {
    let [log_spectral, envelope, transient, band_energy] = components;
    Evaluation {
        sequence,
        restart: 0,
        outcome,
        normalized,
        components: Some(LossComponents {
            log_spectral,
            envelope,
            transient,
            band_energy,
        }),
        total: Some(total),
        best_after: Some(best_after),
        rejection: None,
    }
}

/// A hand-built log: a rejected candidate first, then two valid ones, with reals at the edges of
/// `f64` (the smallest subnormal, 1e-300, 0.1, 1/3, 17 significant digits, the largest finite,
/// negative zero).
fn extreme_log() -> Vec<Evaluation> {
    let paths = Topology::subtractive_mono().contracts().len();
    let mut values = vec![
        f64::from_bits(1),
        1e-300,
        0.1,
        1.0 / 3.0,
        0.123_456_789_012_345_67,
        f64::MAX,
        -0.0,
        f64::MIN_POSITIVE,
    ];
    values.resize(paths, 0.5);
    vec![
        Evaluation {
            sequence: 1,
            restart: 0,
            outcome: EvaluationOutcome::Rejected,
            normalized: values.clone(),
            components: None,
            total: None,
            best_after: None,
            rejection: Some(Rejection::NonFiniteLoss),
        },
        valid(
            2,
            EvaluationOutcome::FirstRetained,
            [f64::MAX, 1e-300, f64::from_bits(1), 1.0 / 3.0],
            0.1,
            0.1,
            values.clone(),
        ),
        valid(
            3,
            EvaluationOutcome::Improved,
            [0.1, 0.2, 0.3, 0.123_456_789_012_345_67],
            f64::MIN_POSITIVE,
            f64::MIN_POSITIVE,
            values.iter().rev().copied().collect(),
        ),
    ]
}

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|v| v.to_bits()).collect()
}

/// D4: every real of the trace reads back bit for bit, a rejected evaluation included.
#[test]
fn adversary_trace_round_trips_extreme_reals_bit_for_bit() {
    let log = extreme_log();
    let text = write_trace(
        &Topology::subtractive_mono(),
        &log,
        StopReason::RenderBudgetExhausted,
    )
    .unwrap();
    let trace = parse_trace(&text).unwrap();
    assert_eq!(trace.records.len(), log.len());
    for (record, evaluation) in trace.records.iter().zip(&log) {
        assert_eq!(record.sequence, evaluation.sequence);
        assert_eq!(record.outcome, evaluation.outcome);
        assert_eq!(bits(&record.normalized), bits(&evaluation.normalized));
        assert_eq!(
            record.total.map(f64::to_bits),
            evaluation.total.map(f64::to_bits)
        );
        assert_eq!(
            record.best_after.map(f64::to_bits),
            evaluation.best_after.map(f64::to_bits)
        );
        let flat = |c: Option<LossComponents>| {
            c.map(|c| bits(&[c.log_spectral, c.envelope, c.transient, c.band_energy]))
        };
        assert_eq!(flat(record.components), flat(evaluation.components));
    }
    for line in text.lines() {
        assert!(line.len() <= patchback_fit::TRACE_MAX_LINE_BYTES, "{line}");
    }
}

/// The documented field order of a valid evaluation line, read without `parse_trace`: the four
/// components, then the total, then the best, then the values. A writer and parser that swapped
/// any two of them together would pass every round-trip test.
#[test]
fn adversary_a_valid_evaluation_line_has_the_documented_field_order() {
    let log = extreme_log();
    let text = write_trace(
        &Topology::subtractive_mono(),
        &log,
        StopReason::RenderBudgetExhausted,
    )
    .unwrap();
    let line = text.lines().nth(4).unwrap();
    let fields: Vec<&str> = line.split(' ').collect();
    assert_eq!(
        fields[..6],
        [
            "evaluation",
            "3",
            "3",
            "0",
            "improved",
            "subtractive-mono/1"
        ]
    );
    let reals: Vec<u64> = fields[6..]
        .iter()
        .map(|token| token.parse::<f64>().unwrap().to_bits())
        .collect();
    let evaluation = &log[2];
    let mut expected = bits(&[0.1, 0.2, 0.3, 0.123_456_789_012_345_67]);
    expected.push(f64::MIN_POSITIVE.to_bits());
    expected.push(f64::MIN_POSITIVE.to_bits());
    expected.extend(bits(&evaluation.normalized));
    assert_eq!(reals, expected, "{line}");
    let header: Vec<&str> = text.lines().nth(1).unwrap().split(' ').collect();
    assert_eq!(
        header,
        [
            "topology",
            "subtractive-mono/1",
            "attack",
            "decay",
            "sustain",
            "release",
            "filter_cutoff",
            "resonance_q",
            "drive",
            "output_gain",
        ]
    );
}

fn written_trace() -> String {
    write_trace(
        &Topology::subtractive_mono(),
        &extreme_log(),
        StopReason::RenderBudgetExhausted,
    )
    .unwrap()
}

/// D3: an evaluation line naming a topology other than the `topology` line's is refused.
#[test]
fn adversary_an_evaluation_line_of_another_topology_is_refused() {
    let text = written_trace();
    let line = text.lines().nth(3).unwrap().to_owned();
    let other = line.replacen("subtractive-mono/1", "subtractive-mono/2", 1);
    let error = parse_trace(&text.replacen(&line, &other, 1)).unwrap_err();
    assert_eq!(error.line, Some(4), "{error}");
}

/// A duplicate sequence and an extra field are refused on their line.
#[test]
fn adversary_a_duplicate_sequence_and_an_extra_field_are_refused() {
    let text = written_trace();
    let third = text.lines().nth(4).unwrap().to_owned();
    let duplicate = third.replacen("evaluation 3 3", "evaluation 2 3", 1);
    let error = parse_trace(&text.replacen(&third, &duplicate, 1)).unwrap_err();
    assert_eq!(error.line, Some(5), "{error}");
    let extra = format!("{third} 0.5");
    let error = parse_trace(&text.replacen(&third, &extra, 1)).unwrap_err();
    assert_eq!(error.line, Some(5), "{error}");
}

/// `TraceFrame.evaluation` is the run's evaluation count after the candidate, which is its
/// sequence (the module documentation: equal to `<sequence>` for every run). A trace whose count
/// disagrees with its sequence is not a trace any run can produce, and the studio would replay a
/// wrong count from it: the parser refuses it.
#[test]
fn adversary_an_evaluation_count_other_than_the_sequence_is_refused() {
    let text = written_trace();
    let second = text.lines().nth(3).unwrap().to_owned();
    let skewed = second.replacen("evaluation 2 2", "evaluation 2 9", 1);
    assert_ne!(skewed, second);
    let parsed = parse_trace(&text.replacen(&second, &skewed, 1));
    assert!(
        parsed.is_err(),
        "accepted evaluation count 9 on sequence 2: {:?}",
        parsed.map(|t| t.records[1].evaluations)
    );
}

/// `TraceFrame.best_loss` is "the retained best loss after this frame ... which never rises"
/// (`spec/domains/studio.yaml`). A trace whose best rises is refused.
#[test]
fn adversary_a_best_loss_that_rises_is_refused() {
    let text = written_trace();
    let third = text.lines().nth(4).unwrap().to_owned();
    let fields: Vec<&str> = third.split(' ').collect();
    let mut risen: Vec<String> = fields.iter().map(|f| (*f).to_owned()).collect();
    risen[10] = "0.5".to_owned();
    risen[11] = "0.5".to_owned();
    let risen = risen.join(" ");
    let parsed = parse_trace(&text.replacen(&third, &risen, 1));
    assert!(
        parsed.is_err(),
        "accepted a best loss rising from 0.1 to 0.5: {:?}",
        parsed.map(|t| t.records[2].best_after)
    );
}

/// Two notes of MIDI 45 at full velocity, 0.25 s each.
fn hot_notes() -> NoteSequence {
    NoteSequence {
        format_version: 1,
        sample_rate_hz: 48_000,
        patch_id: "7f0c0000-0000-4000-8000-000000000a01".to_owned(),
        note_estimation_id: None,
        notes: vec![NoteEvent {
            start_sample: 0,
            end_sample: 24_000,
            midi_note: 45,
            velocity: 1.0,
        }],
    }
}

/// A hot reference: output gain at 0 dB, drive at 24 dB, resonance and sustain at their tops,
/// cut to its first 6,000 frames (0.125 s) while the note still sounds.
fn hot_reference() -> AudioClip {
    let topology = Topology::subtractive_mono();
    let mut point = default_point(&topology);
    for path in ["output_gain", "drive", "resonance_q", "sustain"] {
        point[topology.contract(path).unwrap().index] = 1.0;
    }
    let plan = compile(&validate(&patch_at(&topology, &point).unwrap()).unwrap());
    let rendered = render(&plan, &hot_notes(), &RenderConfig::default()).unwrap();
    assert!(rendered.report.limited, "the reference itself is limited");
    let mut samples = rendered.samples;
    samples.truncate(6_000);
    AudioClip::from_mono(samples).unwrap()
}

/// The limiter warning of a real fit whose best render is limited: the report and the results
/// carry `limiter engaged on <n> of <m> rendered samples`, `<n>` and `<m>` the limited samples
/// and the frame count of the uncut render of the best patch (Da7-15). A reference that ends
/// while the note still sounds is cut from a longer render: `reconstructed.wav` keeps the
/// reference's 6,000 frames, so it holds no more samples at the threshold than the warning
/// counts, and the warning names the longer render it counts in.
#[test]
fn adversary_the_limiter_warning_counts_the_limited_samples_of_the_uncut_render() {
    let reference = hot_reference();
    let config = FitConfig {
        bounds: FitBounds {
            max_renders: 40,
            max_wall_time_ms: 600_000,
            max_restarts: 1,
            improvement_threshold: 1e-9,
        },
        seed: 3,
        render: RenderConfig::default(),
    };
    let outcome = fit(
        &reference,
        &hot_notes(),
        &Topology::subtractive_mono(),
        &config,
    )
    .expect("the fit completes");
    assert!(
        outcome.render_report.limited,
        "precondition: the best render is limited ({:?})",
        outcome.normalized
    );
    let out = scratch("limiter");
    let results = write_results(&out, &outcome, &hot_notes(), &meta()).unwrap();
    let report = fs::read_to_string(out.join("report.txt")).unwrap();
    let warning_lines: Vec<&str> = report
        .lines()
        .filter_map(|line| line.strip_prefix("warning "))
        .collect();
    assert_eq!(warning_lines, results.warnings, "report and results agree");
    let wav = read_wav(&out.join("reconstructed.wav")).unwrap();
    let at_threshold = wav
        .samples()
        .iter()
        .filter(|sample| sample.abs() == 1.0)
        .count();
    let limited_samples = outcome.render_report.limited_samples;
    let frame_count = outcome.render_report.frame_count;
    assert_eq!(
        results.warnings,
        [format!(
            "limiter engaged on {limited_samples} of {frame_count} rendered samples"
        )],
        "reconstructed.wav has {} frames, the render {frame_count}",
        wav.frame_count(),
    );
    assert!(
        u64::try_from(at_threshold).unwrap() <= limited_samples,
        "{at_threshold} samples at the threshold in reconstructed.wav, {limited_samples} limited"
    );
    assert_eq!(wav.frame_count(), reference.frame_count());
    assert_eq!(wav.frame_count(), 6_000);
}

/// A write that fails on the fourth file (`report.txt` exists) removes the three this call wrote
/// and leaves both the file in the way and an unrelated file as they were.
#[test]
fn adversary_a_failed_fourth_write_removes_only_what_it_wrote() {
    let topology = Topology::subtractive_mono();
    let reference = {
        let mut point = default_point(&topology);
        point[topology.contract("filter_cutoff").unwrap().index] = 0.25;
        let plan = compile(&validate(&patch_at(&topology, &point).unwrap()).unwrap());
        let mut notes = hot_notes();
        notes.notes[0].velocity = 0.8;
        AudioClip::from_mono(
            render(&plan, &notes, &RenderConfig::default())
                .unwrap()
                .samples,
        )
        .unwrap()
    };
    let mut notes = hot_notes();
    notes.notes[0].velocity = 0.8;
    let outcome = fit(
        &reference,
        &notes,
        &topology,
        &FitConfig {
            bounds: FitBounds {
                max_renders: 4,
                max_wall_time_ms: 600_000,
                max_restarts: 1,
                improvement_threshold: 10.0,
            },
            seed: 1,
            render: RenderConfig::default(),
        },
    )
    .unwrap();
    let out = scratch("fourth");
    fs::write(out.join("report.txt"), "in the way\n").unwrap();
    fs::write(out.join("unrelated.txt"), "keep\n").unwrap();
    let error = write_results(&out, &outcome, &notes, &meta()).unwrap_err();
    assert!(
        matches!(&error, ResultsError::Io { path, .. } if *path == out.join("report.txt")),
        "{error:?}"
    );
    let mut names: Vec<String> = fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["report.txt", "unrelated.txt"]);
    assert_eq!(
        fs::read_to_string(out.join("report.txt")).unwrap(),
        "in the way\n"
    );
    assert_eq!(
        fs::read_to_string(out.join("unrelated.txt")).unwrap(),
        "keep\n"
    );
}
