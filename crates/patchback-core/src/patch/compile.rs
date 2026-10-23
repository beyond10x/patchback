//! Compilation (DESIGN.md § 5.1): scheduling and buffer assignment of a [`ValidatedPatch`] into
//! an [`ExecutionPlan`], the flat value a renderer walks without following graph references.
//!
//! - **Scheduling**: a topological order by Kahn's algorithm, the ready node with the lowest
//!   identifier first. The order depends on the graph alone, not on the order nodes or edges are
//!   listed in, so the same patch compiles to the same plan (§ 2 invariant 5).
//! - **Buffer assignment**: one buffer per output port of every node, numbered from 0 in plan
//!   order, each node's outputs in its kind's port order ([`NodeKind::output_ports`]). An input
//!   reads the buffer of the output its edge comes from. An unconnected output still has its
//!   buffer; an unconnected input reads none.
//!
//! There is no DSP here; rendering is a later story. A plan is immutable: a changed patch is
//! validated and compiled again.
//!
//! Only a validated patch compiles. [`compile`] takes a [`ValidatedPatch`], which only
//! [`validate`](super::validate::validate) makes, so compiling a loaded patch does not type-check:
//!
//! ```compile_fail
//! use patchback_core::patch::{compile, parse_patch};
//! let patch = parse_patch("patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n").unwrap();
//! let plan = compile(&patch);
//! ```
//!
//! and a [`ValidatedPatch`] cannot be built outside this crate:
//!
//! ```compile_fail
//! use patchback_core::patch::{ValidatedPatch, parse_patch};
//! use patchback_core::parameter::Topology;
//! let patch = parse_patch("patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n").unwrap();
//! let validated = ValidatedPatch { patch, topology: Topology::subtractive_mono() };
//! ```
//!
//! The one path to a plan goes through validation, which refuses a patch that is not the fixed
//! § 5.2 chain, such as one with no nodes:
//!
//! ```
//! use patchback_core::patch::{compile, parse_patch, validate};
//! let header = "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n";
//! assert!(validate(&parse_patch(header).unwrap()).is_err());
//! let voice = format!(
//!     "{header}node 1 NoteInput\nnode 2 SawOscillator\nnode 3 Drive\nparam 3 drive 6\n\
//!      node 4 LowpassFilter\nparam 4 filter_cutoff 1200\nparam 4 resonance_q 0.7\n\
//!      node 5 AdsrEnvelope\nparam 5 attack 0.01\nparam 5 decay 0.2\nparam 5 sustain 0.6\n\
//!      param 5 release 0.3\nnode 6 Vca\nnode 7 Output\nparam 7 output_gain -6\n\
//!      edge 1 pitch Control 2 pitch Control\nedge 1 gate Event 5 gate Event\n\
//!      edge 2 out Audio 3 in Audio\nedge 3 out Audio 4 in Audio\nedge 4 out Audio 6 in Audio\n\
//!      edge 5 env Control 6 gain Control\nedge 6 out Audio 7 in Audio\n"
//! );
//! let plan = compile(&validate(&parse_patch(&voice).unwrap()).unwrap());
//! assert_eq!(plan.order(), [1, 2, 3, 4, 5, 6, 7]);
//! ```

use std::collections::{BTreeMap, BTreeSet};

use super::validate::ValidatedPatch;
use super::{NodeDecl, NodeKind, ParameterValue, Patch, PortRate};

/// An input of a plan step: the port and the buffer it reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanInput {
    pub port: &'static str,
    pub rate: PortRate,
    /// The buffer of the output the edge comes from.
    pub buffer: usize,
    /// The node that writes [`Self::buffer`].
    pub from_node: u32,
}

/// An output of a plan step: the port and the buffer it writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanOutput {
    pub port: &'static str,
    pub rate: PortRate,
    pub buffer: usize,
}

/// One node of the plan, in execution order.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanStep {
    pub node: u32,
    pub kind: NodeKind,
    /// One per edge into the node, in the kind's input port order, then by buffer.
    pub inputs: Vec<PlanInput>,
    /// One per output port the kind declares, in its port order.
    pub outputs: Vec<PlanOutput>,
    /// The node's parameters in physical units, in the kind's order
    /// ([`NodeKind::parameter_paths`]).
    pub parameters: Vec<ParameterValue>,
}

/// A DSP execution plan: node order, buffers and parameter values (§ 5.1). Only [`compile`]
/// makes one.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionPlan {
    steps: Vec<PlanStep>,
    buffer_count: usize,
}

impl ExecutionPlan {
    /// The steps in execution order.
    pub fn steps(&self) -> &[PlanStep] {
        &self.steps
    }

    /// The number of buffers, one per output port; buffer indices are `0..buffer_count`.
    pub fn buffer_count(&self) -> usize {
        self.buffer_count
    }

    /// The node identifiers in execution order.
    pub fn order(&self) -> Vec<u32> {
        self.steps.iter().map(|step| step.node).collect()
    }
}

/// Schedules a validated patch and assigns its buffers (module documentation).
pub fn compile(patch: &ValidatedPatch) -> ExecutionPlan {
    let patch = patch.patch();
    let order = schedule(patch);
    let nodes: BTreeMap<u32, &NodeDecl> = patch.nodes.iter().map(|n| (n.id, n)).collect();

    // Buffers: one per output port, in plan order, each node's outputs in port order.
    let mut buffers: BTreeMap<(u32, &str), (usize, PortRate)> = BTreeMap::new();
    let mut steps: Vec<PlanStep> = Vec::with_capacity(order.len());
    for &id in &order {
        let node = nodes[&id];
        let outputs = node
            .kind
            .output_ports()
            .iter()
            .map(|port| {
                let buffer = buffers.len();
                buffers.insert((id, port.name), (buffer, port.rate));
                PlanOutput {
                    port: port.name,
                    rate: port.rate,
                    buffer,
                }
            })
            .collect();
        let parameters = node
            .kind
            .parameter_paths()
            .iter()
            .filter_map(|path| node.parameters.iter().find(|p| p.name == *path).cloned())
            .collect();
        steps.push(PlanStep {
            node: id,
            kind: node.kind,
            inputs: Vec::new(),
            outputs,
            parameters,
        });
    }
    let position: BTreeMap<u32, usize> = order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    for edge in &patch.edges {
        let step = &mut steps[position[&edge.to_node]];
        let Some(port) = step
            .kind
            .input_ports()
            .iter()
            .find(|port| port.name == edge.to_port)
        else {
            unreachable!("validation admits only declared input ports")
        };
        let Some(&(buffer, rate)) = buffers.get(&(edge.from_node, edge.from_port.as_str())) else {
            unreachable!("validation admits only declared output ports")
        };
        step.inputs.push(PlanInput {
            port: port.name,
            rate,
            buffer,
            from_node: edge.from_node,
        });
    }
    for step in &mut steps {
        let rank = |name: &str| {
            step.kind
                .input_ports()
                .iter()
                .position(|port| port.name == name)
        };
        let mut inputs = std::mem::take(&mut step.inputs);
        inputs.sort_by_key(|input| (rank(input.port), input.buffer));
        step.inputs = inputs;
    }
    ExecutionPlan {
        buffer_count: buffers.len(),
        steps,
    }
}

/// Kahn's algorithm, the ready node with the lowest identifier first. Validation refused every
/// cycle, so every node is scheduled.
fn schedule(patch: &Patch) -> Vec<u32> {
    let mut indegree: BTreeMap<u32, usize> = patch.nodes.iter().map(|n| (n.id, 0)).collect();
    let mut successors: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for edge in &patch.edges {
        *indegree.entry(edge.to_node).or_default() += 1;
        successors
            .entry(edge.from_node)
            .or_default()
            .push(edge.to_node);
    }
    let mut ready: BTreeSet<u32> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut order = Vec::with_capacity(indegree.len());
    while let Some(id) = ready.pop_first() {
        order.push(id);
        for next in successors.get(&id).into_iter().flatten() {
            let degree = indegree.entry(*next).or_default();
            *degree -= 1;
            if *degree == 0 {
                ready.insert(*next);
            }
        }
    }
    debug_assert_eq!(
        order.len(),
        patch.nodes.len(),
        "a validated patch is acyclic"
    );
    order
}
