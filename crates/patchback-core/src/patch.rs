//! The patch (DESIGN.md § 5.1) and its line-oriented `.patch` file (§ 11).
//!
//! A [`Patch`] is a format version, a sample rate, a topology reference, nodes and edges. The
//! names follow the ESS domain `patchback.patch` (`spec/domains/patch.yaml`): [`NodeDecl`],
//! [`EdgeDecl`], [`ParameterValue`], [`NodeKind`] and [`PortRate`]. Parameter values are in
//! physical units (§ 5.2).
//!
//! [`parse_patch`], [`read_patch`] and [`read_patch_file`] return a *loaded* patch: the file is
//! well formed, and nothing more is known. Whether an edge names declared nodes and ports, whether
//! the rates agree, whether the graph has cycles and whether a parameter fits its contract are the
//! graph checks of § 5.1, which a loaded patch has not passed (`patchback.patch.ValidatePatch`).
//! The file grammar, the bounds and the writer are in [`file`].
//!
//! [`validate`] runs those checks and returns a [`ValidatedPatch`], the only input [`compile`]
//! takes; [`compile`] schedules it into an [`ExecutionPlan`]. The checks, the port table and the
//! caps are in [`validate`](mod@validate), scheduling and buffers in [`compile`](mod@compile).
//! [`load_patch_file`] reads, parses and validates a file, answering with the refusal the
//! specification declares first ([`load`]).

pub mod compile;
pub mod file;
pub mod load;
pub mod validate;

pub use compile::{ExecutionPlan, PlanInput, PlanOutput, PlanStep, compile};
pub use file::{
    LOAD_PATCH_PRECEDENCE, MAX_EDGES, MAX_FILE_BYTES, MAX_LINE_BYTES, MAX_NODES, PatchFileError,
    PatchFileReason, ReadPatchError, format_decimal, parse_decimal, parse_patch, read_patch,
    read_patch_file, write_patch,
};
pub use load::{LoadPatchError, load_patch, load_patch_file};
pub use validate::{
    MAX_GRAPH_EDGES, MAX_GRAPH_NODES, MAX_VOICES, Port, PortDirection, ResourceCap,
    VALIDATE_PATCH_PRECEDENCE, ValidatedPatch, ValidationError, known_topology, validate,
};

/// The `.patch` format version this crate reads and writes.
pub const FORMAT_VERSION: u32 = 1;

/// The one sample rate a patch may declare (§ 5.3: 48 kHz, nothing converted).
pub const SUPPORTED_SAMPLE_RATE_HZ: u32 = 48_000;

/// The rate of a port (§ 5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PortRate {
    Audio,
    Control,
    Event,
}

impl PortRate {
    pub const ALL: [Self; 3] = [Self::Audio, Self::Control, Self::Event];

    /// The token of the rate in a `.patch` file, the variant name of the ESS domain.
    pub fn name(self) -> &'static str {
        match self {
            Self::Audio => "Audio",
            Self::Control => "Control",
            Self::Event => "Event",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|rate| rate.name() == name)
    }
}

/// The kind of a node: the seven kinds of the § 5.2 voice, then the § 5.4 families, as the ESS
/// domain declares them. Whether a topology admits a § 5.4 kind is a graph check, not the file's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeKind {
    NoteInput,
    SawOscillator,
    Drive,
    LowpassFilter,
    AdsrEnvelope,
    Vca,
    Output,
    SquareOscillator,
    SineOscillator,
    HighpassFilter,
    BandpassFilter,
    Lfo,
    Delay,
    Noise,
}

impl NodeKind {
    pub const ALL: [Self; 14] = [
        Self::NoteInput,
        Self::SawOscillator,
        Self::Drive,
        Self::LowpassFilter,
        Self::AdsrEnvelope,
        Self::Vca,
        Self::Output,
        Self::SquareOscillator,
        Self::SineOscillator,
        Self::HighpassFilter,
        Self::BandpassFilter,
        Self::Lfo,
        Self::Delay,
        Self::Noise,
    ];

    /// The kinds of the fixed § 5.2 voice.
    pub const MVP: [Self; 7] = [
        Self::NoteInput,
        Self::SawOscillator,
        Self::Drive,
        Self::LowpassFilter,
        Self::AdsrEnvelope,
        Self::Vca,
        Self::Output,
    ];

    /// The token of the kind in a `.patch` file, the variant name of the ESS domain.
    pub fn name(self) -> &'static str {
        match self {
            Self::NoteInput => "NoteInput",
            Self::SawOscillator => "SawOscillator",
            Self::Drive => "Drive",
            Self::LowpassFilter => "LowpassFilter",
            Self::AdsrEnvelope => "AdsrEnvelope",
            Self::Vca => "Vca",
            Self::Output => "Output",
            Self::SquareOscillator => "SquareOscillator",
            Self::SineOscillator => "SineOscillator",
            Self::HighpassFilter => "HighpassFilter",
            Self::BandpassFilter => "BandpassFilter",
            Self::Lfo => "Lfo",
            Self::Delay => "Delay",
            Self::Noise => "Noise",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }
}

/// One parameter of one node, in physical units (§ 5.2). The name is a bare snake_case
/// parameter name (§ 17, OQ-2, open).
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterValue {
    pub name: String,
    pub value: f64,
}

/// A node of a patch: a value nested in the patch, with an identifier unique inside it.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeDecl {
    pub id: u32,
    pub kind: NodeKind,
    pub parameters: Vec<ParameterValue>,
}

/// An edge of a patch: each end's node, port and the rate its node kind declares for that port
/// (§ 5.1), one flat record as one line of the file carries it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdgeDecl {
    pub from_node: u32,
    pub from_port: String,
    pub from_rate: PortRate,
    pub to_node: u32,
    pub to_port: String,
    pub to_rate: PortRate,
}

impl EdgeDecl {
    /// The order of edges in the canonical file.
    fn canonical_key(&self) -> (u32, &str, PortRate, u32, &str, PortRate) {
        (
            self.from_node,
            &self.from_port,
            self.from_rate,
            self.to_node,
            &self.to_port,
            self.to_rate,
        )
    }
}

/// A patch as the file carries it (§ 5.1). The topology is referenced by `topology_id`, its name
/// (`subtractive-mono/1`), which the file does not resolve: whether a topology of that name
/// exists is [`validate`]'s check ([`known_topology`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub format_version: u32,
    pub sample_rate_hz: u32,
    pub topology_id: String,
    pub nodes: Vec<NodeDecl>,
    pub edges: Vec<EdgeDecl>,
}

impl Patch {
    /// The patch in canonical order, the order [`write_patch`] emits: nodes by identifier, each
    /// node's parameters by name (byte order), edges by from node, from port, from rate, to node,
    /// to port, to rate. Values are untouched.
    pub fn canonical(&self) -> Patch {
        let mut patch = self.clone();
        patch.nodes.sort_by_key(|node| node.id);
        for node in &mut patch.nodes {
            node.parameters.sort_by(|a, b| a.name.cmp(&b.name));
        }
        patch
            .edges
            .sort_by(|a, b| a.canonical_key().cmp(&b.canonical_key()));
        patch
    }
}
