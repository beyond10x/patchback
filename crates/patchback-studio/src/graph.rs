//! The drawing of a patch (`patchback.studio.DrawGraph`, event `GraphDrawn`; DESIGN.md § 5.1,
//! § 5.4, § 16): [`drawing`] ranks the nodes and marks delayed feedback, [`check`] answers the
//! domain's refusals, and [`modules`] gives each drawn kind's ports and parameter contracts.
//!
//! # Ranks and feedback
//!
//! An edge is **delayed feedback** when it leaves a `Delay` node and closes a cycle: its to
//! node reaches the `Delay` again along the patch's edges. DESIGN.md § 5.4 admits a cycle only
//! through such a node. A node's **rank** is the longest path to it from a node with no inputs
//! once every delayed-feedback edge is removed; the studio lays nodes out left to right by rank.
//! An edge naming a node the patch does not declare is drawn as written, takes no part in
//! ranking, and is refused by [`check`]. A cycle without a `Delay` node leaves its nodes
//! unranked, at rank 0: such a patch is never drawn, because validation refuses it.
//!
//! # Refusals
//!
//! [`check`] answers in the order `spec/domains/studio.yaml` declares `DrawGraph`'s outcomes:
//! a node id drawn twice ([`StudioError::DuplicateGraphNode`]), an edge whose end is not a drawn
//! node ([`StudioError::GraphEndpointMissing`]), an edge marked as delayed feedback that does not
//! leave a `Delay` node ([`StudioError::FeedbackNotAtDelay`]). The fourth, a patch that has not
//! passed validation (`PatchNotDrawable`), is the exporter's ([`crate::export`]).

use std::collections::{BTreeMap, BTreeSet};

use patchback_core::parameter::{
    ClampPolicy, ParameterContract, ParameterMapping, ParameterUnit, Topology,
};
use patchback_core::patch::{NodeKind, Patch, PortRate};
use serde_json::{Value, json};

use crate::StudioError;
use crate::id;

/// One drawn node (`patchback.studio.GraphNode`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphNode {
    pub node_id: u32,
    pub kind: NodeKind,
    pub rank: u32,
}

/// One drawn edge (`patchback.studio.GraphEdge`): its rate is the one both ends state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphEdge {
    pub from_node: u32,
    pub from_port: String,
    pub to_node: u32,
    pub to_port: String,
    pub rate: PortRate,
    pub delayed_feedback: bool,
}

/// The `patchback.studio.GraphDrawn` event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphDrawn {
    pub graph_view_id: String,
    pub patch_id: String,
    pub topology_id: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// The identity of the topology named `name` (`patchback.patch.TopologyId`).
pub fn topology_id(name: &str) -> String {
    id::derive(id::TOPOLOGY, &[name.as_bytes()])
}

/// The identity of the patch whose `.patch` file is `file`.
pub fn patch_id(file: &[u8]) -> String {
    id::derive(id::PATCH, &[file])
}

/// The drawing of `patch`, read from the `.patch` bytes `file`, which name its identities
/// (module documentation). Nodes in identifier order, edges in the patch's canonical order.
pub fn drawing(patch: &Patch, file: &[u8]) -> GraphDrawn {
    let patch = patch.canonical();
    let kinds: BTreeMap<u32, NodeKind> = patch.nodes.iter().map(|n| (n.id, n.kind)).collect();
    let present = |e: &patchback_core::patch::EdgeDecl| {
        kinds.contains_key(&e.from_node) && kinds.contains_key(&e.to_node)
    };
    let mut successors: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for edge in patch.edges.iter().filter(|e| present(e)) {
        successors
            .entry(edge.from_node)
            .or_default()
            .push(edge.to_node);
    }
    let reaches = |from: u32, to: u32| -> bool {
        let mut seen = BTreeSet::new();
        let mut stack = vec![from];
        while let Some(at) = stack.pop() {
            if at == to {
                return true;
            }
            if seen.insert(at) {
                stack.extend(successors.get(&at).into_iter().flatten().copied());
            }
        }
        false
    };
    let edges: Vec<GraphEdge> = patch
        .edges
        .iter()
        .map(|edge| GraphEdge {
            from_node: edge.from_node,
            from_port: edge.from_port.clone(),
            to_node: edge.to_node,
            to_port: edge.to_port.clone(),
            rate: edge.from_rate,
            delayed_feedback: kinds.get(&edge.from_node) == Some(&NodeKind::Delay)
                && present(edge)
                && reaches(edge.to_node, edge.from_node),
        })
        .collect();

    // longest path over the forward edges, by Kahn's algorithm
    let forward: Vec<&GraphEdge> = edges
        .iter()
        .filter(|e| !e.delayed_feedback && kinds.contains_key(&e.from_node))
        .filter(|e| kinds.contains_key(&e.to_node))
        .collect();
    let mut indegree: BTreeMap<u32, usize> = kinds.keys().map(|id| (*id, 0)).collect();
    for edge in &forward {
        *indegree.entry(edge.to_node).or_default() += 1;
    }
    let mut rank: BTreeMap<u32, u32> = kinds.keys().map(|id| (*id, 0)).collect();
    let mut ready: Vec<u32> = indegree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(id, _)| *id)
        .collect();
    while let Some(at) = ready.pop() {
        let next = rank[&at] + 1;
        for edge in forward.iter().filter(|e| e.from_node == at) {
            let to = edge.to_node;
            if rank[&to] < next {
                rank.insert(to, next);
            }
            let left = indegree.get_mut(&to).expect("a declared node");
            *left -= 1;
            if *left == 0 {
                ready.push(to);
            }
        }
    }
    let nodes = patch
        .nodes
        .iter()
        .map(|node| GraphNode {
            node_id: node.id,
            kind: node.kind,
            rank: rank[&node.id],
        })
        .collect();
    GraphDrawn {
        graph_view_id: id::derive(id::GRAPH_VIEW, &[file]),
        patch_id: patch_id(file),
        topology_id: topology_id(&patch.topology_id),
        nodes,
        edges,
    }
}

/// `DrawGraph`'s refusals of a drawing, the first that applies (module documentation).
pub fn check(graph: &GraphDrawn) -> Result<(), StudioError> {
    let mut ids = BTreeSet::new();
    if !graph.nodes.iter().all(|n| ids.insert(n.node_id)) {
        return Err(StudioError::DuplicateGraphNode {
            graph_view_id: graph.graph_view_id.clone(),
        });
    }
    if let Some(edge) = graph
        .edges
        .iter()
        .find(|e| !ids.contains(&e.from_node) || !ids.contains(&e.to_node))
    {
        return Err(StudioError::GraphEndpointMissing {
            graph_view_id: graph.graph_view_id.clone(),
            from_node: edge.from_node,
            to_node: edge.to_node,
        });
    }
    let kind = |id: u32| graph.nodes.iter().find(|n| n.node_id == id).map(|n| n.kind);
    if let Some(edge) = graph
        .edges
        .iter()
        .find(|e| e.delayed_feedback && kind(e.from_node) != Some(NodeKind::Delay))
    {
        return Err(StudioError::FeedbackNotAtDelay {
            graph_view_id: graph.graph_view_id.clone(),
            from_node: edge.from_node,
        });
    }
    Ok(())
}

impl GraphDrawn {
    /// The event as the studio's generated `GraphDrawn` type reads it.
    pub fn to_json(&self) -> Value {
        json!({
            "graph_view_id": self.graph_view_id,
            "patch_id": self.patch_id,
            "topology_id": self.topology_id,
            "nodes": self.nodes.iter().map(|n| json!({
                "node_id": n.node_id,
                "kind": n.kind.name(),
                "rank": n.rank,
            })).collect::<Vec<_>>(),
            "edges": self.edges.iter().map(|e| json!({
                "from_node": e.from_node,
                "from_port": e.from_port,
                "to_node": e.to_node,
                "to_port": e.to_port,
                "rate": e.rate.name(),
                "delayed_feedback": e.delayed_feedback,
            })).collect::<Vec<_>>(),
        })
    }
}

/// What a port carries, for the graph view's tooltip. Presentation text from DESIGN.md § 5.1 and
/// § 5.2, not a contract: the port names and rates are `patchback_core`'s port table.
fn meaning(kind: NodeKind, port: &str) -> &'static str {
    match (kind, port) {
        (NodeKind::NoteInput, "pitch") => "the held note's pitch",
        (NodeKind::NoteInput, "gate") => "note-on and note-off",
        (NodeKind::SawOscillator, "pitch") => "oscillator frequency follows the note",
        (NodeKind::SawOscillator, "out") => "band-limited saw",
        (NodeKind::Drive, "in") => "oscillator signal",
        (NodeKind::Drive, "out") => "driven signal",
        (NodeKind::LowpassFilter, "in") => "driven signal",
        (NodeKind::LowpassFilter, "out") => "filtered signal",
        (NodeKind::AdsrEnvelope, "gate") => "note-on and note-off",
        (NodeKind::AdsrEnvelope, "env") => "envelope level",
        (NodeKind::Vca, "in") => "filtered signal",
        (NodeKind::Vca, "gain") => "envelope gain",
        (NodeKind::Vca, "out") => "shaped signal",
        (NodeKind::Output, "in") => "the voice's output",
        _ => "",
    }
}

fn family(kind: NodeKind) -> Option<&'static str> {
    match kind {
        NodeKind::SquareOscillator | NodeKind::SineOscillator => Some("SquareSine"),
        NodeKind::HighpassFilter | NodeKind::BandpassFilter => Some("HighpassBandpass"),
        NodeKind::Lfo => Some("Lfo"),
        NodeKind::Delay => Some("Delay"),
        NodeKind::Noise => Some("Noise"),
        _ => None,
    }
}

fn unit(unit: ParameterUnit) -> &'static str {
    match unit {
        ParameterUnit::Seconds => "Seconds",
        ParameterUnit::LinearAmplitude => "LinearAmplitude",
        ParameterUnit::Hertz => "Hertz",
        ParameterUnit::Dimensionless => "Dimensionless",
        ParameterUnit::Decibels => "Decibels",
    }
}

fn contract(contract: &ParameterContract) -> Value {
    json!({
        "path": contract.path,
        "index": contract.index,
        "unit": unit(contract.unit),
        "mapping": match contract.mapping {
            ParameterMapping::Exponential => "Exponential",
            ParameterMapping::Linear => "Linear",
        },
        "physical_at_zero": crate::decimal(contract.physical_at_zero),
        "physical_at_one": crate::decimal(contract.physical_at_one),
        "default_normalized": crate::decimal(contract.default_normalized),
        "clamp": match contract.clamp {
            ClampPolicy::ClampToUnitInterval => "ClampToUnitInterval",
        },
    })
}

/// One `patchback.studio.ModuleSpec` per kind `graph` draws, in `NodeKind::ALL` order: its ports
/// (inputs, then outputs, in the § 5.1 table's order) and the contracts of `topology` its kind
/// owns.
pub fn modules(graph: &GraphDrawn, topology: &Topology) -> Vec<Value> {
    let drawn: BTreeSet<NodeKind> = graph.nodes.iter().map(|n| n.kind).collect();
    NodeKind::ALL
        .into_iter()
        .filter(|kind| drawn.contains(kind))
        .map(|kind| {
            let ports: Vec<Value> = [
                ("Input", kind.input_ports()),
                ("Output", kind.output_ports()),
            ]
            .into_iter()
            .flat_map(|(direction, ports)| {
                ports.iter().map(move |port| {
                    json!({
                        "name": port.name,
                        "direction": direction,
                        "rate": port.rate.name(),
                        "meaning": meaning(kind, port.name),
                    })
                })
            })
            .collect();
            let parameters: Vec<Value> = kind
                .parameter_paths()
                .iter()
                .filter_map(|path| topology.contract(path))
                .map(contract)
                .collect();
            let mut module = json!({
                "kind": kind.name(),
                "ports": ports,
                "parameters": parameters,
            });
            if let Some(family) = family(kind) {
                module["family"] = json!(family);
            }
            module
        })
        .collect()
}
