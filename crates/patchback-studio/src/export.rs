//! `patchback-studio export --run <dir> --out <file>`: the document Patchback Studio loads,
//! made from a fit run's files through Patchback's own readers (approval-record:wave-c1,
//! Dc1-21). The studio parses neither `.patch` nor `trace.events`; it reads this document.
//!
//! # The document
//!
//! One JSON object, written compactly with its keys in byte order and a final `\n`, so two
//! exports of one run are equal bytes. Every member but `format` and `topology` has the shape of
//! a type `ess generate types` makes from `spec/domains/studio.yaml` into `studio/src/generated/`:
//!
//! | Key | Value |
//! |---|---|
//! | `format` | [`FORMAT`], `patchback-studio-export/1` |
//! | `topology` | the topology's name, as the patch file writes it |
//! | `graph` | `GraphDrawn`: the run's `patch.patch` drawn ([`crate::graph`]) |
//! | `modules` | `ModuleSpec` per drawn kind: its ports, rates and parameter contracts |
//! | `trace` | `{opened, frames, closed}`: `TraceOpened`, one `FrameAppended` per `evaluation` line of `trace.events`, `TraceClosedEvent` ([`crate::replay`]) |
//!
//! Decimals are strings, written in the shortest form that reads back to the same `f64`
//! (`patchback_core::patch::format_decimal`). Identities are version-8 UUIDs ([`crate::id`]):
//! a patch's of its `.patch` text, the topology's of its name, the fit run's of `run.meta`
//! without its `patchback <version>` line, `trace.events` and `patch.patch`, so a release that
//! only renames the version keeps a run's identity; the trace's of the run's, a frame's of the
//! trace's and its sequence. A frame of a valid candidate carries the identity of the patch
//! `patchback_fit::patch_at` makes of its values; a rejected one carries none.
//!
//! A frame's sequence is the `evaluation` line's `<sequence>`, from 1, and the replay closes
//! `Completed` with the closing line's stop reason: a failed run writes no `trace.events`
//! (story:fit-run-artifacts), so no replay closes `Failed`.
//!
//! # Refusals
//!
//! Each is an [`ExportError`], answered in this order, and nothing is written:
//!
//! 1. `patchback.studio.PatchNotDrawable`: `patch.patch` is missing or unreadable, or its parser
//!    refuses it; the message names the `patchback.patch` error.
//! 2. `patchback.studio.GraphEndpointMissing`, `patchback.studio.FeedbackNotAtDelay`: the
//!    drawing of the patch file as parsed ([`crate::graph::check`]); the domain answers them
//!    before it asks whether the patch is validated.
//! 3. `patchback.studio.PatchNotDrawable`: graph validation (`patchback_core::patch::validate`)
//!    refuses the patch.
//! 4. `run`: `run.meta` or `trace.events` is missing or unreadable, or the trace is of another
//!    topology or parameter list than the patch's.
//! 5. `patchback.studio.FrameOutOfOrder`, `patchback.studio.BestLossWorsened`,
//!    `patchback.studio.InconsistentFrame`: `patchback_fit::parse_trace` refuses an evaluation
//!    line for the reason the domain names (a sequence out of order; a best loss that rises; a
//!    rejected candidate with a loss, a retained one without a best loss, a best loss lost or
//!    before the first retained), or the replay refuses a frame ([`crate::replay`]: a retained
//!    candidate whose loss is not the best).
//! 6. `trace`: `parse_trace` refuses the text for any other reason (header, version, shape,
//!    number, length, an unclosed trace).
//! 7. `output`: the output file exists or cannot be written ([`write_export`]).

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::Path;

use patchback_core::parameter::Topology;
use patchback_core::patch::{read_patch, validate, write_patch};
use patchback_fit::{
    EvaluationOutcome, PATCH_FILE, RUN_META_FILE, TRACE_FILE, Trace, TraceError, TraceReason,
    parse_trace, patch_at,
};
use serde_json::{Value, json};

use crate::StudioError;
use crate::graph;
use crate::id;
use crate::replay::{Frame, Opened, Replay, TraceEnd};

/// The `format` member of the document.
pub const FORMAT: &str = "patchback-studio-export/1";

/// The `parse_trace` expectation of a line whose sequence is not the next one.
const SEQUENCE_ORDER: &str = "sequence numbers 1, 2, 3 and on, in order";
/// The `parse_trace` expectation of a rejected line that carries a loss.
const REJECTED_LOSSES: &str = "`-` for the losses of a rejected candidate";
/// The lines of `trace.events` before its first `evaluation` line.
const TRACE_HEAD_LINES: usize = 2;

/// A refused export: the error it is an instance of and what was refused.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportError {
    name: &'static str,
    message: String,
}

impl ExportError {
    /// The `patchback.studio` error, or `run`, `trace` or `output` (module documentation).
    pub fn name(&self) -> &'static str {
        self.name
    }

    fn studio(error: StudioError, detail: impl fmt::Display) -> Self {
        Self {
            name: error.name(),
            message: format!("{error}: {detail}"),
        }
    }

    fn other(name: &'static str, message: String) -> Self {
        Self { name, message }
    }
}

impl From<StudioError> for ExportError {
    fn from(error: StudioError) -> Self {
        Self {
            name: error.name(),
            message: error.to_string(),
        }
    }
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.message)
    }
}

impl std::error::Error for ExportError {}

fn read(dir: &Path, file: &str) -> Result<Vec<u8>, String> {
    let path = dir.join(file);
    fs::read(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

/// The document of the fit run in `dir` (module documentation).
pub fn export_run(dir: &Path) -> Result<String, ExportError> {
    let bytes = read(dir, PATCH_FILE).map_err(|message| {
        ExportError::studio(
            StudioError::PatchNotDrawable {
                patch_id: String::new(),
            },
            message,
        )
    })?;
    let not_drawable = || StudioError::PatchNotDrawable {
        patch_id: graph::patch_id(&bytes),
    };
    let patch = read_patch(&bytes[..]).map_err(|error| {
        let name = match &error {
            patchback_core::patch::ReadPatchError::Format(e) => e.reason.spec_error(),
            patchback_core::patch::ReadPatchError::Io(_) => "io",
        };
        ExportError::studio(not_drawable(), format!("{name}: {error}"))
    })?;
    let drawing = graph::drawing(&patch, &bytes);
    graph::check(&drawing)?;
    let validated = validate(&patch).map_err(|error| {
        ExportError::studio(not_drawable(), format!("{}: {error}", error.spec_error()))
    })?;
    let topology = validated.topology();

    let run = |message: String| ExportError::other("run", message);
    let meta = read(dir, RUN_META_FILE).map_err(run)?;
    let trace_bytes = read(dir, TRACE_FILE).map_err(run)?;
    let trace_text = String::from_utf8(trace_bytes.clone())
        .map_err(|_| ExportError::other("trace", format!("{TRACE_FILE} is not UTF-8")))?;
    let trace = parse_trace(&trace_text).map_err(|error| trace_refusal(&trace_text, &error))?;
    let paths: Vec<&str> = topology
        .contracts()
        .iter()
        .map(|c| c.path.as_str())
        .collect();
    if trace.topology != patch.topology_id || trace.paths != paths {
        return Err(run(format!(
            "{TRACE_FILE} is of topology {} with parameters {}; {PATCH_FILE} is of {} with {}",
            trace.topology,
            trace.paths.join(" "),
            patch.topology_id,
            paths.join(" ")
        )));
    }

    let fit_run_id = id::derive(id::FIT_RUN, &[&identity_meta(&meta), &trace_bytes, &bytes]);
    let replay = replay(&trace, topology, &fit_run_id, &drawing.topology_id)?;
    let document: Value = json!({
        "format": FORMAT,
        "topology": patch.topology_id,
        "graph": drawing.to_json(),
        "modules": graph::modules(&drawing, topology),
        "trace": replay.to_json(),
    });
    let mut text = serde_json::to_string(&document).map_err(|error| {
        ExportError::other("output", format!("cannot encode the document: {error}"))
    })?;
    text.push('\n');
    Ok(text)
}

/// `run.meta` without its `patchback <version>` line, which every release changes.
fn identity_meta(meta: &[u8]) -> Vec<u8> {
    let mut kept = Vec::with_capacity(meta.len());
    for line in meta.split_inclusive(|b| *b == b'\n') {
        if !line.starts_with(b"patchback ") {
            kept.extend_from_slice(line);
        }
    }
    kept
}

/// The replay of a parsed trace: opened, one frame per record, closed with its stop reason.
fn replay(
    trace: &Trace,
    topology: &Topology,
    fit_run_id: &str,
    topology_id: &str,
) -> Result<Replay, ExportError> {
    let trace_id = id::derive(id::TRACE, &[fit_run_id.as_bytes()]);
    let mut replay = Replay::open(Opened {
        trace_id: trace_id.clone(),
        fit_run_id: fit_run_id.to_owned(),
        topology_id: topology_id.to_owned(),
        topology_name: trace.topology.clone(),
        parameter_paths: trace.paths.clone(),
    })?;
    for record in &trace.records {
        let patch_id = if record.outcome == EvaluationOutcome::Rejected {
            None
        } else {
            let text = patch_at(topology, &record.normalized)
                .map_err(|error| error.to_string())
                .and_then(|patch| write_patch(&patch).map_err(|error| error.to_string()))
                .map_err(|error| {
                    ExportError::other(
                        "trace",
                        format!("evaluation {} is not a patch: {error}", record.sequence),
                    )
                })?;
            Some(graph::patch_id(text.as_bytes()))
        };
        replay.append(Frame {
            frame_id: id::derive(
                id::FRAME,
                &[trace_id.as_bytes(), record.sequence.to_string().as_bytes()],
            ),
            sequence: record.sequence,
            evaluation: record.evaluations,
            restart: record.restart,
            outcome: record.outcome,
            topology_id: topology_id.to_owned(),
            topology_name: record.topology.clone(),
            patch_id,
            total: record.total,
            components: record.components,
            best: record.best_after,
            parameters: trace
                .paths
                .iter()
                .cloned()
                .zip(record.normalized.iter().copied())
                .collect(),
        })?;
    }
    replay.close(TraceEnd::Completed, Some(trace.stop_reason))?;
    Ok(replay)
}

/// The export's answer to a refused `trace.events` (module documentation, refusal 5 and 6).
/// The trace identity is not known yet, so the error names the line and the sequence it holds.
fn trace_refusal(text: &str, error: &TraceError) -> ExportError {
    let sequence = || {
        error
            .line
            .and_then(|line| text.lines().nth(line - 1))
            .and_then(|line| line.split_ascii_whitespace().nth(1))
            .and_then(|token| token.parse::<u64>().ok())
            .or_else(|| {
                error
                    .line
                    .map(|line| line.saturating_sub(TRACE_HEAD_LINES) as u64)
            })
            .unwrap_or(0)
    };
    let trace_id = format!("of {TRACE_FILE}");
    let studio = match &error.reason {
        TraceReason::MalformedRecord { expected } if *expected == SEQUENCE_ORDER => {
            Some(StudioError::FrameOutOfOrder {
                trace_id,
                sequence: sequence(),
            })
        }
        TraceReason::BestLossRose { .. } => Some(StudioError::BestLossWorsened {
            trace_id,
            sequence: sequence(),
        }),
        TraceReason::MalformedRecord { expected } if *expected == REJECTED_LOSSES => {
            Some(StudioError::InconsistentFrame {
                trace_id,
                sequence: sequence(),
            })
        }
        TraceReason::RetainedWithoutBestLoss { .. }
        | TraceReason::BestLossLost { .. }
        | TraceReason::BestLossBeforeRetained { .. } => Some(StudioError::InconsistentFrame {
            trace_id,
            sequence: sequence(),
        }),
        _ => None,
    };
    match studio {
        Some(studio) => ExportError::studio(studio, error),
        None => ExportError::other("trace", format!("{TRACE_FILE}: {error}")),
    }
}

/// Writes the document of the run in `dir` to `out`, a file that must not exist yet.
pub fn write_export(dir: &Path, out: &Path) -> Result<(), ExportError> {
    let text = export_run(dir)?;
    let output = |message: String| ExportError::other("output", message);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out)
        .map_err(|error| output(format!("cannot create {}: {error}", out.display())))?;
    if let Err(error) = file
        .write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
    {
        drop(file);
        let _ = fs::remove_file(out);
        return Err(output(format!("cannot write {}: {error}", out.display())));
    }
    Ok(())
}
