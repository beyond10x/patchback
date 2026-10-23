//! Graph validation (DESIGN.md § 2 invariant 3, § 5.1, § 5.4, § 12.1, § 12.3): a loaded
//! [`Patch`] becomes a [`ValidatedPatch`], the only input [`compile`](super::compile::compile)
//! takes, or is refused with a [`ValidationError`].
//!
//! # Checks
//!
//! The checks of § 5.1, each over the patch in canonical order ([`Patch::canonical`]), in the
//! order `spec/domains/patch.yaml` declares their outcomes:
//!
//! 1. **The file's checks again**, because a [`Patch`] need not come from the parser: the format
//!    version, the sample rate, unique node identifiers and unique parameter names per node.
//! 2. **Identifiers**: the topology is known ([`known_topology`]), every node's kind is in its
//!    catalogue, and both ends of every edge are declared nodes.
//! 3. **Stated rates**: both ends of an edge state one rate. Nothing is converted.
//! 4. **Parameters**: each node carries exactly the parameters its kind declares
//!    ([`NodeKind::parameter_paths`]), each within its contract's physical range
//!    ([`ParameterContract`](crate::parameter::ParameterContract)): a missing, unknown or
//!    out-of-range parameter is refused. Nothing is clamped here; clamping is
//!    [`Topology::set`]'s, on normalized values.
//! 5. **Ports**: each edge's from end names an output port and its to end an input port its node
//!    kind declares (the § 5.1 table, [`NodeKind::output_ports`], [`NodeKind::input_ports`]);
//!    then each end states the rate its kind declares for that port.
//! 6. **Cycles**: any cycle is refused as a zero-delay cycle. The `Delay` exception of § 5.4 is
//!    story:delayed-feedback, not this check: no topology admits `Delay` yet.
//! 7. **Resource caps**: [`MAX_GRAPH_NODES`], [`MAX_GRAPH_EDGES`], [`MAX_VOICES`].
//! 8. **One edge per input port** (§ 5.1): a duplicated edge, or a second source into an input
//!    that already takes one, is refused. An output port feeds any number of edges.
//!
//! § 5.1 lists parameters before ports and rates. The stated-rate check comes first because the
//! domain states it as a predicate on the edge alone, and ESS answers every predicate outcome
//! before any outcome it leaves to the implementation (parameters, ports, cycles, caps, shared
//! inputs).
//!
//! # Topology
//!
//! A patch names its topology by its `TopologyName` (§ 5.1
//! `topology_id: String`, § 11): `subtractive-mono/1`. The Uuid that `spec/domains/patch.yaml`
//! gives `Patch.topology_id` is the model's identity of the topology carrying that name; a Uuid
//! in a patch is a name no topology carries, so it is [`ValidationError::UnknownTopology`].
//!
//! # Precedence
//!
//! A patch with several faults is refused with the one whose error `spec/domains/patch.yaml`
//! declares first ([`VALIDATE_PATCH_PRECEDENCE`]): `LoadPatch`'s errors (which the model checks
//! when the patch is loaded, and this crate's parser leaves the topology to the caller), then
//! `ValidatePatch`'s. Among faults of one error, the first in canonical order: the lowest node
//! identifier, the first edge in canonical order, a node's parameters by name.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;

use super::{
    EdgeDecl, FORMAT_VERSION, MAX_EDGES, MAX_NODES, NodeKind, Patch, PortRate,
    SUPPORTED_SAMPLE_RATE_HZ,
};
use crate::parameter::{SUBTRACTIVE_MONO, Topology};

/// The most nodes a validated graph may have. Provisional: the file bound [`MAX_NODES`]
/// (DESIGN.md § 12.1, § 12.3 give no value; `spec/domains/patch.yaml` `resource-cap`).
pub const MAX_GRAPH_NODES: usize = MAX_NODES;

/// The most edges a validated graph may have. Provisional: the file bound [`MAX_EDGES`].
pub const MAX_GRAPH_EDGES: usize = MAX_EDGES;

/// The most voices a patch may have: one `NoteInput` node is one voice, and the MVP is "one
/// monophonic voice" (DESIGN.md § 5.2). Provisional.
pub const MAX_VOICES: usize = 1;

/// The errors a [`ValidationError`] names, in the order `spec/domains/patch.yaml` declares their
/// outcomes: `LoadPatch`'s first, then `ValidatePatch`'s. The precedence of refusals (module
/// documentation).
pub const VALIDATE_PATCH_PRECEDENCE: [&str; 14] = [
    "patchback.patch.UnknownFormatVersion",
    "patchback.patch.UnsupportedSampleRate",
    "patchback.patch.DuplicateNodeId",
    "patchback.patch.DuplicateKey",
    "patchback.patch.UnknownTopology",
    "patchback.patch.KindOutsideCatalogue",
    "patchback.patch.UnknownNode",
    "patchback.patch.IncompatiblePortRates",
    "patchback.patch.ParameterContractViolation",
    "patchback.patch.UnknownPort",
    "patchback.patch.PortRateMismatch",
    "patchback.patch.ZeroDelayCycle",
    "patchback.patch.ResourceCapExceeded",
    "patchback.patch.InputPortTaken",
];

/// A port a node kind declares (DESIGN.md § 5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Port {
    pub name: &'static str,
    pub rate: PortRate,
}

const fn port(name: &'static str, rate: PortRate) -> Port {
    Port { name, rate }
}

const PITCH: Port = port("pitch", PortRate::Control);
const GATE: Port = port("gate", PortRate::Event);
const AUDIO_IN: Port = port("in", PortRate::Audio);
const AUDIO_OUT: Port = port("out", PortRate::Audio);
const GAIN: Port = port("gain", PortRate::Control);
const ENV: Port = port("env", PortRate::Control);

/// Which end of an edge a port is on: the from end names an output, the to end an input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortDirection {
    Input,
    Output,
}

impl PortDirection {
    fn name(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }
}

impl NodeKind {
    /// The input ports of the kind, in the § 5.1 table's order. A kind outside the MVP seven
    /// declares none until its family is added (§ 5.4; `spec/domains/patch.yaml` `EdgeDecl`).
    pub fn input_ports(self) -> &'static [Port] {
        match self {
            Self::SawOscillator => &[PITCH],
            Self::Drive | Self::LowpassFilter | Self::Output => &[AUDIO_IN],
            Self::AdsrEnvelope => &[GATE],
            Self::Vca => &[AUDIO_IN, GAIN],
            _ => &[],
        }
    }

    /// The output ports of the kind, in the § 5.1 table's order.
    pub fn output_ports(self) -> &'static [Port] {
        match self {
            Self::NoteInput => &[PITCH, GATE],
            Self::SawOscillator | Self::Drive | Self::LowpassFilter | Self::Vca => &[AUDIO_OUT],
            Self::AdsrEnvelope => &[ENV],
            _ => &[],
        }
    }

    /// The ports of the kind on one side.
    pub fn ports(self, direction: PortDirection) -> &'static [Port] {
        match direction {
            PortDirection::Input => self.input_ports(),
            PortDirection::Output => self.output_ports(),
        }
    }

    /// The parameters a node of the kind carries: the paths of the § 5.2 contracts its module
    /// owns, in the § 5.2 table's order. A node's parameter name is its contract's path (§ 17,
    /// OQ-2, open).
    pub fn parameter_paths(self) -> &'static [&'static str] {
        match self {
            Self::Drive => &["drive"],
            Self::LowpassFilter => &["filter_cutoff", "resonance_q"],
            Self::AdsrEnvelope => &["attack", "decay", "sustain", "release"],
            Self::Output => &["output_gain"],
            _ => &[],
        }
    }
}

/// The topology a patch's `topology_id` names by its name, if this crate knows it. One is known:
/// [`SUBTRACTIVE_MONO`], the fixed voice of § 5.2, which admits no § 5.4 family.
pub fn known_topology(topology_id: &str) -> Option<Topology> {
    (topology_id == SUBTRACTIVE_MONO).then(Topology::subtractive_mono)
}

/// Whether a known topology's catalogue holds `kind`. No known topology admits a § 5.4 family.
pub(super) fn admits(_topology: &Topology, kind: NodeKind) -> bool {
    NodeKind::MVP.contains(&kind)
}

/// A resource a cap bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceCap {
    /// [`MAX_GRAPH_NODES`].
    Nodes,
    /// [`MAX_GRAPH_EDGES`].
    Edges,
    /// [`MAX_VOICES`], counted as `NoteInput` nodes.
    Voices,
}

impl ResourceCap {
    pub const ALL: [Self; 3] = [Self::Nodes, Self::Edges, Self::Voices];

    /// The cap's value.
    pub fn limit(self) -> usize {
        match self {
            Self::Nodes => MAX_GRAPH_NODES,
            Self::Edges => MAX_GRAPH_EDGES,
            Self::Voices => MAX_VOICES,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Nodes => "nodes",
            Self::Edges => "edges",
            Self::Voices => "voices (NoteInput nodes)",
        }
    }
}

/// Why a patch is refused by validation.
#[derive(Clone, Debug, PartialEq)]
pub enum ValidationError {
    /// The patch's format version is not [`FORMAT_VERSION`].
    UnknownFormatVersion { format_version: u32 },
    /// The sample rate is not [`SUPPORTED_SAMPLE_RATE_HZ`].
    UnsupportedSampleRate { sample_rate_hz: u32 },
    /// Two nodes share one identifier.
    DuplicateNodeId { node: u32 },
    /// A node sets one parameter twice.
    DuplicateParameter { node: u32, name: String },
    /// The topology identifier names no known topology ([`known_topology`]).
    UnknownTopology { topology_id: String },
    /// A node's kind is outside the topology's catalogue (§ 5.4).
    KindOutsideCatalogue { node: u32, kind: NodeKind },
    /// An edge end names a node the patch does not declare.
    UnknownNode { edge: EdgeDecl, node: u32 },
    /// A node carries a parameter its kind does not declare.
    UnknownParameter {
        node: u32,
        kind: NodeKind,
        name: String,
    },
    /// A node lacks a parameter its kind declares.
    MissingParameter {
        node: u32,
        kind: NodeKind,
        name: &'static str,
    },
    /// A parameter value lies outside its contract's physical range (or is not finite).
    ParameterOutsideContract {
        node: u32,
        name: String,
        value: f64,
        low: f64,
        high: f64,
    },
    /// An edge end names a port its node kind does not declare on that side.
    UnknownPort {
        edge: EdgeDecl,
        node: u32,
        kind: NodeKind,
        direction: PortDirection,
        port: String,
    },
    /// An edge end states a rate other than the one its node kind declares for the port.
    PortRateMismatch {
        edge: EdgeDecl,
        node: u32,
        port: String,
        stated: PortRate,
        declared: PortRate,
    },
    /// An edge joins ports of two rates.
    IncompatiblePortRates { edge: EdgeDecl },
    /// A cycle: its nodes in edge direction, from the lowest identifier on it.
    ZeroDelayCycle { nodes: Vec<u32> },
    /// The graph exceeds a resource cap.
    ResourceCapExceeded {
        cap: ResourceCap,
        limit: usize,
        found: usize,
    },
    /// An edge feeds an input port an earlier edge (canonical order) already feeds: a duplicated
    /// edge, or a second source into one input.
    InputPortTaken {
        edge: EdgeDecl,
        earlier: Box<EdgeDecl>,
    },
}

impl ValidationError {
    /// The error of the ESS domain `patchback.patch` this refusal is an instance of.
    pub fn spec_error(&self) -> &'static str {
        match self {
            Self::UnknownFormatVersion { .. } => "patchback.patch.UnknownFormatVersion",
            Self::UnsupportedSampleRate { .. } => "patchback.patch.UnsupportedSampleRate",
            Self::DuplicateNodeId { .. } => "patchback.patch.DuplicateNodeId",
            Self::DuplicateParameter { .. } => "patchback.patch.DuplicateKey",
            Self::UnknownTopology { .. } => "patchback.patch.UnknownTopology",
            Self::KindOutsideCatalogue { .. } => "patchback.patch.KindOutsideCatalogue",
            Self::UnknownNode { .. } => "patchback.patch.UnknownNode",
            Self::UnknownParameter { .. }
            | Self::MissingParameter { .. }
            | Self::ParameterOutsideContract { .. } => "patchback.patch.ParameterContractViolation",
            Self::UnknownPort { .. } => "patchback.patch.UnknownPort",
            Self::IncompatiblePortRates { .. } => "patchback.patch.IncompatiblePortRates",
            Self::PortRateMismatch { .. } => "patchback.patch.PortRateMismatch",
            Self::ZeroDelayCycle { .. } => "patchback.patch.ZeroDelayCycle",
            Self::ResourceCapExceeded { .. } => "patchback.patch.ResourceCapExceeded",
            Self::InputPortTaken { .. } => "patchback.patch.InputPortTaken",
        }
    }

    /// The rank of this refusal: the index of its [`spec_error`](Self::spec_error) in
    /// [`VALIDATE_PATCH_PRECEDENCE`]; lower is answered first.
    pub fn precedence(&self) -> usize {
        let error = self.spec_error();
        VALIDATE_PATCH_PRECEDENCE
            .iter()
            .position(|declared| *declared == error)
            .unwrap_or(VALIDATE_PATCH_PRECEDENCE.len())
    }
}

/// An edge as its `.patch` line reads, so a refusal names it as the file does.
struct EdgeText<'a>(&'a EdgeDecl);

impl fmt::Display for EdgeText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let edge = self.0;
        write!(
            f,
            "edge {} {} {} {} {} {}",
            edge.from_node,
            edge.from_port,
            edge.from_rate.name(),
            edge.to_node,
            edge.to_port,
            edge.to_rate.name()
        )
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFormatVersion { format_version } => write!(
                f,
                "format version {format_version} is not read; this crate reads version {FORMAT_VERSION}"
            ),
            Self::UnsupportedSampleRate { sample_rate_hz } => write!(
                f,
                "sample rate {sample_rate_hz} Hz is not supported; only {SUPPORTED_SAMPLE_RATE_HZ} Hz is"
            ),
            Self::DuplicateNodeId { node } => write!(f, "node {node} is declared twice"),
            Self::DuplicateParameter { node, name } => {
                write!(f, "node {node} sets parameter {name} twice")
            }
            Self::UnknownTopology { topology_id } => write!(
                f,
                "topology {topology_id} is not known; the known topology is {SUBTRACTIVE_MONO}"
            ),
            Self::KindOutsideCatalogue { node, kind } => write!(
                f,
                "node {node}: kind {} is outside the topology's catalogue",
                kind.name()
            ),
            Self::UnknownNode { edge, node } => {
                write!(f, "{}: node {node} is not declared", EdgeText(edge))
            }
            Self::UnknownParameter { node, kind, name } => {
                write!(f, "node {node}: {} has no parameter {name}", kind.name())
            }
            Self::MissingParameter { node, kind, name } => write!(
                f,
                "node {node}: {} needs parameter {name}, which is missing",
                kind.name()
            ),
            Self::ParameterOutsideContract {
                node,
                name,
                value,
                low,
                high,
            } => write!(
                f,
                "node {node}: parameter {name} is {value}, outside its contract's range [{low}, {high}]"
            ),
            Self::UnknownPort {
                edge,
                node,
                kind,
                direction,
                port,
            } => write!(
                f,
                "{}: node {node} ({}) declares no {} port {port}",
                EdgeText(edge),
                kind.name(),
                direction.name()
            ),
            Self::PortRateMismatch {
                edge,
                node,
                port,
                stated,
                declared,
            } => write!(
                f,
                "{}: node {node} port {port} is {}, not {}",
                EdgeText(edge),
                declared.name(),
                stated.name()
            ),
            Self::IncompatiblePortRates { edge } => write!(
                f,
                "{}: the {} rate cannot feed the {} rate; an edge joins ports of one rate",
                EdgeText(edge),
                edge.from_rate.name(),
                edge.to_rate.name()
            ),
            Self::ZeroDelayCycle { nodes } => {
                let path: Vec<String> = nodes
                    .iter()
                    .chain(nodes.first())
                    .map(u32::to_string)
                    .collect();
                write!(
                    f,
                    "zero-delay cycle through nodes {}; no Delay node is on it",
                    path.join(" -> ")
                )
            }
            Self::ResourceCapExceeded { cap, limit, found } => {
                write!(f, "{found} {} exceed the cap of {limit}", cap.name())
            }
            Self::InputPortTaken { edge, earlier } => write!(
                f,
                "{}: node {} input port {} already takes {}; an input port takes one edge",
                EdgeText(edge),
                edge.to_node,
                edge.to_port,
                EdgeText(earlier)
            ),
        }
    }
}

impl std::error::Error for ValidationError {}

/// A patch that passed every check of [`validate`], in canonical order, with its topology. Only
/// [`validate`] makes one.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedPatch {
    patch: Patch,
    topology: Topology,
}

impl ValidatedPatch {
    /// The patch, in canonical order ([`Patch::canonical`]).
    pub fn patch(&self) -> &Patch {
        &self.patch
    }

    pub fn topology(&self) -> &Topology {
        &self.topology
    }
}

/// Validates a loaded patch (module documentation).
pub fn validate(patch: &Patch) -> Result<ValidatedPatch, ValidationError> {
    let patch = patch.canonical();
    let topology = check_file(&patch)?;
    let kinds = check_identifiers(&patch, &topology)?;
    if let Some(edge) = patch.edges.iter().find(|e| e.from_rate != e.to_rate) {
        return Err(ValidationError::IncompatiblePortRates { edge: edge.clone() });
    }
    check_parameters(&patch, &topology)?;
    check_ports(&patch, &kinds)?;
    check_cycles(&patch)?;
    check_caps(&patch)?;
    check_inputs(&patch)?;
    Ok(ValidatedPatch { patch, topology })
}

/// The checks of the file (and the topology, which the parser leaves to the caller), in
/// `LoadPatch`'s declaration order.
fn check_file(patch: &Patch) -> Result<Topology, ValidationError> {
    if patch.format_version != FORMAT_VERSION {
        return Err(ValidationError::UnknownFormatVersion {
            format_version: patch.format_version,
        });
    }
    if patch.sample_rate_hz != SUPPORTED_SAMPLE_RATE_HZ {
        return Err(ValidationError::UnsupportedSampleRate {
            sample_rate_hz: patch.sample_rate_hz,
        });
    }
    // Canonical order: a duplicate identifier is adjacent to its first.
    if let Some(pair) = patch.nodes.windows(2).find(|pair| pair[0].id == pair[1].id) {
        return Err(ValidationError::DuplicateNodeId { node: pair[0].id });
    }
    for node in &patch.nodes {
        if let Some(pair) = node
            .parameters
            .windows(2)
            .find(|pair| pair[0].name == pair[1].name)
        {
            return Err(ValidationError::DuplicateParameter {
                node: node.id,
                name: pair[0].name.clone(),
            });
        }
    }
    known_topology(&patch.topology_id).ok_or_else(|| ValidationError::UnknownTopology {
        topology_id: patch.topology_id.clone(),
    })
}

/// The catalogue and the edge endpoints. Returns each node's kind by identifier.
fn check_identifiers(
    patch: &Patch,
    topology: &Topology,
) -> Result<BTreeMap<u32, NodeKind>, ValidationError> {
    if let Some(node) = patch.nodes.iter().find(|n| !admits(topology, n.kind)) {
        return Err(ValidationError::KindOutsideCatalogue {
            node: node.id,
            kind: node.kind,
        });
    }
    let kinds: BTreeMap<u32, NodeKind> = patch.nodes.iter().map(|n| (n.id, n.kind)).collect();
    for edge in &patch.edges {
        for end in [edge.from_node, edge.to_node] {
            if !kinds.contains_key(&end) {
                return Err(ValidationError::UnknownNode {
                    edge: edge.clone(),
                    node: end,
                });
            }
        }
    }
    Ok(kinds)
}

/// Each node carries exactly its kind's parameters, each within its contract's physical range.
fn check_parameters(patch: &Patch, topology: &Topology) -> Result<(), ValidationError> {
    for node in &patch.nodes {
        let declared = node.kind.parameter_paths();
        // Parameters are in name order (canonical), so the first unknown one is the first by name.
        if let Some(unknown) = node
            .parameters
            .iter()
            .find(|p| !declared.contains(&p.name.as_str()))
        {
            return Err(ValidationError::UnknownParameter {
                node: node.id,
                kind: node.kind,
                name: unknown.name.clone(),
            });
        }
        let carried: HashSet<&str> = node.parameters.iter().map(|p| p.name.as_str()).collect();
        let mut missing: Vec<&'static str> = declared
            .iter()
            .copied()
            .filter(|path| !carried.contains(path))
            .collect();
        missing.sort_unstable();
        if let Some(name) = missing.first() {
            return Err(ValidationError::MissingParameter {
                node: node.id,
                kind: node.kind,
                name,
            });
        }
        for parameter in &node.parameters {
            let Some(contract) = topology.contract(&parameter.name) else {
                unreachable!("every declared parameter path has a contract in the topology")
            };
            let low = contract.physical_at_zero.min(contract.physical_at_one);
            let high = contract.physical_at_zero.max(contract.physical_at_one);
            // NaN fails the range test as well.
            if !(low..=high).contains(&parameter.value) {
                return Err(ValidationError::ParameterOutsideContract {
                    node: node.id,
                    name: parameter.name.clone(),
                    value: parameter.value,
                    low,
                    high,
                });
            }
        }
    }
    Ok(())
}

/// Every edge end names a port its kind declares on that side; then each end states its rate.
/// The stated rates already agree (the check before parameters), so the declared ones do too.
fn check_ports(patch: &Patch, kinds: &BTreeMap<u32, NodeKind>) -> Result<(), ValidationError> {
    let declared = |edge: &EdgeDecl, direction: PortDirection| {
        let (node, name) = match direction {
            PortDirection::Output => (edge.from_node, edge.from_port.as_str()),
            PortDirection::Input => (edge.to_node, edge.to_port.as_str()),
        };
        let kind = kinds[&node];
        kind.ports(direction)
            .iter()
            .find(|port| port.name == name)
            .copied()
            .ok_or_else(|| ValidationError::UnknownPort {
                edge: edge.clone(),
                node,
                kind,
                direction,
                port: name.to_owned(),
            })
    };
    for edge in &patch.edges {
        declared(edge, PortDirection::Output)?;
        declared(edge, PortDirection::Input)?;
    }
    for edge in &patch.edges {
        let from = declared(edge, PortDirection::Output)?;
        let to = declared(edge, PortDirection::Input)?;
        for (node, port, stated) in [
            (edge.from_node, from, edge.from_rate),
            (edge.to_node, to, edge.to_rate),
        ] {
            if stated != port.rate {
                return Err(ValidationError::PortRateMismatch {
                    edge: edge.clone(),
                    node,
                    port: port.name.to_owned(),
                    stated,
                    declared: port.rate,
                });
            }
        }
    }
    Ok(())
}

/// Refuses any cycle. Kahn's algorithm removes every node no cycle reaches back to; from what
/// remains, a walk backwards along edges (the lowest identifier first) must repeat a node, and the
/// repeated stretch is a cycle.
fn check_cycles(patch: &Patch) -> Result<(), ValidationError> {
    let mut remaining: BTreeSet<u32> = patch.nodes.iter().map(|n| n.id).collect();
    let mut indegree: BTreeMap<u32, usize> = remaining.iter().map(|&id| (id, 0)).collect();
    let mut successors: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for edge in &patch.edges {
        *indegree.entry(edge.to_node).or_default() += 1;
        successors
            .entry(edge.from_node)
            .or_default()
            .push(edge.to_node);
    }
    let mut ready: Vec<u32> = indegree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(id, _)| *id)
        .collect();
    while let Some(id) = ready.pop() {
        remaining.remove(&id);
        for next in successors.get(&id).into_iter().flatten() {
            let degree = indegree.entry(*next).or_default();
            *degree -= 1;
            if *degree == 0 {
                ready.push(*next);
            }
        }
    }
    let Some(&start) = remaining.first() else {
        return Ok(());
    };
    // Every remaining node has a remaining predecessor.
    let predecessor = |id: u32| {
        patch
            .edges
            .iter()
            .filter(|e| e.to_node == id && remaining.contains(&e.from_node))
            .map(|e| e.from_node)
            .min()
    };
    let mut walk = vec![start];
    let mut at = start;
    loop {
        let Some(previous) = predecessor(at) else {
            unreachable!("a node Kahn's algorithm leaves has a predecessor it also leaves")
        };
        if let Some(seen) = walk.iter().position(|&id| id == previous) {
            let mut nodes: Vec<u32> = walk[seen..].to_vec();
            // The walk went against the edges; the cycle reads along them.
            nodes.reverse();
            let lowest = (0..nodes.len()).min_by_key(|&i| nodes[i]).unwrap_or(0);
            nodes.rotate_left(lowest);
            return Err(ValidationError::ZeroDelayCycle { nodes });
        }
        walk.push(previous);
        at = previous;
    }
}

fn check_caps(patch: &Patch) -> Result<(), ValidationError> {
    let voices = patch
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::NoteInput)
        .count();
    for (cap, found) in [
        (ResourceCap::Nodes, patch.nodes.len()),
        (ResourceCap::Edges, patch.edges.len()),
        (ResourceCap::Voices, voices),
    ] {
        if found > cap.limit() {
            return Err(ValidationError::ResourceCapExceeded {
                cap,
                limit: cap.limit(),
                found,
            });
        }
    }
    Ok(())
}

/// Each input port takes at most one edge; the refusal names the first edge, in canonical order,
/// into a port an earlier edge feeds.
fn check_inputs(patch: &Patch) -> Result<(), ValidationError> {
    let mut fed: BTreeMap<(u32, &str), &EdgeDecl> = BTreeMap::new();
    for edge in &patch.edges {
        if let Some(earlier) = fed.insert((edge.to_node, edge.to_port.as_str()), edge) {
            return Err(ValidationError::InputPortTaken {
                edge: edge.clone(),
                earlier: Box::new(earlier.clone()),
            });
        }
    }
    Ok(())
}
