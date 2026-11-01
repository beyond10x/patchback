//! A fit run's evolution trace as the studio replays it (`spec/domains/studio.yaml`
//! `EvolutionTrace`, `TraceFrame`; DESIGN.md § 16 fit replay): [`Replay::open`],
//! [`Replay::append`] and [`Replay::close`] answer `OpenTrace`, `AppendFrame` and `CloseTrace`,
//! and [`Replay::to_json`] gives the events a replay emits, `TraceOpened`, one `FrameAppended`
//! per frame and `TraceClosedEvent`, as the studio's generated types read them.
//!
//! # Refusals
//!
//! In the domain's precedence order, the first that applies. `AppendFrame`: a rejected frame
//! with a loss, a valid frame without one, a `FirstRetained` or `Improved` frame whose loss is
//! not the best loss ([`StudioError::InconsistentFrame`]); a closed trace
//! ([`StudioError::TraceClosed`]); a sequence not above every sequence the trace holds
//! ([`StudioError::FrameOutOfOrder`]); a best loss above an earlier frame's
//! ([`StudioError::BestLossWorsened`]). `CloseTrace`: a completed run without a stop reason or a
//! failed run with one ([`StudioError::InconsistentEnd`]), then a closed trace
//! ([`StudioError::TraceStateConflict`]). `OpenTrace`: a parameter path named twice
//! ([`StudioError::DuplicateParameterPath`]). A refused command changes nothing.
//!
//! Losses are compared as the `f64` values the trace's decimals read back to, bit for bit
//! (`patchback_core::patch::format_decimal` writes the shortest form that reads back).

use std::collections::BTreeSet;

use patchback_core::patch::format_decimal;
use patchback_fit::{EvaluationOutcome, LossComponents, StopReason};
use serde_json::{Map, Value, json};

use crate::StudioError;

/// How the fit run a trace follows ended (`TraceEnd`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceEnd {
    Completed,
    Failed,
}

impl TraceEnd {
    /// The variant's name in the specification.
    pub fn name(self) -> &'static str {
        match self {
            Self::Completed => "Completed",
            Self::Failed => "Failed",
        }
    }
}

/// `OpenTrace`'s input for a replay (`TraceSource` `Replay`).
#[derive(Clone, Debug, PartialEq)]
pub struct Opened {
    pub trace_id: String,
    pub fit_run_id: String,
    pub topology_id: String,
    pub topology_name: String,
    pub parameter_paths: Vec<String>,
}

/// `AppendFrame`'s input: one evaluation of the run. `parameters` are the candidate's
/// normalized values by path.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub frame_id: String,
    pub sequence: u64,
    pub evaluation: u64,
    pub restart: u32,
    pub outcome: EvaluationOutcome,
    pub topology_id: String,
    pub topology_name: String,
    pub patch_id: Option<String>,
    pub total: Option<f64>,
    pub components: Option<LossComponents>,
    pub best: Option<f64>,
    pub parameters: Vec<(String, f64)>,
}

/// A trace being replayed: `Streaming` until [`Replay::close`], then `Closed`.
#[derive(Clone, Debug, PartialEq)]
pub struct Replay {
    opened: Opened,
    frames: Vec<Frame>,
    closed: Option<(TraceEnd, Option<StopReason>)>,
}

/// The name `FrameOutcome` gives an outcome.
fn outcome_name(outcome: EvaluationOutcome) -> &'static str {
    match outcome {
        EvaluationOutcome::FirstRetained => "FirstRetained",
        EvaluationOutcome::Improved => "Improved",
        EvaluationOutcome::NotImproved => "NotImproved",
        EvaluationOutcome::Rejected => "Rejected",
    }
}

impl Replay {
    /// `OpenTrace`.
    pub fn open(opened: Opened) -> Result<Self, StudioError> {
        let mut seen = BTreeSet::new();
        if !opened.parameter_paths.iter().all(|p| seen.insert(p)) {
            return Err(StudioError::DuplicateParameterPath {
                trace_id: opened.trace_id,
            });
        }
        Ok(Self {
            opened,
            frames: Vec::new(),
            closed: None,
        })
    }

    /// `AppendFrame`.
    pub fn append(&mut self, frame: Frame) -> Result<(), StudioError> {
        let trace_id = || self.opened.trace_id.clone();
        let sequence = frame.sequence;
        let rejected = frame.outcome == EvaluationOutcome::Rejected;
        let retained = matches!(
            frame.outcome,
            EvaluationOutcome::FirstRetained | EvaluationOutcome::Improved
        );
        let same = |a: Option<f64>, b: Option<f64>| match (a, b) {
            (Some(a), Some(b)) => a.to_bits() == b.to_bits(),
            (None, None) => true,
            _ => false,
        };
        if (rejected && frame.total.is_some())
            || (!rejected && frame.total.is_none())
            || (retained && !same(frame.total, frame.best))
        {
            return Err(StudioError::InconsistentFrame {
                trace_id: trace_id(),
                sequence,
            });
        }
        if self.closed.is_some() {
            return Err(StudioError::TraceClosed {
                trace_id: trace_id(),
            });
        }
        if self.frames.iter().any(|f| f.sequence >= sequence) {
            return Err(StudioError::FrameOutOfOrder {
                trace_id: trace_id(),
                sequence,
            });
        }
        if let Some(best) = frame.best
            && self
                .frames
                .iter()
                .any(|f| f.best.is_some_and(|earlier| earlier < best))
        {
            return Err(StudioError::BestLossWorsened {
                trace_id: trace_id(),
                sequence,
            });
        }
        self.frames.push(frame);
        Ok(())
    }

    /// `CloseTrace`.
    pub fn close(
        &mut self,
        ended: TraceEnd,
        stop_reason: Option<StopReason>,
    ) -> Result<(), StudioError> {
        if (ended == TraceEnd::Completed) != stop_reason.is_some() {
            return Err(StudioError::InconsistentEnd {
                trace_id: self.opened.trace_id.clone(),
            });
        }
        if self.closed.is_some() {
            return Err(StudioError::TraceStateConflict {
                trace_id: self.opened.trace_id.clone(),
            });
        }
        self.closed = Some((ended, stop_reason));
        Ok(())
    }

    /// The frames appended so far, in order.
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    /// The replay's events: `{opened, frames, closed}`, `closed` null while streaming. An
    /// absent optional field is left out, as the generated types declare it optional.
    pub fn to_json(&self) -> Value {
        let o = &self.opened;
        let opened = json!({
            "trace_id": o.trace_id,
            "fit_run_id": o.fit_run_id,
            "source": "Replay",
            "topology_id": o.topology_id,
            "topology_name": o.topology_name,
            "parameter_paths": o.parameter_paths,
        });
        let frames: Vec<Value> = self.frames.iter().map(|f| self.frame_json(f)).collect();
        let closed = self.closed.map_or(Value::Null, |(ended, reason)| {
            let mut closed = Map::new();
            closed.insert("trace_id".into(), json!(o.trace_id));
            closed.insert("ended".into(), json!(ended.name()));
            if let Some(reason) = reason {
                closed.insert("stop_reason".into(), json!(reason.name()));
            }
            Value::Object(closed)
        });
        json!({ "opened": opened, "frames": frames, "closed": closed })
    }

    fn frame_json(&self, f: &Frame) -> Value {
        let mut frame = Map::new();
        frame.insert("frame_id".into(), json!(f.frame_id));
        frame.insert("trace_id".into(), json!(self.opened.trace_id));
        frame.insert("sequence".into(), json!(f.sequence));
        frame.insert("evaluation".into(), json!(f.evaluation));
        frame.insert("restart".into(), json!(f.restart));
        frame.insert("outcome".into(), json!(outcome_name(f.outcome)));
        frame.insert("topology_id".into(), json!(f.topology_id));
        frame.insert("topology_name".into(), json!(f.topology_name));
        if let Some(patch_id) = &f.patch_id {
            frame.insert("patch_id".into(), json!(patch_id));
        }
        if let Some(total) = f.total {
            frame.insert("total_loss".into(), json!(format_decimal(total)));
        }
        if let Some(c) = f.components {
            frame.insert(
                "components".into(),
                json!({
                    "log_spectral": format_decimal(c.log_spectral),
                    "envelope": format_decimal(c.envelope),
                    "transient": format_decimal(c.transient),
                    "band_energy": format_decimal(c.band_energy),
                }),
            );
        }
        if let Some(best) = f.best {
            frame.insert("best_loss".into(), json!(format_decimal(best)));
        }
        let parameters: Vec<Value> = f
            .parameters
            .iter()
            .map(|(path, value)| json!({"path": path, "normalized_value": format_decimal(*value)}))
            .collect();
        frame.insert("parameters".into(), Value::Array(parameters));
        Value::Object(frame)
    }
}
