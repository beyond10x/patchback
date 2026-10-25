//! The acceptance scenarios of story:fit-run-artifacts (`spec/scenarios/fit-run-artifacts-*.yaml`)
//! as tests of [`write_results`], `WriteFitResults` of `spec/domains/fit.yaml`, and the six files
//! of DESIGN.md § 11 it writes, each read back with its own reader:
//!
//! | Scenario | Test |
//! |---|---|
//! | `fit-run-artifacts-completed-writes-results` | [`a_completed_run_writes_its_six_files_naming_the_best_patch`] |
//! | `fit-run-artifacts-failed-run-writes-no-patch` | [`a_failed_run_is_refused_and_writes_nothing`] |
//! | `fit-run-artifacts-no-valid-candidate` | [`a_run_with_only_rejected_candidates_fails_and_writes_nothing`] (and `tests/fit_run.rs::a_run_without_a_valid_candidate_cannot_complete_and_fails`) |
//! | `fit-run-artifacts-running-run-refused` | [`a_running_run_is_refused_and_writes_nothing`]; its `no-such-run` step has no counterpart: a run is a value (Da7-7) |
//!
//! One fit of a reference one parameter away from the default patch, 0.25 s, 12 renders, is shared
//! by every test of this file.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use patchback_audio::{AudioClip, WavSampleFormat, read_wav, write_wav};
use patchback_core::digest::{hex, sha256};
use patchback_core::parameter::Topology;
use patchback_core::patch::{compile, load_patch_file, validate};
use patchback_core::score::{NoteEvent, NoteSequence, read_events_file, write_events};
use patchback_dsp::{RenderConfig, render};
use patchback_fit::{
    Evaluation, EvaluationOutcome, FailureReason, FitBounds, FitConfig, FitError, FitOutcome,
    FitRun, FitState, LossComponents, PATCHBACK_VERSION, RESULT_FILES, Rejection, ResultsError,
    RunMeta, StopReason, TRACE_HEADER, TRACE_MAX_LINE_BYTES, TraceReason, compare, default_point,
    fit, parse_trace, patch_at, write_results, write_trace,
};

/// A fresh, empty directory for one test under Cargo's per-target scratch directory.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-fit-results")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Two notes of MIDI 45, 0.125 s and about 0.1 s, listed out of order so the canonical form
/// differs from the input.
fn notes() -> NoteSequence {
    NoteSequence {
        format_version: 1,
        sample_rate_hz: 48_000,
        patch_id: "7f0c0000-0000-4000-8000-000000000911".to_owned(),
        note_estimation_id: None,
        notes: vec![
            NoteEvent {
                start_sample: 7_000,
                end_sample: 12_000,
                midi_note: 45,
                velocity: 0.5,
            },
            NoteEvent {
                start_sample: 0,
                end_sample: 6_000,
                midi_note: 45,
                velocity: 0.8,
            },
        ],
    }
}

/// The default patch with the filter cutoff at `p = 0.25`, rendered against [`notes`].
fn reference() -> AudioClip {
    let topology = Topology::subtractive_mono();
    let mut point = default_point(&topology);
    point[topology.contract("filter_cutoff").unwrap().index] = 0.25;
    let plan = compile(&validate(&patch_at(&topology, &point).unwrap()).unwrap());
    let samples = render(&plan, &notes(), &RenderConfig::default())
        .unwrap()
        .samples;
    AudioClip::from_mono(samples).unwrap()
}

fn config() -> FitConfig {
    FitConfig {
        bounds: FitBounds {
            max_renders: 12,
            max_wall_time_ms: 600_000,
            max_restarts: 2,
            improvement_threshold: 10.0,
        },
        seed: 42,
        render: RenderConfig::default(),
    }
}

/// The shared fit: its reference and its completed outcome.
fn fitted() -> &'static (AudioClip, FitOutcome) {
    static FITTED: OnceLock<(AudioClip, FitOutcome)> = OnceLock::new();
    FITTED.get_or_init(|| {
        let reference = reference();
        let outcome = fit(
            &reference,
            &notes(),
            &Topology::subtractive_mono(),
            &config(),
        )
        .expect("the fit completes");
        (reference, outcome)
    })
}

fn meta() -> RunMeta {
    RunMeta {
        seed: 42,
        block_size_frames: 256,
        modelspec_version: "0.5.0".to_owned(),
        reference_sha256: sha256(b"reference"),
        notes_sha256: sha256(b"notes"),
    }
}

/// The `key value` records of a line-oriented file, its header line included.
fn records(text: &str) -> Vec<(String, String)> {
    assert!(text.ends_with('\n'), "every line ends in \\n");
    text.lines()
        .map(|line| {
            let (key, value) = line.split_once(' ').expect("key value");
            (key.to_owned(), value.to_owned())
        })
        .collect()
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort_unstable();
    names
}

/// The completed outcome with its run replaced by `run`.
fn with_run(run: FitRun<u64>) -> FitOutcome {
    let mut outcome = fitted().1.clone();
    outcome.run = run;
    outcome
}

/// fit-run-artifacts-completed-writes-results: the results of a completed run are written, its
/// best patch as the recovered patch, and the six files of DESIGN.md § 11 exist.
#[test]
fn a_completed_run_writes_its_six_files_naming_the_best_patch() {
    let (_, outcome) = fitted();
    assert_eq!(outcome.run.state(), FitState::Completed);
    let out = scratch("completed").join("results");
    let results = write_results(&out, outcome, &notes(), &meta()).expect("written");
    assert_eq!(results.out_dir, out);
    assert_eq!(results.recovered_candidate, outcome.run.best().copied());
    assert_eq!(results.recovered_patch, outcome.patch);
    assert_eq!(results.reconstructed_wav, out.join("reconstructed.wav"));
    assert!(results.warnings.is_empty(), "{:?}", results.warnings);
    assert_eq!(
        RESULT_FILES,
        [
            "patch.patch",
            "notes.events",
            "reconstructed.wav",
            "report.txt",
            "run.meta",
            "trace.events",
        ]
    );
    assert_eq!(
        results.files,
        RESULT_FILES.map(|name| out.join(name)).to_vec()
    );
    let mut expected: Vec<String> = RESULT_FILES.iter().map(|name| (*name).to_owned()).collect();
    expected.sort_unstable();
    assert_eq!(names(&out), expected);
    let best = outcome.run.best().copied().unwrap();
    let retained = &outcome.log[usize::try_from(best).unwrap() - 1];
    assert!(retained.outcome.retained());
    assert_eq!(retained.normalized, outcome.normalized);
}

/// fit-run-artifacts-failed-run-writes-no-patch: a run that failed on a renderer fault is
/// refused FailedRunHasNoPatch and nothing is written, not even the directory.
#[test]
fn a_failed_run_is_refused_and_writes_nothing() {
    let mut run = FitRun::start(config().bounds).unwrap();
    run.fail(FailureReason::RendererFault, 1_000).unwrap();
    let out = scratch("failed").join("results");
    let error = write_results(&out, &with_run(run), &notes(), &meta()).unwrap_err();
    let ResultsError::Run(error) = error else {
        panic!("{error:?}");
    };
    assert_eq!(error, FitError::FailedRunHasNoPatch);
    assert_eq!(error.domain_error(), "patchback.fit.FailedRunHasNoPatch");
    assert!(!out.exists());
}

/// fit-run-artifacts-no-valid-candidate: a run whose only candidate was rejected cannot complete
/// (NoValidCandidate), fails with that reason, and its results are then refused like any failed
/// run's.
#[test]
fn a_run_with_only_rejected_candidates_fails_and_writes_nothing() {
    let mut run = FitRun::start(FitBounds {
        max_renders: 2_000,
        max_wall_time_ms: 600_000,
        max_restarts: 4,
        improvement_threshold: 0.001,
    })
    .unwrap();
    assert_eq!(run.reject(), Ok(EvaluationOutcome::Rejected));
    assert_eq!(
        run.complete(StopReason::RestartsExhausted, 2_000),
        Err(FitError::NoValidCandidate)
    );
    assert_eq!(run.fail(FailureReason::NoValidCandidate, 2_000), Ok(()));
    assert_eq!(run.failure_reason(), Some(FailureReason::NoValidCandidate));
    let out = scratch("no-valid-candidate").join("results");
    let error = write_results(&out, &with_run(run), &notes(), &meta()).unwrap_err();
    assert!(
        matches!(error, ResultsError::Run(FitError::FailedRunHasNoPatch)),
        "{error:?}"
    );
    assert!(!out.exists());
}

/// fit-run-artifacts-running-run-refused: the results of a run still Running are refused
/// FitStillRunning and nothing is written.
#[test]
fn a_running_run_is_refused_and_writes_nothing() {
    let mut run = FitRun::start(config().bounds).unwrap();
    let components = fitted().1.run.best_components().copied().unwrap();
    run.evaluate(1, components, 0.5).unwrap();
    let out = scratch("running").join("results");
    let error = write_results(&out, &with_run(run), &notes(), &meta()).unwrap_err();
    let ResultsError::Run(error) = error else {
        panic!("{error:?}");
    };
    assert_eq!(error, FitError::FitStillRunning);
    assert_eq!(error.domain_error(), "patchback.fit.FitStillRunning");
    assert!(!out.exists());
}

/// `patch.patch` loads back to the best patch; `notes.events` reads back to the canonical input;
/// `reconstructed.wav` compared with the reference gives the best loss exactly (the story's last
/// open question).
#[test]
fn the_patch_notes_and_wav_read_back_to_the_run() {
    let (reference, outcome) = fitted();
    let out = scratch("read-back");
    write_results(&out, outcome, &notes(), &meta()).unwrap();
    let loaded = load_patch_file(out.join("patch.patch")).unwrap();
    assert_eq!(*loaded.patch(), outcome.patch.canonical());
    assert_eq!(
        fs::read_to_string(out.join("patch.patch")).unwrap(),
        patchback_core::patch::write_patch(&outcome.patch).unwrap()
    );
    let read = read_events_file(out.join("notes.events")).unwrap();
    assert_eq!(read, notes().canonical());
    assert_ne!(read, notes(), "the input was not canonical");
    assert_eq!(
        fs::read_to_string(out.join("notes.events")).unwrap(),
        write_events(&notes()).unwrap()
    );
    let wav = read_wav(&out.join("reconstructed.wav")).unwrap();
    assert_eq!(wav.source_format(), Some(WavSampleFormat::Float32));
    assert_eq!(wav.samples(), outcome.render.samples());
    assert_eq!(wav.frame_count(), reference.frame_count());
    let comparison = compare(reference, &wav).unwrap();
    assert_eq!(Some(comparison.total), outcome.run.best_loss());
    assert_eq!(Some(&comparison.components), outcome.run.best_components());
}

/// `report.txt`: the header, then the documented records in their order, each value the run's.
#[test]
fn the_report_has_its_records_in_order() {
    let (_, outcome) = fitted();
    let out = scratch("report");
    write_results(&out, outcome, &notes(), &meta()).unwrap();
    let text = fs::read_to_string(out.join("report.txt")).unwrap();
    let run = &outcome.run;
    let components = run.best_components().unwrap();
    let expected: Vec<(String, String)> = [
        ("patchback-fit-report", "1".to_owned()),
        ("patchback", PATCHBACK_VERSION.to_owned()),
        ("initial_loss", run.initial_loss().unwrap().to_string()),
        ("best_loss", run.best_loss().unwrap().to_string()),
        ("log_spectral", components.log_spectral.to_string()),
        ("envelope", components.envelope.to_string()),
        ("transient", components.transient.to_string()),
        ("band_energy", components.band_energy.to_string()),
        ("evaluations", run.evaluations().to_string()),
        ("restarts", outcome.restarts.to_string()),
        ("stop_reason", run.stop_reason().unwrap().name().to_owned()),
        ("wall_time_ms", run.wall_time_ms().unwrap().to_string()),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value))
    .collect();
    assert_eq!(records(&text), expected);
    assert_eq!(PATCHBACK_VERSION, env!("CARGO_PKG_VERSION"));
    assert!(!text.contains("confidence"), "Da7-4: no confidence record");
}

/// A best render the limiter changed is a warning: one `warning` line at the end of the report,
/// and in the results' warnings, counting the limited samples of the best patch's uncut render
/// out of its frames (Da7-15).
#[test]
fn a_limited_best_render_is_a_warning() {
    let mut outcome = fitted().1.clone();
    outcome.render_report.limited = true;
    outcome.render_report.limited_samples = 7;
    let frames = outcome.render_report.frame_count;
    let out = scratch("limited");
    let results = write_results(&out, &outcome, &notes(), &meta()).unwrap();
    let warning = format!("limiter engaged on 7 of {frames} rendered samples");
    assert_eq!(results.warnings, std::slice::from_ref(&warning));
    let text = fs::read_to_string(out.join("report.txt")).unwrap();
    assert!(text.ends_with(&format!("\nwarning {warning}\n")), "{text}");
    assert_eq!(text.matches("warning").count(), 1, "{text}");
}

/// `run.meta`: the header, then the documented records in their order; the digests are the
/// SHA-256 of the input files' bytes.
#[test]
fn the_run_meta_has_its_records_in_order_and_the_input_digests() {
    let (reference, outcome) = fitted();
    let dir = scratch("run-meta");
    let wav = dir.join("reference.wav");
    write_wav(&wav, reference, WavSampleFormat::Pcm16).unwrap();
    let events = dir.join("notes.events");
    fs::write(&events, write_events(&notes()).unwrap()).unwrap();
    let meta = RunMeta {
        seed: 42,
        block_size_frames: 512,
        modelspec_version: "9.8.7".to_owned(),
        reference_sha256: sha256(&fs::read(&wav).unwrap()),
        notes_sha256: sha256(&fs::read(&events).unwrap()),
    };
    let out = dir.join("results");
    write_results(&out, outcome, &notes(), &meta).unwrap();
    let text = fs::read_to_string(out.join("run.meta")).unwrap();
    let expected: Vec<(String, String)> = [
        ("patchback-fit-run-meta", "1".to_owned()),
        ("patchback", PATCHBACK_VERSION.to_owned()),
        ("init", "default".to_owned()),
        ("seed", "42".to_owned()),
        ("topology", "subtractive-mono/1".to_owned()),
        (
            "feature_contract",
            patchback_analysis::FEATURES_1_VERSION.to_owned(),
        ),
        ("prng", "xoshiro256**".to_owned()),
        ("modelspec", "9.8.7".to_owned()),
        (
            "reference",
            format!("sha256 {}", hex(&sha256(&fs::read(&wav).unwrap()))),
        ),
        (
            "notes",
            format!("sha256 {}", hex(&sha256(&fs::read(&events).unwrap()))),
        ),
        ("max_renders", "12".to_owned()),
        ("max_wall_time_ms", "600000".to_owned()),
        ("max_restarts", "2".to_owned()),
        ("improvement_threshold", "10".to_owned()),
        ("block_size", "512".to_owned()),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value))
    .collect();
    assert_eq!(records(&text), expected);
    assert_eq!(patchback_core::prng::NAME, "xoshiro256**");
}

/// `trace.events` parses back to the log: one line per evaluation between the header and
/// topology lines and the closing line, each with the log's sequence, outcome, losses and
/// values; the best loss never rises along it.
#[test]
fn the_trace_parses_back_to_the_log() {
    let (_, outcome) = fitted();
    let out = scratch("trace");
    write_results(&out, outcome, &notes(), &meta()).unwrap();
    let text = fs::read_to_string(out.join("trace.events")).unwrap();
    assert_eq!(text.lines().count(), outcome.log.len() + 3);
    assert!(text.starts_with(&format!("{TRACE_HEADER} 1\n")), "{text}");
    assert!(
        text.ends_with(&format!(
            "\ncompleted {}\n",
            outcome.run.stop_reason().unwrap()
        )),
        "{text}"
    );
    let trace = parse_trace(&text).unwrap();
    let topology = Topology::subtractive_mono();
    let mut contracts = topology.contracts().to_vec();
    contracts.sort_by_key(|contract| contract.index);
    let paths: Vec<String> = contracts.into_iter().map(|c| c.path).collect();
    assert_eq!(trace.topology, "subtractive-mono/1");
    assert_eq!(trace.paths, paths);
    assert_eq!(Some(trace.stop_reason), outcome.run.stop_reason());
    assert_eq!(trace.records.len(), outcome.log.len());
    let mut previous_best: Option<f64> = None;
    for (record, evaluation) in trace.records.iter().zip(&outcome.log) {
        assert_eq!(record.sequence, evaluation.sequence);
        assert_eq!(record.evaluations, evaluation.sequence);
        assert_eq!(record.restart, evaluation.restart);
        assert_eq!(record.outcome, evaluation.outcome);
        assert_eq!(record.topology, "subtractive-mono/1");
        assert_eq!(record.components, evaluation.components);
        assert_eq!(record.total, evaluation.total);
        assert_eq!(record.best_after, evaluation.best_after);
        assert_eq!(record.normalized, evaluation.normalized);
        if let (Some(before), Some(after)) = (previous_best, record.best_after) {
            assert!(after <= before, "the best rose from {before} to {after}");
        }
        assert!(previous_best.is_none() || record.best_after.is_some());
        previous_best = record.best_after;
    }
    assert_eq!(previous_best, outcome.run.best_loss());
}

/// A rejected candidate carries one `-` for its losses, and `-` stands for the best loss until a
/// candidate is retained.
#[test]
fn a_rejected_candidate_and_an_absent_best_are_markers() {
    let (_, outcome) = fitted();
    let topology = Topology::subtractive_mono();
    let mut log = outcome.log.clone();
    log[0].outcome = EvaluationOutcome::Rejected;
    log[0].components = None;
    log[0].total = None;
    log[0].best_after = None;
    let text = write_trace(&topology, &log, StopReason::RestartsExhausted).unwrap();
    let line = text.lines().nth(2).unwrap();
    let fields: Vec<&str> = line.split(' ').collect();
    assert_eq!(
        &fields[..8],
        [
            "evaluation",
            "1",
            "1",
            "0",
            "rejected",
            "subtractive-mono/1",
            "-",
            "-"
        ],
        "{line}"
    );
    assert_eq!(fields.len(), 8 + topology.contracts().len(), "{line}");
    let trace = parse_trace(&text).unwrap();
    assert_eq!(trace.records[0].components, None);
    assert_eq!(trace.records[0].total, None);
    assert_eq!(trace.records[0].best_after, None);
    assert_eq!(trace.records[0].normalized, log[0].normalized);
}

/// The trace parser refuses an unknown version, a malformed number and a line past the bound,
/// as `.events` does, and a file without its closing line.
#[test]
fn the_trace_parser_refuses_what_the_writer_never_writes() {
    let (_, outcome) = fitted();
    let text = write_trace(
        &Topology::subtractive_mono(),
        &outcome.log,
        StopReason::RestartsExhausted,
    )
    .unwrap();
    assert!(parse_trace(&text).is_ok());
    let unknown = text.replacen(
        &format!("{TRACE_HEADER} 1\n"),
        &format!("{TRACE_HEADER} 2\n"),
        1,
    );
    let error = parse_trace(&unknown).unwrap_err();
    assert_eq!(error.line, Some(1));
    assert!(
        matches!(error.reason, TraceReason::UnknownFormatVersion { .. }),
        "{error:?}"
    );

    let evaluation = text.lines().nth(2).unwrap();
    let last = evaluation.rsplit_once(' ').unwrap().0;
    for bad in ["NaN", "inf", "0.5.5", "1e", "x"] {
        let malformed = text.replacen(evaluation, &format!("{last} {bad}"), 1);
        let error = parse_trace(&malformed).unwrap_err();
        assert_eq!(error.line, Some(3), "{bad}");
        assert!(
            matches!(error.reason, TraceReason::MalformedNumber { .. }),
            "{bad}: {error:?}"
        );
    }

    let long = text.replacen(
        evaluation,
        &format!("{evaluation}{}", " ".repeat(TRACE_MAX_LINE_BYTES)),
        1,
    );
    let error = parse_trace(&long).unwrap_err();
    assert_eq!(error.line, Some(3));
    assert!(
        matches!(error.reason, TraceReason::LineTooLong { .. }),
        "{error:?}"
    );

    let open = text.rsplit_once("completed").unwrap().0;
    let error = parse_trace(open).unwrap_err();
    assert!(matches!(error.reason, TraceReason::Incomplete), "{error:?}");

    let short = text.replacen(evaluation, last, 1);
    let error = parse_trace(&short).unwrap_err();
    assert_eq!(error.line, Some(3));
    assert!(
        matches!(error.reason, TraceReason::MalformedRecord { .. }),
        "{error:?}"
    );
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

/// `text` with field `field` of line `line` (1-based) replaced by `value`.
fn with_field(text: &str, line: usize, field: usize, value: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let mut fields: Vec<&str> = lines[line - 1].split(' ').collect();
    fields[field] = value;
    lines[line - 1] = fields.join(" ");
    lines.join("\n") + "\n"
}

/// Da7-16: a trace no run writes is refused, each fault by its own reason naming the line and
/// the values: an evaluation count other than the sequence number, a best loss above the previous
/// line's, and a best loss of `-` after a numeric one. The first numeric best may follow any
/// number of `-` lines, and a best equal to the previous one is no rise.
#[test]
fn the_trace_parser_refuses_a_count_or_a_best_no_run_writes() {
    use EvaluationOutcome::{FirstRetained, Improved, NotImproved, Rejected};
    let log = [
        logged(1, Rejected, None, None),
        logged(2, Rejected, None, None),
        logged(3, FirstRetained, Some(0.5), Some(0.5)),
        logged(4, Rejected, None, Some(0.5)),
        logged(5, NotImproved, Some(0.9), Some(0.5)),
        logged(6, Improved, Some(0.25), Some(0.25)),
    ];
    let text = write_trace(
        &Topology::subtractive_mono(),
        &log,
        StopReason::RestartsExhausted,
    )
    .unwrap();
    let trace = parse_trace(&text).unwrap();
    let bests: Vec<Option<f64>> = trace.records.iter().map(|r| r.best_after).collect();
    assert_eq!(
        bests,
        [None, None, Some(0.5), Some(0.5), Some(0.5), Some(0.25)]
    );

    let skewed = with_field(&text, 4, 2, "7");
    let error = parse_trace(&skewed).unwrap_err();
    assert_eq!(error.line, Some(4), "{error}");
    assert_eq!(
        error.reason,
        TraceReason::EvaluationCountNotSequence {
            sequence: 2,
            evaluations: 7
        }
    );
    let message = error.to_string();
    assert!(
        message.starts_with("line 4: ") && message.contains('7') && message.contains('2'),
        "{message}"
    );

    let risen = with_field(&text, 8, 11, "0.75");
    let error = parse_trace(&risen).unwrap_err();
    assert_eq!(error.line, Some(8), "{error}");
    assert_eq!(
        error.reason,
        TraceReason::BestLossRose {
            previous: 0.5,
            best: 0.75
        }
    );
    let message = error.to_string();
    assert!(
        message.starts_with("line 8: ") && message.contains("0.75") && message.contains("0.5"),
        "{message}"
    );

    let lost = with_field(&text, 8, 11, "-");
    let error = parse_trace(&lost).unwrap_err();
    assert_eq!(error.line, Some(8), "{error}");
    assert_eq!(error.reason, TraceReason::BestLossLost { previous: 0.5 });
    let message = error.to_string();
    assert!(
        message.starts_with("line 8: ") && message.contains("0.5"),
        "{message}"
    );

    let lost_on_a_rejected_line = with_field(&text, 6, 7, "-");
    let error = parse_trace(&lost_on_a_rejected_line).unwrap_err();
    assert_eq!(error.line, Some(6), "{error}");
    assert_eq!(error.reason, TraceReason::BestLossLost { previous: 0.5 });
}

/// Da7-20: a retained candidate has a best loss, and a best loss no candidate scored is refused.
/// A `first-retained`, `improved` or `not-improved` line with `-` and no numeric best before it
/// is `RetainedWithoutBestLoss`; a numeric best on a `rejected` line before the first
/// `first-retained` line is `BestLossBeforeRetained`. After it, a `rejected` line carries the
/// unchanged best (the unmodified trace parses).
#[test]
fn the_trace_parser_refuses_a_retained_line_without_a_best_and_a_best_before_one() {
    use EvaluationOutcome::{FirstRetained, Improved, NotImproved, Rejected};
    let log = [
        logged(1, Rejected, None, None),
        logged(2, Rejected, None, None),
        logged(3, FirstRetained, Some(0.5), Some(0.5)),
        logged(4, Rejected, None, Some(0.5)),
        logged(5, Improved, Some(0.25), Some(0.25)),
    ];
    let text = write_trace(
        &Topology::subtractive_mono(),
        &log,
        StopReason::RestartsExhausted,
    )
    .unwrap();
    assert_eq!(parse_trace(&text).unwrap().records[3].best_after, Some(0.5));

    for outcome in [FirstRetained, Improved, NotImproved] {
        let renamed = with_field(&text, 5, 4, outcome.name());
        let absent = with_field(&renamed, 5, 11, "-");
        let error = parse_trace(&absent).unwrap_err();
        assert_eq!(error.line, Some(5), "{error}");
        assert_eq!(
            error.reason,
            TraceReason::RetainedWithoutBestLoss { outcome }
        );
        let message = error.to_string();
        assert!(
            message.starts_with("line 5: ") && message.contains(outcome.name()),
            "{message}"
        );
    }

    let early = with_field(&text, 4, 7, "0.75");
    let error = parse_trace(&early).unwrap_err();
    assert_eq!(error.line, Some(4), "{error}");
    assert_eq!(
        error.reason,
        TraceReason::BestLossBeforeRetained { best: 0.75 }
    );
    let message = error.to_string();
    assert!(
        message.starts_with("line 4: ") && message.contains("0.75"),
        "{message}"
    );
}

/// Writing the same outcome twice gives the same bytes for every file; the report differs only
/// in its `wall_time_ms` record when the wall time differs.
#[test]
fn the_same_outcome_writes_the_same_bytes() {
    let (_, outcome) = fitted();
    let dir = scratch("same-bytes");
    let mut slower = outcome.clone();
    let mut run = FitRun::start(config().bounds).unwrap();
    for evaluation in &outcome.log {
        match evaluation.components {
            Some(components) => {
                run.evaluate(evaluation.sequence, components, evaluation.total.unwrap())
                    .unwrap();
            }
            None => {
                run.reject().unwrap();
            }
        }
    }
    let wall = outcome.run.wall_time_ms().unwrap() + 1_000;
    run.complete(outcome.run.stop_reason().unwrap(), wall)
        .unwrap();
    slower.run = run;
    write_results(&dir.join("a"), outcome, &notes(), &meta()).unwrap();
    write_results(&dir.join("b"), &slower, &notes(), &meta()).unwrap();
    for name in RESULT_FILES {
        let a = fs::read(dir.join("a").join(name)).unwrap();
        let b = fs::read(dir.join("b").join(name)).unwrap();
        if name == "report.txt" {
            let differing: Vec<(&str, &str)> = std::str::from_utf8(&a)
                .unwrap()
                .lines()
                .zip(std::str::from_utf8(&b).unwrap().lines())
                .filter(|(x, y)| x != y)
                .collect();
            assert_eq!(differing.len(), 1, "{differing:?}");
            assert!(differing[0].0.starts_with("wall_time_ms "));
            assert_eq!(differing[0].1, format!("wall_time_ms {wall}"));
        } else {
            assert_eq!(a, b, "{name}");
        }
    }
}

/// A write that fails part-way removes every file this call wrote and leaves what it did not
/// write: here `trace.events`, the last file, exists, so the five before it are written, then
/// removed.
#[test]
fn a_failed_write_removes_every_file_it_wrote() {
    let (_, outcome) = fitted();
    let out = scratch("cleanup");
    fs::write(out.join("trace.events"), "keep me\n").unwrap();
    let error = write_results(&out, outcome, &notes(), &meta()).unwrap_err();
    let ResultsError::Io { path, error } = &error else {
        panic!("{error:?}");
    };
    assert_eq!(*path, out.join("trace.events"));
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(names(&out), ["trace.events"]);
    assert_eq!(
        fs::read_to_string(out.join("trace.events")).unwrap(),
        "keep me\n"
    );
}
