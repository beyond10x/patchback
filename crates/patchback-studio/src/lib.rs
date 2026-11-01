#![forbid(unsafe_code)]
//! Patchback Studio's exporter (DESIGN.md § 16, ADR-011): turns a fit run directory into the one
//! JSON document the studio in `studio/` loads, shaped like the types `ess generate types` writes
//! from `spec/domains/studio.yaml`.
//!
//! This is tooling, as `patchback-docs` is: no application crate depends on it, and it adds no
//! crate to the lock (`crates/patchback-cli/tests/dependencies.rs`). It reads the run's files
//! with the parsers that wrote them and computes nothing the run did not record, except the
//! layout ranks and the identities ([`id`]).

use std::fmt;

pub mod export;
pub mod graph;
pub mod id;
pub mod replay;

/// A finite `f64` as an ESS `Decimal` (`^-?(0|[1-9][0-9]*)(\.[0-9]+)?$`): the shortest digits
/// that read back to the same `f64`, written without an exponent, as Rust's `Display` writes
/// them (`1e-3` is `0.001`). The `.patch` and `trace.events` files keep their own form
/// (`patchback_core::patch::format_decimal`); only the export uses this one.
pub fn decimal(value: f64) -> String {
    format!("{value}")
}

/// The errors of `spec/domains/studio.yaml` the exporter answers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StudioError {
    /// A node id is drawn twice.
    DuplicateGraphNode { graph_view_id: String },
    /// An edge names a node the drawing does not hold.
    GraphEndpointMissing {
        graph_view_id: String,
        from_node: u32,
        to_node: u32,
    },
    /// An edge marked as delayed feedback does not leave a `Delay` node.
    FeedbackNotAtDelay {
        graph_view_id: String,
        from_node: u32,
    },
    /// The patch has not passed validation (`patchback.patch.ValidatePatch`).
    PatchNotDrawable { patch_id: String },
    /// A trace names one parameter path twice.
    DuplicateParameterPath { trace_id: String },
    /// The trace is closed and takes no further frame.
    TraceClosed { trace_id: String },
    /// A frame's sequence is not above every sequence the trace already holds.
    FrameOutOfOrder { trace_id: String, sequence: u64 },
    /// A frame's retained best loss is above an earlier frame's.
    BestLossWorsened { trace_id: String, sequence: u64 },
    /// A frame's outcome disagrees with its losses.
    InconsistentFrame { trace_id: String, sequence: u64 },
    /// A trace's end disagrees with its stop reason.
    InconsistentEnd { trace_id: String },
    /// The trace is closed already.
    TraceStateConflict { trace_id: String },
}

impl StudioError {
    /// The error's qualified name in the specification.
    pub fn name(&self) -> &'static str {
        match self {
            Self::DuplicateGraphNode { .. } => "patchback.studio.DuplicateGraphNode",
            Self::GraphEndpointMissing { .. } => "patchback.studio.GraphEndpointMissing",
            Self::FeedbackNotAtDelay { .. } => "patchback.studio.FeedbackNotAtDelay",
            Self::PatchNotDrawable { .. } => "patchback.studio.PatchNotDrawable",
            Self::DuplicateParameterPath { .. } => "patchback.studio.DuplicateParameterPath",
            Self::TraceClosed { .. } => "patchback.studio.TraceClosed",
            Self::FrameOutOfOrder { .. } => "patchback.studio.FrameOutOfOrder",
            Self::BestLossWorsened { .. } => "patchback.studio.BestLossWorsened",
            Self::InconsistentFrame { .. } => "patchback.studio.InconsistentFrame",
            Self::InconsistentEnd { .. } => "patchback.studio.InconsistentEnd",
            Self::TraceStateConflict { .. } => "patchback.studio.TraceStateConflict",
        }
    }
}

impl fmt::Display for StudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateGraphNode { graph_view_id } => {
                write!(f, "graph view {graph_view_id} draws a node id twice")
            }
            Self::GraphEndpointMissing {
                graph_view_id,
                from_node,
                to_node,
            } => write!(
                f,
                "graph view {graph_view_id} has an edge from node {from_node} to node \
                 {to_node}, and one of them is not drawn"
            ),
            Self::FeedbackNotAtDelay {
                graph_view_id,
                from_node,
            } => write!(
                f,
                "graph view {graph_view_id} marks an edge from node {from_node} as delayed \
                 feedback, and node {from_node} is not a Delay"
            ),
            Self::PatchNotDrawable { patch_id } if patch_id.is_empty() => {
                write!(f, "the patch has not passed validation")
            }
            Self::PatchNotDrawable { patch_id } => {
                write!(f, "patch {patch_id} has not passed validation")
            }
            Self::DuplicateParameterPath { trace_id } => {
                write!(f, "trace {trace_id} names a parameter path twice")
            }
            Self::TraceClosed { trace_id } => {
                write!(f, "trace {trace_id} is closed and takes no further frame")
            }
            Self::FrameOutOfOrder { trace_id, sequence } => write!(
                f,
                "trace {trace_id}: the frame of sequence {sequence} is not above every sequence \
                 the trace holds"
            ),
            Self::BestLossWorsened { trace_id, sequence } => write!(
                f,
                "trace {trace_id}: the best loss of the frame of sequence {sequence} is above an \
                 earlier frame's; it never rises"
            ),
            Self::InconsistentFrame { trace_id, sequence } => write!(
                f,
                "trace {trace_id}: the outcome of the frame of sequence {sequence} disagrees \
                 with its losses"
            ),
            Self::InconsistentEnd { trace_id } => write!(
                f,
                "trace {trace_id}: a completed run names a stop reason and a failed run none"
            ),
            Self::TraceStateConflict { trace_id } => {
                write!(f, "trace {trace_id} is closed already")
            }
        }
    }
}

impl std::error::Error for StudioError {}
