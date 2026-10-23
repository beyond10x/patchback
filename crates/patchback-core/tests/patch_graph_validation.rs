//! The obligations of story:patch-graph-validation (DESIGN.md § 2 invariants 3 and 5, § 5.1,
//! § 5.4, § 12.1, § 12.3). The ESS scenarios `spec/scenarios/patch-graph-validation-*.yaml` name
//! the command outcomes; these tests hold the checks, the precedence, the caps, the cycle search
//! and the deterministic schedule they cannot express.

use std::collections::BTreeSet;

use patchback_core::parameter::{SUBTRACTIVE_MONO, Topology};
use patchback_core::patch::validate::{FIXED_CHAIN_EDGES, FIXED_CHAIN_KINDS};
use patchback_core::patch::{
    EdgeDecl, ExecutionPlan, FORMAT_VERSION, LoadPatchError, MAX_GRAPH_EDGES, MAX_GRAPH_NODES,
    MAX_VOICES, NodeDecl, NodeKind, ParameterValue, Patch, PlanInput, PlanOutput, PortDirection,
    PortRate, ResourceCap, SUPPORTED_SAMPLE_RATE_HZ, VALIDATE_PATCH_PRECEDENCE, ValidatedPatch,
    ValidationError, compile, known_topology, load_patch, parse_patch, validate, write_patch,
};

// ---------------------------------------------------------------------------------------------
// Fixtures

fn value(name: &str, value: f64) -> ParameterValue {
    ParameterValue {
        name: name.to_owned(),
        value,
    }
}

fn node(id: u32, kind: NodeKind, parameters: Vec<ParameterValue>) -> NodeDecl {
    NodeDecl {
        id,
        kind,
        parameters,
    }
}

fn edge(from: (u32, &str, PortRate), to: (u32, &str, PortRate)) -> EdgeDecl {
    EdgeDecl {
        from_node: from.0,
        from_port: from.1.to_owned(),
        from_rate: from.2,
        to_node: to.0,
        to_port: to.1.to_owned(),
        to_rate: to.2,
    }
}

fn patch(nodes: Vec<NodeDecl>, edges: Vec<EdgeDecl>) -> Patch {
    Patch {
        format_version: FORMAT_VERSION,
        sample_rate_hz: SUPPORTED_SAMPLE_RATE_HZ,
        topology_id: SUBTRACTIVE_MONO.to_owned(),
        nodes,
        edges,
    }
}

/// The parameters of a node of `kind` at their contract defaults.
fn defaults(kind: NodeKind) -> Vec<ParameterValue> {
    let topology = Topology::subtractive_mono();
    kind.parameter_paths()
        .iter()
        .map(|path| {
            value(
                path,
                topology
                    .contract(path)
                    .expect("a contract")
                    .default_physical(),
            )
        })
        .collect()
}

fn with_defaults(id: u32, kind: NodeKind) -> NodeDecl {
    node(id, kind, defaults(kind))
}

/// The § 5.2 voice wired by the § 5.1 port table, every parameter at its contract default.
fn default_voice() -> Patch {
    use NodeKind::*;
    use PortRate::*;
    patch(
        vec![
            with_defaults(1, NoteInput),
            with_defaults(2, SawOscillator),
            with_defaults(3, Drive),
            with_defaults(4, LowpassFilter),
            with_defaults(5, AdsrEnvelope),
            with_defaults(6, Vca),
            with_defaults(7, Output),
        ],
        vec![
            edge((1, "pitch", Control), (2, "pitch", Control)),
            edge((1, "gate", Event), (5, "gate", Event)),
            edge((2, "out", Audio), (3, "in", Audio)),
            edge((3, "out", Audio), (4, "in", Audio)),
            edge((4, "out", Audio), (6, "in", Audio)),
            edge((5, "env", Control), (6, "gain", Control)),
            edge((6, "out", Audio), (7, "in", Audio)),
        ],
    )
}

/// The voice of `spec/scenarios/patch-graph-validation-accepted.yaml`, with its values.
fn scenario_voice() -> Patch {
    use NodeKind::*;
    let mut voice = default_voice();
    voice.nodes = vec![
        node(1, NoteInput, vec![]),
        node(2, SawOscillator, vec![]),
        node(3, Drive, vec![value("drive", 6.0)]),
        node(
            4,
            LowpassFilter,
            vec![value("filter_cutoff", 1200.0), value("resonance_q", 0.7)],
        ),
        node(
            5,
            AdsrEnvelope,
            vec![
                value("attack", 0.01),
                value("decay", 0.2),
                value("sustain", 0.6),
                value("release", 0.3),
            ],
        ),
        node(6, Vca, vec![]),
        node(7, Output, vec![value("output_gain", -6.0)]),
    ];
    voice
}

fn valid(patch: &Patch) -> ValidatedPatch {
    validate(patch).unwrap_or_else(|e| panic!("expected a valid patch, refused: {e}"))
}

fn refused(patch: &Patch) -> ValidationError {
    match validate(patch) {
        Ok(validated) => panic!("expected a refusal, validated {validated:?}"),
        Err(error) => error,
    }
}

fn output(port: &'static str, rate: PortRate, buffer: usize) -> PlanOutput {
    PlanOutput { port, rate, buffer }
}

fn input(port: &'static str, rate: PortRate, buffer: usize, from_node: u32) -> PlanInput {
    PlanInput {
        port,
        rate,
        buffer,
        from_node,
    }
}

// ---------------------------------------------------------------------------------------------
// Accepted (spec/scenarios/patch-graph-validation-accepted.yaml)

#[test]
fn accepted_the_default_voice_validates_and_compiles() {
    use PortRate::*;
    let validated = valid(&default_voice());
    assert_eq!(validated.topology(), &Topology::subtractive_mono());
    let plan = compile(&validated);
    assert_eq!(plan.order(), [1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(plan.buffer_count(), 7);
    let steps = plan.steps();
    assert_eq!(steps[0].kind, NodeKind::NoteInput);
    assert!(steps[0].inputs.is_empty());
    assert_eq!(
        steps[0].outputs,
        [output("pitch", Control, 0), output("gate", Event, 1)]
    );
    assert_eq!(steps[1].inputs, [input("pitch", Control, 0, 1)]);
    assert_eq!(steps[1].outputs, [output("out", Audio, 2)]);
    assert_eq!(steps[2].inputs, [input("in", Audio, 2, 2)]);
    assert_eq!(steps[2].outputs, [output("out", Audio, 3)]);
    assert_eq!(steps[3].inputs, [input("in", Audio, 3, 3)]);
    assert_eq!(steps[3].outputs, [output("out", Audio, 4)]);
    assert_eq!(steps[4].inputs, [input("gate", Event, 1, 1)]);
    assert_eq!(steps[4].outputs, [output("env", Control, 5)]);
    assert_eq!(
        steps[5].inputs,
        [input("in", Audio, 4, 4), input("gain", Control, 5, 5)]
    );
    assert_eq!(steps[5].outputs, [output("out", Audio, 6)]);
    assert_eq!(steps[6].inputs, [input("in", Audio, 6, 6)]);
    assert!(steps[6].outputs.is_empty());
    for step in steps {
        assert_eq!(step.parameters, defaults(step.kind), "node {}", step.node);
    }
}

#[test]
fn accepted_the_scenario_voice_validates_and_compiles_with_its_values() {
    let plan = compile(&valid(&scenario_voice()));
    assert_eq!(plan.order(), [1, 2, 3, 4, 5, 6, 7]);
    let adsr = &plan.steps()[4];
    assert_eq!(
        adsr.parameters,
        [
            value("attack", 0.01),
            value("decay", 0.2),
            value("sustain", 0.6),
            value("release", 0.3)
        ]
    );
}

#[test]
fn accepted_the_default_voice_loads_from_its_file_validates_and_compiles() {
    let text = write_patch(&default_voice()).expect("writable");
    let loaded = parse_patch(&text).expect("loads");
    assert_eq!(compile(&valid(&loaded)), compile(&valid(&default_voice())));
}

#[test]
fn a_validated_patch_is_the_canonical_patch() {
    let mut listed = default_voice();
    listed.nodes.reverse();
    listed.edges.reverse();
    assert_eq!(valid(&listed).patch(), &default_voice().canonical());
}

#[test]
fn every_parameter_at_either_end_of_its_contract_is_accepted() {
    let topology = Topology::subtractive_mono();
    for low_end in [true, false] {
        let end = |path: &str| {
            let contract = topology.contract(path).expect("a contract");
            if low_end {
                contract.physical_at_zero
            } else {
                contract.physical_at_one
            }
        };
        let mut voice = default_voice();
        for node in &mut voice.nodes {
            for parameter in &mut node.parameters {
                parameter.value = end(&parameter.name);
            }
        }
        let validated = valid(&voice);
        for node in &validated.patch().nodes {
            for parameter in &node.parameters {
                assert_eq!(parameter.value, end(&parameter.name), "{}", parameter.name);
            }
        }
    }
}

#[test]
fn the_known_topology_is_the_subtractive_voice() {
    assert_eq!(
        known_topology(SUBTRACTIVE_MONO),
        Some(Topology::subtractive_mono())
    );
    assert_eq!(known_topology("subtractive-mono/2"), None);
}

// ---------------------------------------------------------------------------------------------
// Determinism (numeric obligation 1: the same patch compiles to the same plan, § 2 invariant 5)

#[test]
fn the_same_patch_compiles_to_an_equal_plan_twice() {
    let voice = default_voice();
    assert_eq!(compile(&valid(&voice)), compile(&valid(&voice)));
}

#[test]
fn a_patch_listed_in_another_order_compiles_to_the_same_plan() {
    let reference = compile(&valid(&default_voice()));
    let mut reversed = default_voice();
    reversed.nodes.reverse();
    reversed.edges.reverse();
    for node in &mut reversed.nodes {
        node.parameters.reverse();
    }
    assert_eq!(compile(&valid(&reversed)), reference);
    let mut rotated = default_voice();
    rotated.nodes.rotate_left(3);
    rotated.edges.rotate_left(4);
    assert_eq!(compile(&valid(&rotated)), reference);
}

#[test]
fn ties_are_broken_by_the_lowest_node_id() {
    use NodeKind::*;
    use PortRate::*;
    // The envelope is node 2 and the oscillator node 5: both are ready after the note input.
    let voice = patch(
        vec![
            with_defaults(1, NoteInput),
            with_defaults(5, SawOscillator),
            with_defaults(3, Drive),
            with_defaults(4, LowpassFilter),
            with_defaults(2, AdsrEnvelope),
            with_defaults(6, Vca),
            with_defaults(7, Output),
        ],
        vec![
            edge((1, "pitch", Control), (5, "pitch", Control)),
            edge((1, "gate", Event), (2, "gate", Event)),
            edge((5, "out", Audio), (3, "in", Audio)),
            edge((3, "out", Audio), (4, "in", Audio)),
            edge((4, "out", Audio), (6, "in", Audio)),
            edge((2, "env", Control), (6, "gain", Control)),
            edge((6, "out", Audio), (7, "in", Audio)),
        ],
    );
    let plan = compile(&valid(&voice));
    assert_eq!(plan.order(), [1, 2, 5, 3, 4, 6, 7]);
    let buffers: Vec<Vec<usize>> = plan
        .steps()
        .iter()
        .map(|s| s.outputs.iter().map(|o| o.buffer).collect())
        .collect();
    assert_eq!(
        buffers,
        [
            vec![0, 1],
            vec![2],
            vec![3],
            vec![4],
            vec![5],
            vec![6],
            vec![]
        ]
    );
}

/// A patch with unconnected ports is not the fixed chain (story:patch-review-fixes), so no plan
/// has one: in the chain's plan every input reads one buffer and every buffer is read.
#[test]
fn unconnected_ports_are_refused_and_the_plan_of_the_chain_connects_every_port() {
    use NodeKind::*;
    let unconnected = patch(
        vec![with_defaults(9, Vca), with_defaults(4, NoteInput)],
        vec![],
    );
    assert!(matches!(
        refused(&unconnected),
        ValidationError::FixedChainMismatch { .. }
    ));
    let plan = compile(&valid(&default_voice()));
    let inputs: usize = plan
        .steps()
        .iter()
        .map(|s| {
            assert_eq!(
                s.inputs.len(),
                s.kind.input_ports().len(),
                "node {}",
                s.node
            );
            s.inputs.len()
        })
        .sum();
    let read: BTreeSet<usize> = plan
        .steps()
        .iter()
        .flat_map(|s| s.inputs.iter().map(|i| i.buffer))
        .collect();
    assert_eq!(inputs, FIXED_CHAIN_EDGES.len());
    assert_eq!(read, (0..plan.buffer_count()).collect());
}

// ---------------------------------------------------------------------------------------------
// Identifiers (spec/scenarios/patch-graph-validation-identifiers.yaml)

#[test]
fn refuses_an_unknown_format_version() {
    let mut voice = default_voice();
    voice.format_version = 2;
    assert_eq!(
        refused(&voice),
        ValidationError::UnknownFormatVersion { format_version: 2 }
    );
}

#[test]
fn refuses_an_unsupported_sample_rate() {
    let mut voice = default_voice();
    voice.sample_rate_hz = 44_100;
    assert_eq!(
        refused(&voice),
        ValidationError::UnsupportedSampleRate {
            sample_rate_hz: 44_100
        }
    );
}

#[test]
fn identifiers_refuses_a_duplicate_node_id() {
    let mut voice = default_voice();
    voice.nodes.push(with_defaults(6, NodeKind::Vca));
    assert_eq!(
        refused(&voice),
        ValidationError::DuplicateNodeId { node: 6 }
    );
}

#[test]
fn refuses_a_parameter_set_twice() {
    let mut voice = default_voice();
    voice.nodes[2].parameters.push(value("drive", 1.0));
    assert_eq!(
        refused(&voice),
        ValidationError::DuplicateParameter {
            node: 3,
            name: "drive".to_owned()
        }
    );
}

#[test]
fn identifiers_refuses_an_unknown_topology() {
    // A file names its topology by name; a name no topology carries, a later version of the known
    // one, and a Uuid (the model's identity, never a name) are all unknown.
    for unknown in [
        "fm-poly/1",
        "subtractive-mono/2",
        "7f0c0000-0000-4000-8000-000000000099",
    ] {
        let mut voice = default_voice();
        voice.topology_id = unknown.to_owned();
        assert_eq!(
            refused(&voice),
            ValidationError::UnknownTopology {
                topology_id: unknown.to_owned()
            }
        );
    }
}

#[test]
fn identifiers_refuses_a_kind_outside_the_catalogue() {
    for kind in NodeKind::ALL {
        let mut voice = default_voice();
        voice.nodes.push(node(20, kind, vec![]));
        if NodeKind::MVP.contains(&kind) {
            continue;
        }
        assert_eq!(
            refused(&voice),
            ValidationError::KindOutsideCatalogue { node: 20, kind },
            "{kind:?}"
        );
    }
}

#[test]
fn identifiers_refuses_an_edge_to_an_undeclared_node() {
    use NodeKind::*;
    use PortRate::*;
    let to = edge((2, "out", Audio), (9, "in", Audio));
    let scenario = patch(
        vec![with_defaults(2, SawOscillator), with_defaults(7, Output)],
        vec![to.clone()],
    );
    assert_eq!(
        refused(&scenario),
        ValidationError::UnknownNode { edge: to, node: 9 }
    );
    let from = edge((8, "out", Audio), (7, "in", Audio));
    let scenario = patch(
        vec![with_defaults(2, SawOscillator), with_defaults(7, Output)],
        vec![from.clone()],
    );
    assert_eq!(
        refused(&scenario),
        ValidationError::UnknownNode {
            edge: from,
            node: 8
        }
    );
}

// ---------------------------------------------------------------------------------------------
// Parameters

#[test]
fn refuses_a_parameter_its_kind_does_not_declare() {
    let mut voice = default_voice();
    voice.nodes[3].parameters.push(value("cutoff", 1200.0));
    assert_eq!(
        refused(&voice),
        ValidationError::UnknownParameter {
            node: 4,
            kind: NodeKind::LowpassFilter,
            name: "cutoff".to_owned()
        }
    );
    let mut voice = default_voice();
    voice.nodes[1].parameters.push(value("drive", 1.0));
    assert_eq!(
        refused(&voice),
        ValidationError::UnknownParameter {
            node: 2,
            kind: NodeKind::SawOscillator,
            name: "drive".to_owned()
        }
    );
}

#[test]
fn refuses_a_missing_parameter() {
    let mut voice = default_voice();
    voice.nodes[4].parameters.retain(|p| p.name != "sustain");
    assert_eq!(
        refused(&voice),
        ValidationError::MissingParameter {
            node: 5,
            kind: NodeKind::AdsrEnvelope,
            name: "sustain"
        }
    );
}

#[test]
fn refuses_a_parameter_outside_its_contract() {
    let topology = Topology::subtractive_mono();
    for contract in topology.contracts() {
        let (low, high) = (contract.physical_at_zero, contract.physical_at_one);
        for outside in [next_down(low), next_up(high), f64::NAN, f64::INFINITY] {
            let mut voice = default_voice();
            let (id, parameter) = voice
                .nodes
                .iter_mut()
                .find_map(|n| {
                    let id = n.id;
                    n.parameters
                        .iter_mut()
                        .find(|p| p.name == contract.path)
                        .map(|p| (id, p))
                })
                .expect("every contract is on a node");
            parameter.value = outside;
            match refused(&voice) {
                ValidationError::ParameterOutsideContract {
                    node,
                    name,
                    value,
                    low: l,
                    high: h,
                } => {
                    assert_eq!(
                        (node, name.as_str(), l, h),
                        (id, contract.path.as_str(), low, high)
                    );
                    assert_eq!(value.to_bits(), outside.to_bits());
                }
                other => panic!("{}: {other:?}", contract.path),
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Ports and rates (spec/scenarios/patch-graph-validation-identifiers.yaml, -rates.yaml)

#[test]
fn the_port_table_is_the_one_of_design_section_5_1() {
    use NodeKind::*;
    use PortRate::*;
    let table = |ports: &[patchback_core::patch::Port]| -> Vec<(&str, PortRate)> {
        ports.iter().map(|p| (p.name, p.rate)).collect()
    };
    type Ports = Vec<(&'static str, PortRate)>;
    let rows: [(NodeKind, Ports, Ports); 7] = [
        (NoteInput, vec![], vec![("pitch", Control), ("gate", Event)]),
        (
            SawOscillator,
            vec![("pitch", Control)],
            vec![("out", Audio)],
        ),
        (Drive, vec![("in", Audio)], vec![("out", Audio)]),
        (LowpassFilter, vec![("in", Audio)], vec![("out", Audio)]),
        (AdsrEnvelope, vec![("gate", Event)], vec![("env", Control)]),
        (
            Vca,
            vec![("in", Audio), ("gain", Control)],
            vec![("out", Audio)],
        ),
        (Output, vec![("in", Audio)], vec![]),
    ];
    for (kind, inputs, outputs) in rows {
        assert_eq!(table(kind.input_ports()), inputs, "{kind:?}");
        assert_eq!(table(kind.output_ports()), outputs, "{kind:?}");
        assert_eq!(kind.ports(PortDirection::Input), kind.input_ports());
        assert_eq!(kind.ports(PortDirection::Output), kind.output_ports());
    }
    for kind in NodeKind::ALL {
        if !NodeKind::MVP.contains(&kind) {
            assert!(kind.input_ports().is_empty(), "{kind:?}");
            assert!(kind.output_ports().is_empty(), "{kind:?}");
            assert!(kind.parameter_paths().is_empty(), "{kind:?}");
        }
    }
}

#[test]
fn identifiers_refuses_a_port_its_node_kind_does_not_declare() {
    use NodeKind::*;
    use PortRate::*;
    let sidechain = edge((2, "out", Audio), (7, "sidechain", Audio));
    let scenario = patch(
        vec![with_defaults(2, SawOscillator), with_defaults(7, Output)],
        vec![sidechain.clone()],
    );
    assert_eq!(
        refused(&scenario),
        ValidationError::UnknownPort {
            edge: sidechain,
            node: 7,
            kind: Output,
            direction: PortDirection::Input,
            port: "sidechain".to_owned()
        }
    );
    // An input port named as a from end is not an output.
    let backwards = edge((7, "in", Control), (2, "pitch", Control));
    let scenario = patch(
        vec![with_defaults(2, SawOscillator), with_defaults(7, Output)],
        vec![backwards.clone()],
    );
    assert_eq!(
        refused(&scenario),
        ValidationError::UnknownPort {
            edge: backwards,
            node: 7,
            kind: Output,
            direction: PortDirection::Output,
            port: "in".to_owned()
        }
    );
}

#[test]
fn rates_refuses_an_audio_port_into_a_control_port() {
    use NodeKind::*;
    use PortRate::*;
    let wrong = edge((2, "out", Audio), (6, "gain", Control));
    let scenario = patch(
        vec![with_defaults(2, SawOscillator), with_defaults(6, Vca)],
        vec![wrong.clone()],
    );
    assert_eq!(
        refused(&scenario),
        ValidationError::IncompatiblePortRates { edge: wrong }
    );
}

#[test]
fn rates_refuses_an_end_that_states_another_rate_than_its_kind_declares() {
    use NodeKind::*;
    use PortRate::*;
    // Both ends agree, but the oscillator's out and the VCA's gain are not Event ports.
    let stated = edge((2, "out", Event), (6, "in", Event));
    let scenario = patch(
        vec![with_defaults(2, SawOscillator), with_defaults(6, Vca)],
        vec![stated.clone()],
    );
    assert_eq!(
        refused(&scenario),
        ValidationError::PortRateMismatch {
            edge: stated,
            node: 2,
            port: "out".to_owned(),
            stated: Event,
            declared: Audio
        }
    );
    let stated = edge((2, "out", Audio), (6, "gain", Audio));
    let scenario = patch(
        vec![with_defaults(2, SawOscillator), with_defaults(6, Vca)],
        vec![stated.clone()],
    );
    assert_eq!(
        refused(&scenario),
        ValidationError::PortRateMismatch {
            edge: stated,
            node: 6,
            port: "gain".to_owned(),
            stated: Audio,
            declared: Control
        }
    );
}

// ---------------------------------------------------------------------------------------------
// The fixed chain (spec/scenarios/patch-graph-validation-fixed-chain.yaml; DESIGN.md § 5.2,
// story:patch-review-fixes)

const FIXED_CHAIN: &str = "patchback.patch.FixedChainMismatch";

fn spec_error_of_patch(patch: &Patch) -> &'static str {
    refused(patch).spec_error()
}

#[test]
fn fixed_chain_refuses_a_patch_with_no_nodes() {
    assert_eq!(spec_error_of_patch(&patch(vec![], vec![])), FIXED_CHAIN);
}

#[test]
fn fixed_chain_refuses_the_voice_without_its_output() {
    let mut voice = default_voice();
    voice.nodes.retain(|n| n.kind != NodeKind::Output);
    voice.edges.retain(|e| e.to_node != 7);
    assert_eq!(spec_error_of_patch(&voice), FIXED_CHAIN);
}

#[test]
fn fixed_chain_refuses_the_voice_without_any_one_of_its_nodes() {
    for kind in NodeKind::MVP {
        let mut voice = default_voice();
        let id = voice
            .nodes
            .iter()
            .find(|n| n.kind == kind)
            .expect("the voice has every kind")
            .id;
        voice.nodes.retain(|n| n.id != id);
        voice.edges.retain(|e| e.from_node != id && e.to_node != id);
        assert_eq!(spec_error_of_patch(&voice), FIXED_CHAIN, "{kind:?}");
    }
}

#[test]
fn fixed_chain_refuses_the_voice_without_any_one_of_its_edges() {
    for index in 0..default_voice().edges.len() {
        let mut voice = default_voice();
        let removed = voice.edges.remove(index);
        assert_eq!(spec_error_of_patch(&voice), FIXED_CHAIN, "{removed:?}");
    }
}

#[test]
fn fixed_chain_refuses_a_second_node_of_any_kind_of_the_voice() {
    for kind in NodeKind::MVP {
        let mut voice = default_voice();
        voice.nodes.push(with_defaults(20, kind));
        assert_eq!(spec_error_of_patch(&voice), FIXED_CHAIN, "{kind:?}");
    }
}

#[test]
fn fixed_chain_the_refusal_names_what_is_missing_or_extra() {
    use NodeKind::*;
    // No nodes: every kind and every edge of the chain is missing.
    assert_eq!(
        refused(&patch(vec![], vec![])),
        ValidationError::FixedChainMismatch {
            missing_kinds: FIXED_CHAIN_KINDS.to_vec(),
            extra_nodes: vec![],
            missing_edges: FIXED_CHAIN_EDGES.to_vec(),
        }
    );
    // The voice without its Output node and the edge into it.
    let mut voice = default_voice();
    voice.nodes.retain(|n| n.id != 7);
    voice.edges.retain(|e| e.to_node != 7);
    let error = refused(&voice);
    assert_eq!(
        error,
        ValidationError::FixedChainMismatch {
            missing_kinds: vec![Output],
            extra_nodes: vec![],
            missing_edges: vec![FIXED_CHAIN_EDGES[6]],
        }
    );
    assert_eq!(
        error.to_string(),
        "the patch is not the fixed chain of subtractive-mono/1: missing nodes Output; \
         missing edges Vca out -> Output in"
    );
    // A second node of a kind: the extra one is the node the chain edges do not use, whatever its
    // identifier, and no chain edge the patch has is named missing.
    let mut voice = default_voice();
    voice.nodes.push(with_defaults(9, Drive));
    voice.nodes.push(with_defaults(0, Vca));
    let error = refused(&voice);
    assert_eq!(
        error,
        ValidationError::FixedChainMismatch {
            missing_kinds: vec![],
            extra_nodes: vec![(0, Vca), (9, Drive)],
            missing_edges: vec![],
        }
    );
    assert_eq!(
        error.to_string(),
        "the patch is not the fixed chain of subtractive-mono/1: extra nodes 0 (Vca), 9 (Drive)"
    );
    // An edge of the chain wired to another port of the same nodes is missing.
    let mut voice = default_voice();
    voice.edges.retain(|e| e.to_port != "gate");
    assert_eq!(
        refused(&voice),
        ValidationError::FixedChainMismatch {
            missing_kinds: vec![],
            extra_nodes: vec![],
            missing_edges: vec![FIXED_CHAIN_EDGES[1]],
        }
    );
}

/// Of several nodes of one kind, the chain uses the one that carries the most chain edges, the
/// lowest identifier among equals; the others are extra. A chain edge is missing only when no
/// nodes of its two kinds carry it.
#[test]
fn fixed_chain_the_node_of_a_kind_is_the_one_the_chain_edges_use() {
    use NodeKind::*;
    use PortRate::*;
    let audio = |from: u32, to: u32| edge((from, "out", Audio), (to, "in", Audio));
    let chain_error = |extra_nodes: Vec<(u32, NodeKind)>, missing_edges: Vec<usize>| {
        ValidationError::FixedChainMismatch {
            missing_kinds: vec![],
            extra_nodes,
            missing_edges: missing_edges
                .into_iter()
                .map(|i| FIXED_CHAIN_EDGES[i])
                .collect(),
        }
    };
    // The chain's edges into the VCA split over two VCAs: node 0 takes the filter, node 6 the
    // envelope and the output. Node 6 carries more, so node 0 is extra; no edge is missing.
    let mut voice = default_voice();
    voice.nodes.push(with_defaults(0, Vca));
    voice.edges.retain(|e| (e.from_node, e.to_node) != (4, 6));
    voice.edges.push(audio(4, 0));
    assert_eq!(refused(&voice), chain_error(vec![(0, Vca)], vec![]));
    // The same split the other way: node 0 takes the filter and the envelope.
    let mut voice = default_voice();
    voice.nodes.push(with_defaults(0, Vca));
    voice.edges.retain(|e| e.to_node != 6);
    voice.edges.push(audio(4, 0));
    voice
        .edges
        .push(edge((5, "env", Control), (0, "gain", Control)));
    assert_eq!(refused(&voice), chain_error(vec![(6, Vca)], vec![]));
    // A tie: Drive 3 takes the oscillator, Drive 9 feeds the filter. The lower identifier is used.
    let mut voice = default_voice();
    voice.nodes.push(with_defaults(9, Drive));
    voice.edges.retain(|e| e.from_node != 3);
    voice.edges.push(audio(9, 4));
    assert_eq!(refused(&voice), chain_error(vec![(9, Drive)], vec![]));
    // A chain edge no node of its kinds carries is missing, beside the extra node.
    let mut voice = default_voice();
    voice.nodes.push(with_defaults(0, Vca));
    voice.edges.retain(|e| e.to_node != 7);
    assert_eq!(refused(&voice), chain_error(vec![(0, Vca)], vec![6]));
    // An edge between a used node and an extra one carries its chain edge.
    let mut voice = default_voice();
    voice.nodes.push(with_defaults(8, Output));
    voice.edges.retain(|e| e.to_node != 7);
    voice.edges.push(audio(6, 8));
    assert_eq!(refused(&voice), chain_error(vec![(7, Output)], vec![]));
}

#[test]
fn fixed_chain_is_the_voice_of_design_section_5_2() {
    use NodeKind::*;
    assert_eq!(FIXED_CHAIN_KINDS, NodeKind::MVP);
    let edges: Vec<(NodeKind, &str, NodeKind, &str)> = FIXED_CHAIN_EDGES
        .iter()
        .map(|e| (e.from_kind, e.from_port, e.to_kind, e.to_port))
        .collect();
    assert_eq!(
        edges,
        [
            (NoteInput, "pitch", SawOscillator, "pitch"),
            (NoteInput, "gate", AdsrEnvelope, "gate"),
            (SawOscillator, "out", Drive, "in"),
            (Drive, "out", LowpassFilter, "in"),
            (LowpassFilter, "out", Vca, "in"),
            (AdsrEnvelope, "env", Vca, "gain"),
            (Vca, "out", Output, "in"),
        ]
    );
    // Each end is a port its kind declares, the two ends at one rate, and the chain feeds every
    // input port of its kinds exactly once.
    let mut fed = Vec::new();
    for chain in FIXED_CHAIN_EDGES {
        let out = chain
            .from_kind
            .output_ports()
            .iter()
            .find(|p| p.name == chain.from_port)
            .expect("an output port");
        let input = chain
            .to_kind
            .input_ports()
            .iter()
            .find(|p| p.name == chain.to_port)
            .expect("an input port");
        assert_eq!(out.rate, input.rate, "{chain}");
        fed.push((chain.to_kind, chain.to_port));
    }
    let mut inputs: Vec<(NodeKind, &str)> = FIXED_CHAIN_KINDS
        .iter()
        .flat_map(|&kind| kind.input_ports().iter().map(move |p| (kind, p.name)))
        .collect();
    fed.sort();
    inputs.sort();
    assert_eq!(fed, inputs);
    // The default voice of these tests is that chain.
    let voice = default_voice();
    for chain in FIXED_CHAIN_EDGES {
        let id = |kind| {
            voice
                .nodes
                .iter()
                .find(|n| n.kind == kind)
                .expect("a node")
                .id
        };
        assert!(
            voice
                .edges
                .iter()
                .any(|e| e.from_node == id(chain.from_kind)
                    && e.from_port == chain.from_port
                    && e.to_node == id(chain.to_kind)
                    && e.to_port == chain.to_port)
        );
    }
}

#[test]
fn fixed_chain_the_voice_is_accepted_whatever_its_node_ids() {
    // The chain is a matter of kinds and ports, not of identifiers.
    let mut voice = default_voice();
    let renumber = |id: u32| 100 - id * 7;
    for node in &mut voice.nodes {
        node.id = renumber(node.id);
    }
    for edge in &mut voice.edges {
        edge.from_node = renumber(edge.from_node);
        edge.to_node = renumber(edge.to_node);
    }
    valid(&voice);
}

/// Every input port of the seven nodes is fed by the chain, so an edge beyond it, between any two
/// ports the § 5.1 table declares at their rate, closes a cycle or feeds a taken input: the later
/// checks answer it, and no patch other than the chain validates.
#[test]
fn fixed_chain_an_edge_beyond_the_chain_is_a_cycle_or_a_taken_input() {
    let voice = default_voice();
    let mut tried = 0;
    for from in &voice.nodes {
        for to in &voice.nodes {
            for out in from.kind.output_ports() {
                for input in to.kind.input_ports().iter().filter(|p| p.rate == out.rate) {
                    let mut extended = default_voice();
                    extended.edges.push(edge(
                        (from.id, out.name, out.rate),
                        (to.id, input.name, input.rate),
                    ));
                    tried += 1;
                    let error = refused(&extended);
                    assert!(
                        matches!(
                            error,
                            ValidationError::ZeroDelayCycle { .. }
                                | ValidationError::InputPortTaken { .. }
                        ),
                        "{from:?} {out:?} -> {to:?} {input:?}: {error}"
                    );
                }
            }
        }
    }
    assert!(tried > voice.edges.len(), "{tried}");
}

// ---------------------------------------------------------------------------------------------
// Cycles (spec/scenarios/patch-graph-validation-cycle-and-caps.yaml; numeric obligation 2:
// cycle detection refuses every cycle)

fn audio(from: u32, to: u32) -> EdgeDecl {
    edge((from, "out", PortRate::Audio), (to, "in", PortRate::Audio))
}

// A cycle needs an edge beyond the fixed chain, between nodes of the chain: a patch with a node
// beyond the chain is refused by the chain check first (story:patch-review-fixes).

#[test]
fn cycle_refuses_a_self_loop() {
    let mut looped = default_voice();
    looped.edges.push(audio(3, 3));
    assert_eq!(
        refused(&looped),
        ValidationError::ZeroDelayCycle { nodes: vec![3] }
    );
}

#[test]
fn cycle_refuses_a_two_node_cycle() {
    let mut scenario = default_voice();
    scenario.edges.push(audio(4, 3));
    assert_eq!(
        refused(&scenario),
        ValidationError::ZeroDelayCycle { nodes: vec![3, 4] }
    );
}

/// The cycle is named from its lowest identifier, not from the node the voice reaches it by: the
/// drive (70) is entered first from the oscillator, the filter (10) is the lowest on the cycle.
#[test]
fn cycle_refuses_a_cycle_and_names_it_from_its_lowest_identifier() {
    let mut voice = renumbered_voice(&[50, 20, 70, 10, 40, 30, 60]);
    voice.edges.push(audio(10, 70));
    assert_eq!(
        refused(&voice),
        ValidationError::ZeroDelayCycle {
            nodes: vec![10, 70]
        }
    );
    // A longer cycle: drive -> filter -> VCA -> drive.
    let mut voice = renumbered_voice(&[50, 20, 70, 10, 40, 30, 60]);
    voice.edges.push(audio(30, 70));
    assert_eq!(
        refused(&voice),
        ValidationError::ZeroDelayCycle {
            nodes: vec![10, 30, 70]
        }
    );
}

#[test]
fn cycle_refuses_a_cycle_downstream_of_the_voice_and_names_only_the_cycle() {
    let mut voice = default_voice();
    // 4 -> 6 is the voice; 6 -> 4 through the filter's input closes a loop past 1, 2 and 3.
    voice.edges.push(audio(6, 4));
    assert_eq!(
        refused(&voice),
        ValidationError::ZeroDelayCycle { nodes: vec![4, 6] }
    );
}

/// The default voice with the node of `FIXED_CHAIN_KINDS[i]` numbered `ids[i]`.
fn renumbered_voice(ids: &[u32]) -> Patch {
    let mut voice = default_voice();
    let id = |old: u32| ids[(old - 1) as usize];
    for node in &mut voice.nodes {
        assert_eq!(FIXED_CHAIN_KINDS[(node.id - 1) as usize], node.kind);
        node.id = id(node.id);
    }
    for edge in &mut voice.edges {
        edge.from_node = id(edge.from_node);
        edge.to_node = id(edge.to_node);
    }
    voice
}

/// Whether `edges` (pairs of indices into `n` nodes) have a cycle, by reachability.
fn has_cycle(n: usize, edges: &[(usize, usize)]) -> bool {
    edges.iter().any(|&(u, v)| reaches(n, edges, v, u))
}

fn reaches(n: usize, edges: &[(usize, usize)], from: usize, to: usize) -> bool {
    let mut seen = vec![false; n];
    let mut stack = vec![from];
    while let Some(at) = stack.pop() {
        if at == to {
            return true;
        }
        if !std::mem::replace(&mut seen[at], true) {
            stack.extend(edges.iter().filter(|e| e.0 == at).map(|e| e.1));
        }
    }
    false
}

/// The lexicographically lowest topological order of node identifiers, by brute force.
fn lowest_order(ids: &[u32], edges: &[(usize, usize)]) -> Vec<u32> {
    let n = ids.len();
    let mut best: Option<Vec<u32>> = None;
    let mut perm: Vec<usize> = (0..n).collect();
    permutations(&mut perm, 0, &mut |order| {
        let position = |i: usize| order.iter().position(|&x| x == i).expect("in order");
        if edges.iter().all(|&(u, v)| position(u) < position(v)) {
            let named: Vec<u32> = order.iter().map(|&i| ids[i]).collect();
            if best.as_ref().is_none_or(|b| named < *b) {
                best = Some(named);
            }
        }
    });
    best.expect("an acyclic graph has an order")
}

fn permutations(items: &mut Vec<usize>, k: usize, visit: &mut impl FnMut(&[usize])) {
    if k == items.len() {
        visit(items);
        return;
    }
    for i in k..items.len() {
        items.swap(k, i);
        permutations(items, k + 1, visit);
        items.swap(k, i);
    }
}

#[test]
fn cycle_detection_refuses_every_cycle_and_schedules_every_acyclic_graph_of_four_nodes() {
    // The fixed chain (story:patch-review-fixes) admits no node beyond its seven, so the graphs
    // swept are the chain plus every subset of the 12 audio edges beyond it: from the oscillator,
    // drive, filter or VCA output into the drive, filter, VCA or output input (16 pairs, self-loops
    // included), less the chain's own four. Each feeds an input the chain already feeds, so an
    // acyclic one is refused for that, after the cycle check, and only the chain itself is
    // scheduled. Identifiers are not in index order, so cycles are named by identifier, not
    // position. Every acyclic graph of the chain's numbering is scheduled by
    // `the_chain_is_scheduled_lowest_identifier_first_under_every_numbering`.
    let ids = [50, 20, 70, 10, 40, 30, 60];
    let index = |kind: NodeKind| {
        FIXED_CHAIN_KINDS
            .iter()
            .position(|&k| k == kind)
            .expect("a chain kind")
    };
    let chain: Vec<(usize, usize)> = FIXED_CHAIN_EDGES
        .iter()
        .map(|e| (index(e.from_kind), index(e.to_kind)))
        .collect();
    let (sources, sinks) = ([1, 2, 3, 5], [2, 3, 5, 6]);
    let pairs: Vec<(usize, usize)> = sources
        .iter()
        .flat_map(|&u| sinks.iter().map(move |&v| (u, v)))
        .filter(|pair| !chain.contains(pair))
        .collect();
    assert_eq!(pairs.len(), 12);
    let mut cyclic = 0;
    let mut shared = 0;
    for mask in 0u32..(1 << pairs.len()) {
        let extra: Vec<(usize, usize)> = pairs
            .iter()
            .enumerate()
            .filter(|(bit, _)| mask & (1 << bit) != 0)
            .map(|(_, pair)| *pair)
            .collect();
        let mut graph = renumbered_voice(&ids);
        graph
            .edges
            .extend(extra.iter().map(|&(u, v)| audio(ids[u], ids[v])));
        let edges: Vec<(usize, usize)> = chain.iter().chain(&extra).copied().collect();
        match validate(&graph) {
            Ok(validated) => {
                assert_eq!(mask, 0, "mask {mask:#x} validated");
                assert_eq!(
                    compile(&validated).order(),
                    lowest_order(&ids, &edges),
                    "mask {mask:#x}"
                );
            }
            Err(ValidationError::ZeroDelayCycle { nodes }) => {
                cyclic += 1;
                assert!(has_cycle(ids.len(), &edges), "mask {mask:#x} is acyclic");
                // The nodes named form a cycle of the graph, from its lowest identifier.
                let index = |id: u32| ids.iter().position(|&x| x == id).expect("a node");
                assert_eq!(nodes.iter().min(), nodes.first(), "mask {mask:#x}");
                assert_eq!(
                    nodes.iter().collect::<BTreeSet<_>>().len(),
                    nodes.len(),
                    "mask {mask:#x}"
                );
                for (at, next) in nodes.iter().zip(nodes.iter().cycle().skip(1)) {
                    assert!(
                        edges.contains(&(index(*at), index(*next))),
                        "mask {mask:#x}: {nodes:?} is not a cycle"
                    );
                }
            }
            // Every extra edge feeds an audio input the chain already feeds: an acyclic graph
            // with one is refused for that, after the cycle check.
            Err(ValidationError::InputPortTaken { edge, earlier }) => {
                shared += 1;
                assert!(!has_cycle(ids.len(), &edges), "mask {mask:#x} has a cycle");
                assert!(!extra.is_empty(), "mask {mask:#x}");
                assert_eq!(
                    (edge.to_node, edge.to_port.as_str()),
                    (earlier.to_node, earlier.to_port.as_str()),
                    "mask {mask:#x}"
                );
            }
            Err(other) => panic!("mask {mask:#x}: {other}"),
        }
    }
    assert!(cyclic > 0);
    assert!(shared > 0);
}

/// The schedule's tie-break, under every numbering of the chain's seven nodes (7! of them): the
/// plan's order is the lexicographically lowest topological order of identifiers, found by brute
/// force over the orders of the seven kinds the chain's edges allow.
#[test]
fn the_chain_is_scheduled_lowest_identifier_first_under_every_numbering() {
    let index = |kind: NodeKind| {
        FIXED_CHAIN_KINDS
            .iter()
            .position(|&k| k == kind)
            .expect("a chain kind")
    };
    let chain: Vec<(usize, usize)> = FIXED_CHAIN_EDGES
        .iter()
        .map(|e| (index(e.from_kind), index(e.to_kind)))
        .collect();
    let mut extensions: Vec<Vec<usize>> = Vec::new();
    let mut kinds: Vec<usize> = (0..FIXED_CHAIN_KINDS.len()).collect();
    permutations(&mut kinds, 0, &mut |order| {
        let position = |i: usize| order.iter().position(|&x| x == i).expect("in order");
        if chain.iter().all(|&(u, v)| position(u) < position(v)) {
            extensions.push(order.to_vec());
        }
    });
    // The note input first, the envelope anywhere among oscillator, drive and filter, then the
    // VCA and the output.
    assert_eq!(extensions.len(), 4);
    let mut numberings = 0;
    let mut assignment: Vec<usize> = (0..FIXED_CHAIN_KINDS.len()).collect();
    permutations(&mut assignment, 0, &mut |assignment| {
        let ids: Vec<u32> = assignment.iter().map(|&i| i as u32 + 1).collect();
        let expected = extensions
            .iter()
            .map(|order| order.iter().map(|&i| ids[i]).collect::<Vec<u32>>())
            .min()
            .expect("an order");
        assert_eq!(
            compile(&valid(&renumbered_voice(&ids))).order(),
            expected,
            "{ids:?}"
        );
        numberings += 1;
    });
    assert_eq!(numberings, 5040);
}

// ---------------------------------------------------------------------------------------------
// Resource caps (spec/scenarios/patch-graph-validation-cycle-and-caps.yaml)

fn vca_bank(count: usize) -> Patch {
    patch(
        (0..count)
            .map(|i| with_defaults(i as u32, NodeKind::Vca))
            .collect(),
        vec![],
    )
}

/// The fixed chain has seven nodes, so a graph past the node cap is refused by the chain check,
/// which `spec/domains/patch.yaml` declares first; the node cap is not reached while every known
/// topology is a fixed chain (story:patch-review-fixes).
#[test]
fn caps_a_graph_past_the_node_cap_is_answered_by_the_fixed_chain_first() {
    assert_eq!(MAX_GRAPH_NODES, 256);
    let mut graph = default_voice();
    graph
        .nodes
        .extend(vca_bank(MAX_GRAPH_NODES).nodes.into_iter().map(|mut n| {
            n.id += 100;
            n
        }));
    assert_eq!(graph.nodes.len(), MAX_GRAPH_NODES + 7);
    match refused(&graph) {
        ValidationError::FixedChainMismatch {
            missing_kinds,
            extra_nodes,
            missing_edges,
        } => {
            assert!(missing_kinds.is_empty() && missing_edges.is_empty());
            assert_eq!(extra_nodes.len(), MAX_GRAPH_NODES);
        }
        other => panic!("{other:?}"),
    }
}

/// The chain with its VCA-to-output edge repeated until the patch has `edges` edges.
fn fan_out(edges: usize) -> Patch {
    let mut graph = default_voice();
    let repeats = edges - graph.edges.len();
    graph
        .edges
        .extend(std::iter::repeat_n(audio(6, 7), repeats));
    graph
}

#[test]
fn caps_a_graph_of_the_edge_cap_passes_the_cap_and_one_more_edge_is_refused() {
    assert_eq!(MAX_GRAPH_EDGES, 1024);
    // An input port takes one edge, so no valid graph reaches the edge cap: at the cap, the graph
    // passes the cap and is refused for its shared inputs, the check after it.
    let at_cap = refused(&fan_out(MAX_GRAPH_EDGES));
    assert!(
        matches!(at_cap, ValidationError::InputPortTaken { .. }),
        "{at_cap:?}"
    );
    assert_eq!(
        refused(&fan_out(MAX_GRAPH_EDGES + 1)),
        ValidationError::ResourceCapExceeded {
            cap: ResourceCap::Edges,
            limit: MAX_GRAPH_EDGES,
            found: MAX_GRAPH_EDGES + 1
        }
    );
}

/// One voice is the chain's one `NoteInput`; a second is a node beyond the chain, refused by the
/// chain check before the voice cap (story:patch-review-fixes).
#[test]
fn caps_one_voice_is_accepted_and_a_second_note_input_is_answered_by_the_fixed_chain_first() {
    use NodeKind::*;
    assert_eq!(MAX_VOICES, 1);
    let one = default_voice();
    valid(&one);
    let mut two = one;
    two.nodes.push(with_defaults(8, NoteInput));
    assert_eq!(
        refused(&two),
        ValidationError::FixedChainMismatch {
            missing_kinds: vec![],
            extra_nodes: vec![(8, NoteInput)],
            missing_edges: vec![],
        }
    );
    for cap in ResourceCap::ALL {
        assert!(cap.limit() > 0, "{cap:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Compile unvalidated (spec/scenarios/patch-graph-validation-compile-unvalidated.yaml)

#[test]
fn compile_unvalidated_the_only_path_to_a_plan_goes_through_validation() {
    // `compile` takes a `ValidatedPatch`; a loaded `Patch` does not type-check there, and a
    // `ValidatedPatch` has no public constructor (the `compile_fail` examples of
    // `patchback_core::patch::compile`). So a refused patch has no plan:
    let mut cycle = default_voice();
    cycle.edges.push(audio(4, 3));
    let plan: Option<ExecutionPlan> = validate(&cycle).ok().map(|v| compile(&v));
    assert_eq!(plan, None);
    // The scenario's loaded patch (an oscillator into the output) is not the fixed chain
    // (story:patch-review-fixes), so it has no plan either,
    let loaded = parse_patch(
        "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n\
         node 2 SawOscillator\nnode 7 Output\nparam 7 output_gain -6\n\
         edge 2 out Audio 7 in Audio\n",
    )
    .expect("loads");
    let plan: Option<ExecutionPlan> = validate(&loaded).ok().map(|v| compile(&v));
    assert_eq!(plan, None);
    // and the loaded voice reaches a plan only once validated.
    let loaded = parse_patch(&voice_text()).expect("loads");
    let plan = compile(&valid(&loaded));
    assert_eq!(plan.order(), [1, 2, 3, 4, 5, 6, 7]);
}

// ---------------------------------------------------------------------------------------------
// One edge per input port (DESIGN.md § 5.1, § 12.1 "duplicates")

#[test]
fn a_duplicated_edge_is_refused_naming_it_twice() {
    let mut voice = default_voice();
    voice.edges.push(audio(6, 7));
    assert_eq!(
        refused(&voice),
        ValidationError::InputPortTaken {
            edge: audio(6, 7),
            earlier: Box::new(audio(6, 7))
        }
    );
}

#[test]
fn a_second_source_into_one_input_is_refused_naming_both_edges() {
    let mut voice = default_voice();
    // 2 -> 6 sorts before 4 -> 6 canonically, so 4 -> 6 is the edge that finds the port taken.
    voice.edges.push(audio(2, 6));
    assert_eq!(
        refused(&voice),
        ValidationError::InputPortTaken {
            edge: audio(4, 6),
            earlier: Box::new(audio(2, 6))
        }
    );
    let text = refused(&voice).to_string();
    assert_eq!(
        text,
        "edge 4 out Audio 6 in Audio: node 6 input port in already takes \
         edge 2 out Audio 6 in Audio; an input port takes one edge"
    );
}

#[test]
fn a_second_edge_out_of_an_output_port_is_refused_for_the_taken_input_it_feeds() {
    // In the fixed chain (story:patch-review-fixes) every input is fed, so a second edge out of an
    // output port feeds a taken input; the refusal names that input, not the output it shares.
    let mut voice = default_voice();
    voice.edges.push(audio(4, 7));
    assert_eq!(
        refused(&voice),
        ValidationError::InputPortTaken {
            edge: audio(6, 7),
            earlier: Box::new(audio(4, 7))
        }
    );
}

#[test]
fn the_vca_takes_one_edge_on_each_of_its_two_inputs() {
    use PortRate::*;
    let mut voice = default_voice();
    voice
        .edges
        .push(edge((1, "pitch", Control), (6, "gain", Control)));
    assert_eq!(
        refused(&voice),
        ValidationError::InputPortTaken {
            edge: edge((5, "env", Control), (6, "gain", Control)),
            earlier: Box::new(edge((1, "pitch", Control), (6, "gain", Control)))
        }
    );
}

#[test]
fn shared_inputs_are_answered_in_canonical_order() {
    let mut voice = default_voice();
    voice.edges.push(audio(6, 7));
    voice.edges.push(audio(2, 3));
    // Canonical order: 2 -> 3 twice comes before 6 -> 7 twice.
    assert_eq!(
        refused(&voice),
        ValidationError::InputPortTaken {
            edge: audio(2, 3),
            earlier: Box::new(audio(2, 3))
        }
    );
}

// ---------------------------------------------------------------------------------------------
// Loading a file: `LoadPatch`'s declared order, the topology and catalogue included
// (`patchback_core::patch::load_patch`)

fn voice_text() -> String {
    write_patch(&default_voice()).expect("writable")
}

fn load(text: &str) -> Result<ValidatedPatch, LoadPatchError> {
    load_patch(text.as_bytes())
}

fn load_refusal(text: &str) -> LoadPatchError {
    match load(text) {
        Ok(validated) => panic!("expected a refusal, loaded {validated:?}"),
        Err(error) => error,
    }
}

fn spec_error_of(text: &str) -> &'static str {
    load_refusal(text)
        .spec_error()
        .expect("a refusal of the domain")
}

const UNKNOWN_TOPOLOGY: (&str, &str) = (
    "topology_id subtractive-mono/1\n",
    "topology_id subtractive-mono/2\n",
);
const MALFORMED_NUMBER: &str = "param 7 extra 1x\n";

#[test]
fn load_a_valid_file_is_the_validated_patch() {
    let validated = load(&voice_text()).expect("loads");
    assert_eq!(validated, valid(&default_voice()));
}

#[test]
fn load_an_unknown_topology_outranks_a_malformed_number_record_or_bound() {
    let unknown = voice_text().replacen(UNKNOWN_TOPOLOGY.0, UNKNOWN_TOPOLOGY.1, 1);
    for later in [MALFORMED_NUMBER, "node 9 Drive extra\n", "bogus\n"] {
        let text = format!("{unknown}{later}");
        assert_eq!(
            spec_error_of(&text),
            "patchback.patch.UnknownTopology",
            "{later:?}"
        );
        assert!(matches!(
            load_refusal(&text),
            LoadPatchError::Graph(ValidationError::UnknownTopology { .. })
        ));
    }
    let long = format!("{unknown}param 7 {} 1\n", "a".repeat(300));
    assert_eq!(spec_error_of(&long), "patchback.patch.UnknownTopology");
}

#[test]
fn load_a_uuid_in_the_topology_field_is_an_unknown_topology() {
    let uuid = voice_text().replacen(
        UNKNOWN_TOPOLOGY.0,
        "topology_id 7f0c0000-0000-4000-8000-000000000011\n",
        1,
    );
    assert_eq!(spec_error_of(&uuid), "patchback.patch.UnknownTopology");
    let text = format!("{uuid}{MALFORMED_NUMBER}");
    assert_eq!(spec_error_of(&text), "patchback.patch.UnknownTopology");
}

#[test]
fn load_a_kind_outside_the_catalogue_outranks_a_malformed_number_and_names_the_lowest_node() {
    let text = format!(
        "{}node 40 Noise\nnode 30 Lfo\n{MALFORMED_NUMBER}",
        voice_text()
    );
    assert!(matches!(
        load_refusal(&text),
        LoadPatchError::Graph(ValidationError::KindOutsideCatalogue {
            node: 30,
            kind: NodeKind::Lfo
        })
    ));
}

#[test]
fn load_the_topology_outranks_the_catalogue() {
    let text = format!(
        "{}node 30 Lfo\n{MALFORMED_NUMBER}",
        voice_text().replacen(UNKNOWN_TOPOLOGY.0, UNKNOWN_TOPOLOGY.1, 1)
    );
    assert_eq!(spec_error_of(&text), "patchback.patch.UnknownTopology");
}

#[test]
fn load_faults_declared_before_the_topology_still_win() {
    let unknown = voice_text().replacen(UNKNOWN_TOPOLOGY.0, UNKNOWN_TOPOLOGY.1, 1);
    let cases = [
        (
            format!("{unknown}node 1 Drive\n"),
            "patchback.patch.DuplicateNodeId",
        ),
        (
            unknown.replacen("sample_rate_hz 48000", "sample_rate_hz 44100", 1),
            "patchback.patch.UnsupportedSampleRate",
        ),
        (
            format!("{unknown}param 3 drive 1\n"),
            "patchback.patch.DuplicateKey",
        ),
        (
            unknown.replacen("patchback-patch 1", "patchback-patch 2", 1),
            "patchback.patch.UnknownFormatVersion",
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(spec_error_of(&text), expected);
        assert!(matches!(load_refusal(&text), LoadPatchError::File(_)));
    }
}

#[test]
fn load_a_line_1_fault_or_an_unreadable_topology_line_leaves_the_parser_answer() {
    let unknown = voice_text().replacen(UNKNOWN_TOPOLOGY.0, UNKNOWN_TOPOLOGY.1, 1);
    // A fault on line 1 is answered alone.
    let header = unknown.replacen("patchback-patch 1", "patchback-patch", 1);
    assert_eq!(spec_error_of(&header), "patchback.patch.MalformedRecord");
    // A topology line the parser refuses names no topology to check.
    let malformed = voice_text().replacen(UNKNOWN_TOPOLOGY.0, "topology_id fm poly\n", 1);
    let text = format!("{malformed}node 30 Lfo\n");
    assert!(matches!(
        load_refusal(&text),
        LoadPatchError::File(ref error)
            if error.reason.spec_error() == "patchback.patch.MalformedRecord"
    ));
}

#[test]
fn load_a_file_past_the_byte_bound_is_answered_alone() {
    let mut text = voice_text().replacen(UNKNOWN_TOPOLOGY.0, UNKNOWN_TOPOLOGY.1, 1);
    while text.len() <= patchback_core::patch::MAX_FILE_BYTES {
        text.push_str("node 9 Drive\n");
    }
    assert_eq!(spec_error_of(&text), "patchback.patch.LengthBoundExceeded");
}

#[test]
fn load_the_graph_checks_follow_a_well_formed_file() {
    let text = format!("{}edge 6 out Audio 7 in Audio\n", voice_text());
    assert_eq!(spec_error_of(&text), "patchback.patch.InputPortTaken");
}

// ---------------------------------------------------------------------------------------------
// Precedence: a patch with several faults is refused with the one whose error
// `spec/domains/patch.yaml` declares first.

/// One fault per rank of [`VALIDATE_PATCH_PRECEDENCE`], each added to the default voice alone.
fn inject(rank: usize, voice: &mut Patch) {
    use NodeKind::*;
    use PortRate::*;
    match rank {
        0 => voice.format_version = 2,
        1 => voice.sample_rate_hz = 44_100,
        2 => voice.nodes.push(with_defaults(6, Vca)),
        3 => voice.nodes[2].parameters.push(value("drive", 1.0)),
        4 => voice.topology_id = "unknown/1".to_owned(),
        5 => voice.nodes.push(node(200, Lfo, vec![])),
        6 => voice.edges.push(audio(2, 300)),
        7 => voice
            .edges
            .push(edge((2, "out", Audio), (6, "gain", Control))),
        8 => voice.nodes[6].parameters[0].value = 10.0,
        9 => voice
            .edges
            .push(edge((2, "out", Audio), (7, "sidechain", Audio))),
        10 => voice.edges.push(edge((2, "out", Event), (6, "in", Event))),
        // The chain without its gate edge.
        11 => voice.edges.retain(|e| e.to_port != "gate"),
        // The filter feeds the drive: 3 -> 4 -> 3. No node is added, so the chain still holds.
        12 => voice.edges.push(audio(4, 3)),
        // The only cap a patch of the fixed chain can exceed: the edges, by repeating one.
        13 => voice
            .edges
            .extend(std::iter::repeat_n(audio(6, 7), MAX_GRAPH_EDGES - 6)),
        14 => voice.edges.push(audio(6, 7)),
        _ => unreachable!(),
    }
}

#[test]
fn validate_patch_precedence_is_the_declaration_order_of_the_spec() {
    let domain = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../spec/domains/patch.yaml"
    ))
    .expect("spec/domains/patch.yaml");
    let outcomes_of = |command: &str| -> Vec<String> {
        domain
            .lines()
            .skip_while(|l| l.trim() != format!("- name: {command}"))
            .skip(1)
            .take_while(|l| !l.starts_with("  - name: "))
            .filter_map(|l| l.trim().strip_prefix("error: "))
            .map(str::to_owned)
            .collect()
    };
    let mut declared: Vec<String> = Vec::new();
    for error in outcomes_of("patchback.patch.LoadPatch")
        .into_iter()
        .chain(outcomes_of("patchback.patch.ValidatePatch"))
    {
        if VALIDATE_PATCH_PRECEDENCE.contains(&error.as_str()) && !declared.contains(&error) {
            declared.push(error);
        }
    }
    assert_eq!(declared, VALIDATE_PATCH_PRECEDENCE);
}

#[test]
fn each_fault_alone_is_refused_with_its_rank() {
    for (rank, spec_error) in VALIDATE_PATCH_PRECEDENCE.iter().enumerate() {
        let mut voice = default_voice();
        inject(rank, &mut voice);
        let error = refused(&voice);
        assert_eq!(error.precedence(), rank, "{error}");
        assert_eq!(error.spec_error(), *spec_error);
    }
}

#[test]
fn of_two_faults_the_first_declared_is_answered() {
    for first in 0..VALIDATE_PATCH_PRECEDENCE.len() {
        for second in first + 1..VALIDATE_PATCH_PRECEDENCE.len() {
            for order in [[first, second], [second, first]] {
                let mut voice = default_voice();
                for rank in order {
                    inject(rank, &mut voice);
                }
                let error = refused(&voice);
                assert_eq!(error.precedence(), first, "{first} and {second}: {error}");
            }
        }
    }
}

#[test]
fn faults_of_one_error_are_answered_in_canonical_order() {
    use PortRate::*;
    let mut voice = default_voice();
    voice.edges.push(audio(6, 90));
    voice.edges.push(audio(3, 80));
    assert_eq!(
        refused(&voice),
        ValidationError::UnknownNode {
            edge: audio(3, 80),
            node: 80
        }
    );
    let mut voice = default_voice();
    voice.nodes[6].parameters[0].value = 10.0;
    voice.nodes[2].parameters[0].value = -1.0;
    voice.nodes.reverse();
    assert!(matches!(
        refused(&voice),
        ValidationError::ParameterOutsideContract { node: 3, .. }
    ));
    let mut voice = default_voice();
    voice.edges.push(edge((6, "out", Audio), (7, "aux", Audio)));
    voice
        .edges
        .push(edge((2, "out", Audio), (7, "side", Audio)));
    assert!(matches!(
        refused(&voice),
        ValidationError::UnknownPort { ref port, .. } if port == "side"
    ));
}

// ---------------------------------------------------------------------------------------------
// The class: every refusal names an error of the spec domain and its node or edge.

/// Exhaustive on purpose: a new refusal does not compile until it is listed here.
fn ordinal(error: &ValidationError) -> usize {
    match error {
        ValidationError::UnknownFormatVersion { .. } => 0,
        ValidationError::UnsupportedSampleRate { .. } => 1,
        ValidationError::DuplicateNodeId { .. } => 2,
        ValidationError::DuplicateParameter { .. } => 3,
        ValidationError::UnknownTopology { .. } => 4,
        ValidationError::KindOutsideCatalogue { .. } => 5,
        ValidationError::UnknownNode { .. } => 6,
        ValidationError::UnknownParameter { .. } => 7,
        ValidationError::MissingParameter { .. } => 8,
        ValidationError::ParameterOutsideContract { .. } => 9,
        ValidationError::UnknownPort { .. } => 10,
        ValidationError::PortRateMismatch { .. } => 11,
        ValidationError::IncompatiblePortRates { .. } => 12,
        ValidationError::FixedChainMismatch { .. } => 13,
        ValidationError::ZeroDelayCycle { .. } => 14,
        ValidationError::ResourceCapExceeded { .. } => 15,
        ValidationError::InputPortTaken { .. } => 16,
    }
}

const REFUSALS: usize = 17;

#[test]
fn every_refusal_names_an_error_of_the_spec_domain_and_where_it_is() {
    let domain = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../spec/domains/patch.yaml"
    ))
    .expect("spec/domains/patch.yaml");
    let declared: BTreeSet<&str> = domain
        .lines()
        .skip_while(|l| *l != "errors:")
        .take_while(|l| *l != "commands:")
        .filter_map(|l| l.trim().strip_prefix("- name: "))
        .collect();
    let mut samples: Vec<ValidationError> = (0..VALIDATE_PATCH_PRECEDENCE.len())
        .map(|rank| {
            let mut voice = default_voice();
            inject(rank, &mut voice);
            refused(&voice)
        })
        .collect();
    let mut voice = default_voice();
    voice.nodes[1].parameters.push(value("drive", 1.0));
    samples.push(refused(&voice));
    let mut voice = default_voice();
    voice.nodes[2].parameters.clear();
    samples.push(refused(&voice));
    let mut reached = BTreeSet::new();
    for error in &samples {
        reached.insert(ordinal(error));
        let name = error.spec_error();
        assert!(
            declared.contains(name),
            "{error:?} names {name}, undeclared"
        );
        assert!(
            error.precedence() < VALIDATE_PATCH_PRECEDENCE.len(),
            "{error:?}"
        );
        let text = error.to_string();
        assert!(!text.contains('\n'), "{text}");
        let located = match error {
            ValidationError::UnknownFormatVersion { .. }
            | ValidationError::UnsupportedSampleRate { .. }
            | ValidationError::UnknownTopology { .. }
            | ValidationError::FixedChainMismatch { .. }
            | ValidationError::ResourceCapExceeded { .. } => true,
            ValidationError::UnknownNode { .. }
            | ValidationError::UnknownPort { .. }
            | ValidationError::PortRateMismatch { .. }
            | ValidationError::IncompatiblePortRates { .. }
            | ValidationError::InputPortTaken { .. } => text.starts_with("edge "),
            ValidationError::ZeroDelayCycle { .. } => text.contains("nodes "),
            _ => text.starts_with("node "),
        };
        assert!(located, "{text}");
    }
    assert_eq!(reached, (0..REFUSALS).collect::<BTreeSet<_>>());
}

// ---------------------------------------------------------------------------------------------

fn next_up(v: f64) -> f64 {
    if v == 0.0 {
        f64::from_bits(1)
    } else if v > 0.0 {
        f64::from_bits(v.to_bits() + 1)
    } else {
        f64::from_bits(v.to_bits() - 1)
    }
}

fn next_down(v: f64) -> f64 {
    -next_up(-v)
}
