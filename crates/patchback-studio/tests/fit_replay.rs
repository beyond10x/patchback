//! story:studio-fit-replay: a fit run's `trace.events` replayed as `TraceOpened`, one
//! `FrameAppended` per evaluation and `TraceClosedEvent` (spec/domains/studio.yaml, Dc1-24). The
//! model ([`patchback_studio::replay`]) answers `OpenTrace`, `AppendFrame` and `CloseTrace` in the
//! domain's order; the exporter reads a run directory through it, so every scenario holds both
//! on the model's inputs and on the trace file a run would write.

mod common;

use std::fs;

use common::{
    TOPOLOGY, default_run_meta, is_uuid_v8, json, repository_root, run_dir, scenario,
    topology_paths, trace_text,
};
use patchback_core::patch::{known_topology, write_patch};
use patchback_fit::{EvaluationOutcome, LossComponents, StopReason, patch_at};
use patchback_studio::StudioError;
use patchback_studio::export::export_run;
use patchback_studio::graph;
use patchback_studio::replay::{Frame, Opened, Replay, TraceEnd};
use serde_json::Value;

const TRACE: &str = "5a0d0000-0000-4000-8000-000000000401";

fn opened(paths: &[&str]) -> Opened {
    Opened {
        trace_id: TRACE.to_owned(),
        fit_run_id: "5a0d0000-0000-4000-8000-000000000501".to_owned(),
        topology_id: "5a0d0000-0000-4000-8000-000000000101".to_owned(),
        topology_name: TOPOLOGY.to_owned(),
        parameter_paths: paths.iter().map(|p| (*p).to_owned()).collect(),
    }
}

fn components(c: [f64; 4]) -> LossComponents {
    LossComponents {
        log_spectral: c[0],
        envelope: c[1],
        transient: c[2],
        band_energy: c[3],
    }
}

/// One frame of the scenarios: sequence, outcome, total, best, (attack, cutoff).
fn frame(
    sequence: u64,
    outcome: EvaluationOutcome,
    total: Option<f64>,
    best: Option<f64>,
    values: [f64; 2],
) -> Frame {
    Frame {
        frame_id: format!("5a0d0000-0000-4000-8000-{:012}", 600 + sequence),
        sequence,
        evaluation: sequence,
        restart: 0,
        outcome,
        topology_id: "5a0d0000-0000-4000-8000-000000000101".to_owned(),
        topology_name: TOPOLOGY.to_owned(),
        patch_id: total.map(|_| format!("5a0d0000-0000-4000-8000-{:012}", 700 + sequence)),
        total,
        components: total.map(|_| components([0.8, 0.6, 0.4, 0.5])),
        best,
        parameters: vec![
            ("attack".to_owned(), values[0]),
            ("cutoff".to_owned(), values[1]),
        ],
    }
}

use EvaluationOutcome::{FirstRetained, Improved, NotImproved, Rejected};

#[test]
fn studio_fit_replay_replayed_scenario_is_replayed_and_closed_with_its_end() {
    scenario("studio-fit-replay-replayed");
    let mut replay = Replay::open(opened(&["attack", "cutoff"])).expect("opened");
    for f in [
        frame(1, FirstRetained, Some(0.645), Some(0.645), [0.5, 0.5]),
        frame(2, Improved, Some(0.43), Some(0.43), [0.25, 0.5]),
        frame(3, NotImproved, Some(0.745), Some(0.43), [0.25, 0.75]),
        frame(4, Rejected, None, Some(0.43), [0.25, 0.25]),
    ] {
        replay.append(f).expect("appended");
    }
    replay
        .close(TraceEnd::Completed, Some(StopReason::RenderBudgetExhausted))
        .expect("closed");
    let document = replay.to_json();
    let frames = document["frames"].as_array().expect("frames");
    assert_eq!(frames.len(), 4);
    assert_eq!(frames[3]["sequence"], 4);
    assert_eq!(frames[3]["outcome"], "Rejected");
    assert_eq!(frames[3]["best_loss"], "0.43");
    assert!(frames[3].get("total_loss").is_none());
    assert!(frames[3].get("components").is_none());
    assert!(frames[3].get("patch_id").is_none());
    assert_eq!(frames[0]["total_loss"], "0.645");
    assert_eq!(frames[0]["components"]["log_spectral"], "0.8");
    assert_eq!(frames[1]["parameters"][0]["path"], "attack");
    assert_eq!(frames[1]["parameters"][0]["normalized_value"], "0.25");
    assert_eq!(document["opened"]["source"], "Replay");
    assert_eq!(document["opened"]["topology_name"], TOPOLOGY);
    assert_eq!(document["closed"]["ended"], "Completed");
    assert_eq!(document["closed"]["stop_reason"], "RenderBudgetExhausted");
    assert_eq!(document["closed"]["trace_id"], TRACE);

    let error = replay
        .append(frame(5, NotImproved, Some(0.5), Some(0.43), [0.5, 0.5]))
        .expect_err("a closed trace takes no frame");
    assert_eq!(error.name(), "patchback.studio.TraceClosed");
    let error = replay
        .close(TraceEnd::Completed, Some(StopReason::RenderBudgetExhausted))
        .expect_err("closed once");
    assert_eq!(error.name(), "patchback.studio.TraceStateConflict");
}

#[test]
fn studio_fit_replay_best_worsened_is_refused() {
    scenario("studio-fit-replay-best-worsened");
    let mut replay = Replay::open(opened(&["cutoff"])).expect("opened");
    replay
        .append(frame(1, FirstRetained, Some(0.43), Some(0.43), [0.5, 0.5]))
        .expect("first");
    let error = replay
        .append(frame(2, NotImproved, Some(0.745), Some(0.645), [0.5, 0.75]))
        .expect_err("the best loss rose");
    assert_eq!(
        error,
        StudioError::BestLossWorsened {
            trace_id: TRACE.to_owned(),
            sequence: 2
        }
    );
    assert_eq!(replay.frames().len(), 1);
}

#[test]
fn studio_fit_replay_out_of_order_is_refused() {
    scenario("studio-fit-replay-out-of-order");
    let mut replay = Replay::open(opened(&["cutoff"])).expect("opened");
    replay
        .append(frame(
            1,
            FirstRetained,
            Some(0.645),
            Some(0.645),
            [0.5, 0.5],
        ))
        .expect("first");
    for sequence in [1, 0] {
        let error = replay
            .append(frame(
                sequence,
                NotImproved,
                Some(0.745),
                Some(0.645),
                [0.5, 0.75],
            ))
            .expect_err("a repeated or lower sequence");
        assert_eq!(
            error,
            StudioError::FrameOutOfOrder {
                trace_id: TRACE.to_owned(),
                sequence
            }
        );
    }
}

#[test]
fn studio_fit_replay_inconsistent_frame_is_refused() {
    scenario("studio-fit-replay-inconsistent-frame");
    let mut replay = Replay::open(opened(&["cutoff"])).expect("opened");
    let inconsistent = |sequence| StudioError::InconsistentFrame {
        trace_id: TRACE.to_owned(),
        sequence,
    };
    let mut rejected = frame(1, Rejected, None, None, [0.5, 0.5]);
    rejected.total = Some(0.645);
    assert_eq!(replay.append(rejected), Err(inconsistent(1)));
    let improved = frame(1, Improved, Some(0.43), Some(0.645), [0.5, 0.5]);
    assert_eq!(replay.append(improved), Err(inconsistent(1)));
    let mut valid = frame(1, NotImproved, Some(0.43), Some(0.43), [0.5, 0.5]);
    valid.total = None;
    assert_eq!(replay.append(valid), Err(inconsistent(1)));
    assert!(replay.frames().is_empty());
    // the precedence order: an inconsistent frame is answered before its sequence
    replay
        .append(frame(
            1,
            FirstRetained,
            Some(0.645),
            Some(0.645),
            [0.5, 0.5],
        ))
        .expect("first");
    let mut late = frame(1, Rejected, None, Some(0.645), [0.5, 0.5]);
    late.total = Some(0.9);
    assert_eq!(replay.append(late), Err(inconsistent(1)));
}

#[test]
fn studio_fit_replay_inconsistent_end_is_refused() {
    scenario("studio-fit-replay-inconsistent-end");
    let mut replay = Replay::open(opened(&["cutoff"])).expect("opened");
    let inconsistent = StudioError::InconsistentEnd {
        trace_id: TRACE.to_owned(),
    };
    assert_eq!(
        replay.close(TraceEnd::Completed, None),
        Err(inconsistent.clone())
    );
    assert_eq!(
        replay.close(TraceEnd::Failed, Some(StopReason::RenderBudgetExhausted)),
        Err(inconsistent)
    );
    replay
        .close(
            TraceEnd::Completed,
            Some(StopReason::ImprovementBelowThreshold),
        )
        .expect("closed");
}

#[test]
fn a_duplicate_parameter_path_is_refused() {
    let error = Replay::open(opened(&["cutoff", "cutoff"])).expect_err("cutoff twice");
    assert_eq!(error.name(), "patchback.studio.DuplicateParameterPath");
}

// --- the exporter on trace files ---

/// The four evaluations of the replayed scenario as `trace.events` lines over the eight
/// parameters of the topology: (outcome, losses, best, values).
fn replayed_lines() -> Vec<String> {
    let rest = |v: f64| format!("{v} 0.5 0.5 0.5 0.5 0.5 0.5 0.5");
    vec![
        format!(
            "evaluation 1 1 0 first-retained {TOPOLOGY} 0.8 0.6 0.4 0.5 0.645 0.645 {}",
            rest(0.5)
        ),
        format!(
            "evaluation 2 2 0 improved {TOPOLOGY} 0.5 0.4 0.3 0.3 0.43 0.43 {}",
            rest(0.25)
        ),
        format!(
            "evaluation 3 3 1 not-improved {TOPOLOGY} 0.9 0.7 0.5 0.6 0.745 0.43 {}",
            rest(0.75)
        ),
        format!(
            "evaluation 4 4 1 rejected {TOPOLOGY} - 0.43 {}",
            rest(0.125)
        ),
    ]
}

fn exported(name: &str, lines: &[String]) -> Value {
    let text = trace_text(lines, "ImprovementBelowThreshold");
    let dir = run_dir(
        name,
        &common::retained_patch(lines),
        Some(default_run_meta()),
        Some(&text),
    );
    json(&export_run(&dir).expect("the run exports"))
}

fn refused(name: &str, lines: &[String]) -> (String, String) {
    let text = trace_text(lines, "ImprovementBelowThreshold");
    let dir = run_dir(
        name,
        &common::retained_patch(lines),
        Some(default_run_meta()),
        Some(&text),
    );
    let error = export_run(&dir).expect_err("the trace is refused");
    (error.name().to_owned(), error.to_string())
}

#[test]
fn studio_fit_replay_replayed_from_a_trace_file() {
    scenario("studio-fit-replay-replayed");
    let document = exported("replayed", &replayed_lines());
    let trace = &document["trace"];
    let opened = &trace["opened"];
    assert_eq!(opened["source"], "Replay");
    assert_eq!(opened["topology_name"], TOPOLOGY);
    assert_eq!(opened["topology_id"], document["graph"]["topology_id"]);
    let paths: Vec<&str> = opened["parameter_paths"]
        .as_array()
        .expect("paths")
        .iter()
        .map(|p| p.as_str().expect("a path"))
        .collect();
    assert_eq!(paths, topology_paths());
    for id in [&opened["trace_id"], &opened["fit_run_id"]] {
        assert!(is_uuid_v8(id.as_str().expect("an id")), "{id}");
    }

    let frames = trace["frames"].as_array().expect("frames");
    let field = |name: &str| -> Vec<Value> { frames.iter().map(|f| f[name].clone()).collect() };
    assert_eq!(field("sequence"), [1, 2, 3, 4].map(Value::from));
    assert_eq!(field("evaluation"), [1, 2, 3, 4].map(Value::from));
    assert_eq!(field("restart"), [0, 0, 1, 1].map(Value::from));
    assert_eq!(
        field("outcome"),
        ["FirstRetained", "Improved", "NotImproved", "Rejected"].map(Value::from)
    );
    assert_eq!(
        field("best_loss"),
        ["0.645", "0.43", "0.43", "0.43"].map(Value::from)
    );
    assert_eq!(
        field("total_loss")[..3],
        ["0.645", "0.43", "0.745"].map(Value::from)
    );
    assert!(frames[3].get("total_loss").is_none());
    assert!(frames[3].get("components").is_none());
    assert!(frames[3].get("patch_id").is_none());
    assert_eq!(frames[1]["components"]["band_energy"], "0.3");
    assert_eq!(frames[3]["parameters"][0]["normalized_value"], "0.125");
    assert_eq!(frames[3]["parameters"][0]["path"], paths[0]);
    for f in frames {
        assert_eq!(f["trace_id"], opened["trace_id"]);
        assert_eq!(f["topology_id"], opened["topology_id"]);
        assert_eq!(f["topology_name"], TOPOLOGY);
        assert!(is_uuid_v8(f["frame_id"].as_str().expect("frame_id")));
    }
    let mut ids = field("frame_id");
    ids.dedup();
    assert_eq!(ids.len(), 4);

    // a frame's patch is the candidate patch_at gives its values, drawn as the graph draws one
    let topology = known_topology(TOPOLOGY).expect("the topology");
    let mut first = vec![0.5; 8];
    first[0] = 0.5;
    let text = write_patch(&patch_at(&topology, &first).expect("a patch")).expect("written");
    assert_eq!(frames[0]["patch_id"], graph::patch_id(text.as_bytes()));
    assert_ne!(frames[0]["patch_id"], frames[1]["patch_id"]);

    assert_eq!(trace["closed"]["ended"], "Completed");
    assert_eq!(trace["closed"]["stop_reason"], "ImprovementBelowThreshold");
    assert_eq!(trace["closed"]["trace_id"], opened["trace_id"]);
}

#[test]
fn studio_fit_replay_best_worsened_in_a_trace_file_is_refused() {
    scenario("studio-fit-replay-best-worsened");
    let mut lines = replayed_lines();
    lines[2] = lines[2].replace("0.745 0.43", "0.745 0.645");
    let (name, message) = refused("worsened", &lines);
    assert_eq!(name, "patchback.studio.BestLossWorsened", "{message}");
    assert!(message.contains("sequence 3"), "{message}");
}

#[test]
fn studio_fit_replay_out_of_order_in_a_trace_file_is_refused() {
    scenario("studio-fit-replay-out-of-order");
    let mut lines = replayed_lines();
    lines[1] = lines[1].replace("evaluation 2 2", "evaluation 1 1");
    let (name, message) = refused("repeated", &lines);
    assert_eq!(name, "patchback.studio.FrameOutOfOrder", "{message}");
    assert!(message.contains("sequence 1"), "{message}");

    // a sequence below one already held, after a gap-free start
    let mut lines = replayed_lines();
    lines[3] = lines[3].replace("evaluation 4 4", "evaluation 2 2");
    let (name, message) = refused("lower", &lines);
    assert_eq!(name, "patchback.studio.FrameOutOfOrder", "{message}");
    assert!(message.contains("sequence 2"), "{message}");
}

/// A sequence above every held one but not the next is not out of order (the domain's
/// `out-of-order` needs a held sequence >= the input's); `trace.events` refuses the gap as its
/// grammar does, `trace`.
#[test]
fn a_gap_in_the_trace_file_is_its_grammar_error_not_out_of_order() {
    let mut lines = replayed_lines();
    lines[2] = lines[2].replace("evaluation 3 3", "evaluation 5 5");
    let (name, message) = refused("gap", &lines);
    assert_eq!(name, "trace", "{message}");
    assert!(message.contains("sequence numbers 1, 2, 3"), "{message}");
}

/// A frame of sequence 0, or of evaluation count 0, breaks a `TraceFrame` invariant
/// (`sequence >= 1`, `evaluation >= 1`): the exporter and the studio's loader both answer
/// `invariant`, whether or not a frame is held before it.
#[test]
fn a_frame_below_the_trace_frame_invariants_is_refused_as_invariant() {
    let mut lines = replayed_lines();
    lines[0] = lines[0].replace("evaluation 1 1", "evaluation 0 0");
    let (name, message) = refused("zero", &lines);
    assert_eq!(name, "invariant", "{message}");
    assert!(message.contains("sequence >= 1"), "{message}");

    let mut lines = replayed_lines();
    lines[2] = lines[2].replace("evaluation 3 3", "evaluation 0 0");
    let (name, message) = refused("zero-later", &lines);
    assert_eq!(name, "invariant", "{message}");
    assert!(message.contains("sequence >= 1"), "{message}");

    let mut lines = replayed_lines();
    lines[0] = lines[0].replace("evaluation 1 1", "evaluation 1 0");
    let (name, message) = refused("evaluation-zero", &lines);
    assert_eq!(name, "invariant", "{message}");
    assert!(message.contains("evaluation >= 1"), "{message}");
}

/// `patch.patch` is the retained patch of the trace's last retained evaluation, its values
/// compared as the patch file writes them; a run whose files do not belong together is `run`.
#[test]
fn a_patch_that_is_not_the_last_retained_candidate_is_refused() {
    let lines = replayed_lines();
    let text = trace_text(&lines, "ImprovementBelowThreshold");
    // the first retained candidate's patch, not the last one's
    let dir = run_dir(
        "earlier-patch",
        &common::fitted_patch(),
        Some(default_run_meta()),
        Some(&text),
    );
    let error = export_run(&dir).expect_err("the patch of evaluation 1");
    assert_eq!(error.name(), "run", "{error}");
    assert!(error.to_string().contains("attack"), "{error}");
    assert!(error.to_string().contains("sequence 2"), "{error}");

    // one value a step of the last digit away
    let patch = common::retained_patch(&lines);
    let line = patch
        .lines()
        .find(|l| l.contains(" output_gain "))
        .expect("the output gain")
        .to_owned();
    let value: f64 = line
        .rsplit(' ')
        .next()
        .expect("a value")
        .parse()
        .expect("a number");
    let moved = format!(
        "{} {}",
        line.rsplit_once(' ').expect("a value").0,
        patchback_core::patch::format_decimal(value.next_up())
    );
    let dir = run_dir(
        "one-ulp",
        &patch.replace(&line, &moved),
        Some(default_run_meta()),
        Some(&text),
    );
    let error = export_run(&dir).expect_err("a value one ulp away");
    assert_eq!(error.name(), "run", "{error}");
    assert!(error.to_string().contains("output_gain"), "{error}");

    // a trace that retained nothing, beside a patch file
    let rejected = vec![format!(
        "evaluation 1 1 0 rejected {TOPOLOGY} - - 0.5 0.5 0.5 0.5 0.5 0.5 0.5 0.5"
    )];
    for (name, lines) in [("nothing-retained", rejected), ("no-evaluation", vec![])] {
        let text = trace_text(&lines, "RenderBudgetExhausted");
        let dir = run_dir(
            name,
            &common::fitted_patch(),
            Some(default_run_meta()),
            Some(&text),
        );
        let error = export_run(&dir).expect_err("no retained candidate");
        assert_eq!(error.name(), "run", "{name}: {error}");
        assert!(
            error.to_string().contains("retains no candidate"),
            "{error}"
        );
    }
}

/// `studio/fixtures/export-replayed-lines.json` is the export of this test's run of four
/// evaluations, one of each outcome, a restart and a rejected candidate: the studio's tests draw
/// a rejected frame from it, as the bundled sample has none. `PATCHBACK_STUDIO_BLESS=1`
/// (`task studio-fixtures`) rewrites it.
#[test]
fn the_rejected_frame_fixture_is_the_export_of_the_replayed_lines() {
    let lines = replayed_lines();
    let text = trace_text(&lines, "ImprovementBelowThreshold");
    let dir = run_dir(
        "fixture",
        &common::retained_patch(&lines),
        Some(default_run_meta()),
        Some(&text),
    );
    let export = export_run(&dir).expect("the run exports");
    let path = repository_root().join("studio/fixtures/export-replayed-lines.json");
    if std::env::var_os("PATCHBACK_STUDIO_BLESS").is_some() {
        fs::write(&path, &export).expect("write the fixture");
    }
    let fixture = fs::read_to_string(&path).expect("the fixture");
    assert!(
        fixture == export,
        "{} is not the export of the replayed lines; run task studio-fixtures",
        path.display()
    );
    let document = json(&fixture);
    let outcomes: Vec<&str> = document["trace"]["frames"]
        .as_array()
        .expect("frames")
        .iter()
        .map(|f| f["outcome"].as_str().expect("an outcome"))
        .collect();
    assert_eq!(
        outcomes,
        ["FirstRetained", "Improved", "NotImproved", "Rejected"]
    );
}

#[test]
fn studio_fit_replay_inconsistent_frame_in_a_trace_file_is_refused() {
    scenario("studio-fit-replay-inconsistent-frame");
    // a rejected candidate that carries a loss
    let mut lines = replayed_lines();
    lines[3] = lines[3].replace(
        "rejected subtractive-mono/1 - 0.43",
        "rejected subtractive-mono/1 0.9 0.43",
    );
    let (name, message) = refused("rejected-loss", &lines);
    assert_eq!(name, "patchback.studio.InconsistentFrame", "{message}");
    assert!(message.contains("sequence 4"), "{message}");
    // an improving candidate whose loss is not the best loss
    let mut lines = replayed_lines();
    lines[1] = lines[1].replace("0.43 0.43", "0.43 0.44");
    let (name, message) = refused("improved-not-best", &lines);
    assert_eq!(name, "patchback.studio.InconsistentFrame", "{message}");
    assert!(message.contains("sequence 2"), "{message}");
    // a retained candidate without a best loss
    let mut lines = replayed_lines();
    lines[0] = lines[0].replace("0.645 0.645", "0.645 -");
    let (name, message) = refused("retained-no-best", &lines);
    assert_eq!(name, "patchback.studio.InconsistentFrame", "{message}");
}

#[test]
fn a_run_without_its_trace_or_meta_or_of_another_topology_is_refused() {
    let dir = run_dir(
        "no-trace",
        &common::fitted_patch(),
        Some(default_run_meta()),
        None,
    );
    fs::remove_file(dir.join("trace.events")).expect("remove the default trace");
    let error = export_run(&dir).expect_err("no trace.events");
    assert_eq!(error.name(), "run", "{error}");

    let dir = run_dir("no-meta", &common::fitted_patch(), None, None);
    fs::remove_file(dir.join("run.meta")).expect("remove the default run.meta");
    assert_eq!(export_run(&dir).expect_err("no run.meta").name(), "run");

    let text = trace_text(&replayed_lines(), "ImprovementBelowThreshold")
        .replace("patchback-fit-trace 1", "patchback-fit-trace 2");
    let dir = run_dir(
        "version",
        &common::fitted_patch(),
        Some(default_run_meta()),
        Some(&text),
    );
    let error = export_run(&dir).expect_err("version 2");
    assert_eq!(error.name(), "trace", "{error}");

    let text = trace_text(&replayed_lines(), "ImprovementBelowThreshold")
        .replace(" drive output_gain", " drive_gain output_gain");
    let dir = run_dir(
        "paths",
        &common::fitted_patch(),
        Some(default_run_meta()),
        Some(&text),
    );
    let error = export_run(&dir).expect_err("another path list");
    assert_eq!(error.name(), "run", "{error}");
}

#[test]
fn the_fit_run_identity_ignores_the_patchback_version_and_nothing_else() {
    let meta = default_run_meta();
    let other_version = meta.replace("patchback 0.11.0", "patchback 9.9.9");
    let other_seed = meta.replace("seed 0", "seed 1");
    let text = trace_text(&replayed_lines(), "ImprovementBelowThreshold");
    let id = |name: &str, meta: &str| {
        let dir = run_dir(
            name,
            &common::retained_patch(&replayed_lines()),
            Some(meta),
            Some(&text),
        );
        json(&export_run(&dir).expect("exports"))["trace"]["opened"]["fit_run_id"].clone()
    };
    let base = id("meta-base", meta);
    assert_eq!(id("meta-version", &other_version), base);
    assert_ne!(id("meta-seed", &other_seed), base);
}

/// The bundled sample is a real fit's export (Dc1-23; `task studio-fixtures-check` holds it to
/// a fresh run): its frames are in order, the best loss never rises and a retained frame's loss
/// is the best (DESIGN.md 9.2, the story's numeric obligations).
#[test]
fn the_bundled_sample_is_a_consistent_replay_of_a_real_fit() {
    let text = fs::read_to_string(repository_root().join("studio/fixtures/sample-run.json"))
        .expect("the bundled sample");
    let document = json(&text);
    let frames = document["trace"]["frames"].as_array().expect("frames");
    assert!(frames.len() >= 100, "{}", frames.len());
    let mut best = f64::INFINITY;
    for (at, f) in frames.iter().enumerate() {
        assert_eq!(f["sequence"], at as u64 + 1);
        let decimal = |v: &Value| v.as_str().map(|s| s.parse::<f64>().expect("a decimal"));
        if let Some(b) = decimal(&f["best_loss"]) {
            assert!(b <= best, "frame {}", at + 1);
            best = b;
        }
        if matches!(f["outcome"].as_str(), Some("FirstRetained" | "Improved")) {
            assert_eq!(f["total_loss"], f["best_loss"], "frame {}", at + 1);
        }
        if f["outcome"] == "Rejected" {
            assert!(f.get("total_loss").is_none());
        }
    }
    assert_eq!(document["trace"]["closed"]["ended"], "Completed");
    assert!(document["trace"]["closed"]["stop_reason"].is_string());
}
