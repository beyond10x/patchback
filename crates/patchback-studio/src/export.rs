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
//! Decimals are strings, ESS `Decimal`s: the shortest plain digits that read back to the same `f64`
//! ([`crate::decimal`], never an exponent form). Identities are version-8 UUIDs ([`crate::id`]):
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
//! 4. `run`: `run.meta` or `trace.events` is missing or unreadable, or the trace's `topology`
//!    line (line 2, after a header `parse_trace` reads) names another topology or parameter list
//!    than the patch's. This is read before the grammar of the frames: a trace of another
//!    topology is `run` whatever its later lines hold.
//! 5. [`INVARIANT`], `invariant`: an `evaluation` line whose sequence is below 1, evaluation
//!    count below 1 or restart below 0 breaks a `TraceFrame` invariant; the studio's loader
//!    gives a frame that does so the same name.
//! 6. `patchback.studio.InconsistentFrame`, `patchback.studio.FrameOutOfOrder`,
//!    `patchback.studio.BestLossWorsened`: the `evaluation` lines, read as frames through the
//!    grammar's own tokenizer (`patchback_fit::trace_lines`) up to the first that does not read
//!    as one, are appended to the domain model ([`crate::replay`]), which answers `AppendFrame`
//!    in the domain's order: a rejected candidate with a loss, a valid one without a loss or a
//!    retained one whose loss is not its best before a sequence not above every held one, and
//!    that before a best loss that rises.
//! 7. `trace`: `patchback_fit::parse_trace` refuses the text (header, version, shape, number,
//!    length, an unclosed trace, a best loss lost or given before the first retained candidate,
//!    and a sequence above every held one that is not the next: the grammar numbers lines
//!    without gaps, which the domain does not ask). No refusal before this one rests on the
//!    parser's message.
//! 8. `run`: `patch.patch` is not the patch of the trace's last retained candidate, compared
//!    value by value as the patch file writes them, or the trace retains none: the run's files
//!    do not belong together. A trace without evaluations is a valid replay, but no completed
//!    fit run writes one beside a patch file, so none exports.
//! 9. `output`: the output file exists or cannot be written ([`write_export`]).

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::Path;

use patchback_core::parameter::Topology;
use patchback_core::patch::{
    Patch, format_decimal, parse_decimal, read_patch, validate, write_patch,
};
use patchback_fit::{
    EvaluationOutcome, LossComponents, PATCH_FILE, RUN_META_FILE, TRACE_FILE, TRACE_FORMAT_VERSION,
    TRACE_HEADER, TRACE_MARKER, TRACE_TOPOLOGY, Trace, TraceLine, parse_trace, patch_at,
    trace_lines,
};
use serde_json::{Value, json};

use crate::StudioError;
use crate::graph;
use crate::id;
use crate::replay::{Frame, Opened, Replay, TraceEnd};

/// The `format` member of the document.
pub const FORMAT: &str = "patchback-studio-export/1";

/// The name of a refusal for a frame that breaks a `TraceFrame` invariant; the studio's loader
/// answers the same input with the same name (`studio/src/document.ts`).
pub const INVARIANT: &str = "invariant";
/// The trace identity a refusal of a `trace.events` line names: the trace's own is not known
/// before the trace parses.
const OF_TRACE_FILE: &str = "of trace.events";
/// The fields of an `evaluation` line the `TraceFrame` invariants bound, by position: the
/// field, its least value.
const BOUNDED_FIELDS: [(usize, &str, i64); 3] =
    [(1, "sequence", 1), (2, "evaluation", 1), (3, "restart", 0)];

/// A refused export: the error it is an instance of and what was refused.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportError {
    name: &'static str,
    message: String,
}

impl ExportError {
    /// The `patchback.studio` error, or `run`, [`INVARIANT`], `trace` or `output` (module
    /// documentation).
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
    let paths: Vec<&str> = topology
        .contracts()
        .iter()
        .map(|c| c.path.as_str())
        .collect();
    let other_run = |name: &str, trace_paths: &[&str]| {
        run(format!(
            "{TRACE_FILE} is of topology {name} with parameters {}; {PATCH_FILE} is of {} with {}",
            trace_paths.join(" "),
            patch.topology_id,
            paths.join(" ")
        ))
    };
    let lines = trace_lines(&trace_text);
    if let Some((name, trace_paths)) = trace_head(&lines) {
        if name != patch.topology_id || trace_paths != paths {
            return Err(other_run(name, &trace_paths));
        }
        check_frames(&lines[2..], &trace_paths)?;
    }
    let trace = parse_trace(&trace_text)
        .map_err(|error| ExportError::other("trace", format!("{TRACE_FILE}: {error}")))?;
    let trace_paths: Vec<&str> = trace.paths.iter().map(String::as_str).collect();
    if trace.topology != patch.topology_id || trace_paths != paths {
        return Err(other_run(&trace.topology, &trace_paths));
    }

    let fit_run_id = id::derive(id::FIT_RUN, &[&identity_meta(&meta), &trace_bytes, &bytes]);
    let replay = replay(&trace, topology, &fit_run_id, &drawing.topology_id)?;
    check_retained(&patch, &trace, topology)?;
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

/// The topology and parameter paths of a trace's `topology` line (refusal 4), when line 1 is
/// the header of the version `parse_trace` reads and line 2 is a `topology` line; otherwise
/// none, and the grammar answers (refusal 7).
fn trace_head<'a>(lines: &[TraceLine<'a>]) -> Option<(&'a str, Vec<&'a str>)> {
    let version = TRACE_FORMAT_VERSION.to_string();
    let [header, topology, ..] = lines else {
        return None;
    };
    if header.fields() != [TRACE_HEADER, version.as_str()] {
        return None;
    }
    match topology.fields()[..] {
        [TRACE_TOPOLOGY, name, ref paths @ ..] => Some((name, paths.to_vec())),
        _ => None,
    }
}

/// Refusals 5 and 6 (module documentation): the `evaluation` lines after the head, read through
/// the grammar's tokenizer as the domain's frames, in order, up to the first line that does not
/// read as one (the grammar answers it, refusal 7). Every frame read is held to the
/// `TraceFrame` invariants first, as the studio's loader holds a document's frames before it
/// replays them; then the frames are appended to a replay of the domain model
/// ([`crate::replay`]), which answers `AppendFrame`'s refusals in the domain's order.
fn check_frames(lines: &[TraceLine<'_>], paths: &[&str]) -> Result<(), ExportError> {
    let mut frames = Vec::new();
    for line in lines.iter().filter(|line| line.is_evaluation()) {
        match read_frame(line, paths)? {
            Some(frame) => frames.push(frame),
            None => break,
        }
    }
    let mut model = Replay::open(Opened {
        trace_id: OF_TRACE_FILE.to_owned(),
        fit_run_id: String::new(),
        topology_id: String::new(),
        topology_name: String::new(),
        parameter_paths: paths.iter().map(|p| (*p).to_owned()).collect(),
    })?;
    for frame in frames {
        model.append(frame)?;
    }
    Ok(())
}

/// One `evaluation` line as the frame it states, or none when a field does not read: an integer
/// field below its `TraceFrame` bound is [`INVARIANT`], checked in field order as it is read.
fn read_frame(line: &TraceLine<'_>, paths: &[&str]) -> Result<Option<Frame>, ExportError> {
    let fields = line.fields();
    let mut integers = [0_i64; 3];
    for (at, (position, field, least)) in BOUNDED_FIELDS.into_iter().enumerate() {
        let Some(value) = fields.get(position).and_then(|t| t.parse::<i64>().ok()) else {
            return Ok(None);
        };
        if value < least {
            return Err(ExportError::other(
                INVARIANT,
                format!(
                    "patchback.studio.TraceFrame {field} >= {least}: {TRACE_FILE} line {}: \
                     {field} is {value}",
                    line.line
                ),
            ));
        }
        integers[at] = value;
    }
    let [sequence, evaluation, restart] = integers;
    let (Ok(sequence), Ok(evaluation), Ok(restart)) = (
        u64::try_from(sequence),
        u64::try_from(evaluation),
        u32::try_from(restart),
    ) else {
        return Ok(None);
    };
    let Some(outcome) = fields.get(4).and_then(|token| {
        [
            EvaluationOutcome::FirstRetained,
            EvaluationOutcome::Improved,
            EvaluationOutcome::NotImproved,
            EvaluationOutcome::Rejected,
        ]
        .into_iter()
        .find(|o| o.name() == *token)
    }) else {
        return Ok(None);
    };
    let losses = fields.len().checked_sub(6 + 1 + paths.len());
    let number = |token: &str| -> Option<Option<f64>> {
        if token == TRACE_MARKER {
            Some(None)
        } else {
            parse_decimal(token).map(Some)
        }
    };
    let (components, total) = match losses {
        Some(1) => match number(fields[6]) {
            Some(total) => (None, total),
            None => return Ok(None),
        },
        Some(5) => {
            let read: Option<Vec<Option<f64>>> = fields[6..11].iter().map(|t| number(t)).collect();
            match read.as_deref() {
                Some(
                    &[
                        Some(log_spectral),
                        Some(envelope),
                        Some(transient),
                        Some(band_energy),
                        Some(total),
                    ],
                ) => (
                    Some(LossComponents {
                        log_spectral,
                        envelope,
                        transient,
                        band_energy,
                    }),
                    Some(total),
                ),
                Some(&[None, None, None, None, None]) => (None, None),
                _ => return Ok(None),
            }
        }
        _ => return Ok(None),
    };
    let best_at = fields.len() - paths.len() - 1;
    let Some(best) = number(fields[best_at]) else {
        return Ok(None);
    };
    let values: Option<Vec<f64>> = fields[best_at + 1..]
        .iter()
        .map(|t| parse_decimal(t))
        .collect();
    let Some(values) = values else {
        return Ok(None);
    };
    Ok(Some(Frame {
        frame_id: String::new(),
        sequence,
        evaluation,
        restart,
        outcome,
        topology_id: String::new(),
        topology_name: fields[5].to_owned(),
        patch_id: None,
        total,
        components,
        best,
        parameters: paths.iter().map(|p| (*p).to_owned()).zip(values).collect(),
    }))
}

/// Refusal 8 (module documentation): `patch.patch` holds the values of the trace's last retained
/// candidate, each compared as the patch file writes it (`format_decimal`), node by node.
fn check_retained(patch: &Patch, trace: &Trace, topology: &Topology) -> Result<(), ExportError> {
    let run = |message: String| ExportError::other("run", message);
    let Some(last) = trace.records.iter().rev().find(|r| r.outcome.retained()) else {
        return Err(run(format!(
            "{TRACE_FILE} retains no candidate, yet {PATCH_FILE} holds a patch"
        )));
    };
    let retained = patch_at(topology, &last.normalized).map_err(|error| {
        ExportError::other(
            "trace",
            format!("evaluation {} is not a patch: {error}", last.sequence),
        )
    })?;
    let values = |p: &Patch| -> Vec<(u32, String, String)> {
        let mut out: Vec<(u32, String, String)> = p
            .nodes
            .iter()
            .flat_map(|node| {
                node.parameters
                    .iter()
                    .map(move |v| (node.id, v.name.clone(), format_decimal(v.value)))
            })
            .collect();
        out.sort();
        out
    };
    let (held, want) = (values(patch), values(&retained));
    if held == want {
        return Ok(());
    }
    let lookup = |list: &[(u32, String, String)], node: u32, name: &str| -> String {
        list.iter()
            .find(|(n, p, _)| *n == node && p == name)
            .map_or_else(|| "absent".to_owned(), |(_, _, v)| v.clone())
    };
    let differing = want
        .iter()
        .chain(&held)
        .find(|entry| !held.contains(entry) || !want.contains(entry))
        .map_or_else(String::new, |(node, name, _)| {
            format!(
                ": {name} of node {node} is {} in {PATCH_FILE}, {} in the trace",
                lookup(&held, *node, name),
                lookup(&want, *node, name)
            )
        });
    Err(run(format!(
        "{PATCH_FILE} is not the patch of the last retained evaluation, sequence {}{differing}",
        last.sequence
    )))
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
