//! The obligations of story:patch-graph-validation (DESIGN.md § 2 invariants 3 and 5, § 5.1,
//! § 5.4, § 12.1, § 12.3). The ESS scenarios `spec/scenarios/patch-graph-validation-*.yaml` name
//! the command outcomes; these tests hold the checks, the precedence, the caps, the cycle search
//! and the deterministic schedule they cannot express.

use std::collections::BTreeSet;

use patchback_core::parameter::{SUBTRACTIVE_MONO, Topology};
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

#[test]
fn unconnected_outputs_keep_their_buffers_and_unconnected_inputs_read_none() {
    use NodeKind::*;
    let plan = compile(&valid(&patch(
        vec![with_defaults(9, Vca), with_defaults(4, NoteInput)],
        vec![],
    )));
    assert_eq!(plan.order(), [4, 9]);
    assert_eq!(plan.buffer_count(), 3);
    assert!(plan.steps().iter().all(|s| s.inputs.is_empty()));
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
// Cycles (spec/scenarios/patch-graph-validation-cycle-and-caps.yaml; numeric obligation 2:
// cycle detection refuses every cycle)

fn audio(from: u32, to: u32) -> EdgeDecl {
    edge((from, "out", PortRate::Audio), (to, "in", PortRate::Audio))
}

#[test]
fn cycle_refuses_a_self_loop() {
    let looped = patch(vec![with_defaults(3, NodeKind::Drive)], vec![audio(3, 3)]);
    assert_eq!(
        refused(&looped),
        ValidationError::ZeroDelayCycle { nodes: vec![3] }
    );
}

#[test]
fn cycle_refuses_a_two_node_cycle() {
    use NodeKind::*;
    let scenario = patch(
        vec![with_defaults(3, Drive), with_defaults(4, LowpassFilter)],
        vec![audio(3, 4), audio(4, 3)],
    );
    assert_eq!(
        refused(&scenario),
        ValidationError::ZeroDelayCycle { nodes: vec![3, 4] }
    );
}

#[test]
fn cycle_refuses_a_cycle_not_reachable_from_the_note_input() {
    use NodeKind::*;
    use PortRate::*;
    let island = patch(
        vec![
            with_defaults(1, NoteInput),
            with_defaults(2, SawOscillator),
            with_defaults(7, Output),
            with_defaults(10, Vca),
            with_defaults(11, Drive),
            with_defaults(12, LowpassFilter),
        ],
        vec![
            edge((1, "pitch", Control), (2, "pitch", Control)),
            audio(2, 7),
            audio(11, 12),
            audio(12, 10),
            audio(10, 11),
        ],
    );
    assert_eq!(
        refused(&island),
        ValidationError::ZeroDelayCycle {
            nodes: vec![10, 11, 12]
        }
    );
}

#[test]
fn cycle_refuses_a_cycle_downstream_of_the_voice_and_names_only_the_cycle() {
    let mut voice = default_voice();
    voice.nodes.push(with_defaults(8, NodeKind::Drive));
    // 6 -> 7 is the voice; 6 -> 8 -> 6 through the VCA's audio input closes a loop.
    voice.edges.push(audio(6, 8));
    voice.edges.push(audio(8, 6));
    assert_eq!(
        refused(&voice),
        ValidationError::ZeroDelayCycle { nodes: vec![6, 8] }
    );
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
    // Four Drive nodes, audio in to audio out: every subset of the 16 directed pairs (self-loops
    // included) is a graph. Identifiers are not in index order, so the schedule's tie-break is
    // tested against identifiers, not positions.
    let ids = [30, 10, 40, 20];
    let pairs: Vec<(usize, usize)> = (0..4).flat_map(|u| (0..4).map(move |v| (u, v))).collect();
    let mut cyclic = 0;
    let mut shared = 0;
    for mask in 0u32..(1 << pairs.len()) {
        let edges: Vec<(usize, usize)> = pairs
            .iter()
            .enumerate()
            .filter(|(bit, _)| mask & (1 << bit) != 0)
            .map(|(_, pair)| *pair)
            .collect();
        let graph = patch(
            ids.iter()
                .map(|&id| with_defaults(id, NodeKind::Drive))
                .collect(),
            edges.iter().map(|&(u, v)| audio(ids[u], ids[v])).collect(),
        );
        match validate(&graph) {
            Ok(validated) => {
                assert!(!has_cycle(4, &edges), "mask {mask:#x} has a cycle");
                assert!(!shares_an_input(&edges), "mask {mask:#x} shares an input");
                assert_eq!(
                    compile(&validated).order(),
                    lowest_order(&ids, &edges),
                    "mask {mask:#x}"
                );
            }
            Err(ValidationError::ZeroDelayCycle { nodes }) => {
                cyclic += 1;
                assert!(has_cycle(4, &edges), "mask {mask:#x} is acyclic");
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
            // Each Drive has one input port: an acyclic graph with two edges into one node is
            // refused for that, after the cycle check.
            Err(ValidationError::InputPortTaken { edge, earlier }) => {
                shared += 1;
                assert!(!has_cycle(4, &edges), "mask {mask:#x} has a cycle");
                assert!(shares_an_input(&edges), "mask {mask:#x}");
                assert_eq!(edge.to_node, earlier.to_node, "mask {mask:#x}");
            }
            Err(other) => panic!("mask {mask:#x}: {other}"),
        }
    }
    assert!(cyclic > 0);
    assert!(shared > 0);
}

/// Whether two of `edges` end at one node (each node of the sweep has one input port).
fn shares_an_input(edges: &[(usize, usize)]) -> bool {
    let ends: BTreeSet<usize> = edges.iter().map(|e| e.1).collect();
    ends.len() < edges.len()
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

#[test]
fn caps_a_graph_of_the_node_cap_is_accepted_and_one_more_node_is_refused() {
    assert_eq!(MAX_GRAPH_NODES, 256);
    valid(&vca_bank(MAX_GRAPH_NODES));
    assert_eq!(
        refused(&vca_bank(MAX_GRAPH_NODES + 1)),
        ValidationError::ResourceCapExceeded {
            cap: ResourceCap::Nodes,
            limit: MAX_GRAPH_NODES,
            found: MAX_GRAPH_NODES + 1
        }
    );
}

fn fan_out(edges: usize) -> Patch {
    let mut graph = vca_bank(9);
    graph
        .nodes
        .push(with_defaults(100, NodeKind::SawOscillator));
    graph.edges = (0..edges).map(|i| audio(100, (i % 9) as u32)).collect();
    graph
}

#[test]
fn caps_a_graph_of_the_edge_cap_passes_the_cap_and_one_more_edge_is_refused() {
    assert_eq!(MAX_GRAPH_EDGES, 1024);
    // An input port takes one edge, so no valid graph of at most `MAX_GRAPH_NODES` nodes (two
    // input ports each at most) reaches the edge cap: at the cap, the graph passes the cap and is
    // refused for its shared inputs, the check after it.
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

#[test]
fn caps_one_voice_is_accepted_and_a_second_note_input_is_refused() {
    use NodeKind::*;
    use PortRate::*;
    assert_eq!(MAX_VOICES, 1);
    let one = patch(
        vec![
            with_defaults(1, NoteInput),
            with_defaults(3, SawOscillator),
            with_defaults(7, Output),
        ],
        vec![
            edge((1, "pitch", Control), (3, "pitch", Control)),
            audio(3, 7),
        ],
    );
    valid(&one);
    let mut two = one;
    two.nodes.push(with_defaults(2, NoteInput));
    assert_eq!(
        refused(&two),
        ValidationError::ResourceCapExceeded {
            cap: ResourceCap::Voices,
            limit: MAX_VOICES,
            found: 2
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
    let cycle = patch(
        vec![
            with_defaults(3, NodeKind::Drive),
            with_defaults(4, NodeKind::LowpassFilter),
        ],
        vec![audio(3, 4), audio(4, 3)],
    );
    let plan: Option<ExecutionPlan> = validate(&cycle).ok().map(|v| compile(&v));
    assert_eq!(plan, None);
    // and the scenario's loaded patch reaches a plan only once validated.
    let loaded = parse_patch(
        "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n\
         node 2 SawOscillator\nnode 7 Output\nparam 7 output_gain -6\n\
         edge 2 out Audio 7 in Audio\n",
    )
    .expect("loads");
    let plan = compile(&valid(&loaded));
    assert_eq!(plan.order(), [2, 7]);
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
fn an_output_port_feeds_any_number_of_edges() {
    let mut voice = default_voice();
    for id in 10..14 {
        voice.nodes.push(with_defaults(id, NodeKind::Drive));
        voice.edges.push(audio(4, id));
    }
    let plan = compile(&valid(&voice));
    let readers = plan
        .steps()
        .iter()
        .filter(|s| s.inputs.iter().any(|i| i.from_node == 4))
        .count();
    assert_eq!(readers, 5);
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
        11 => {
            voice.nodes.push(with_defaults(400, Drive));
            voice.nodes.push(with_defaults(401, LowpassFilter));
            voice.edges.push(audio(400, 401));
            voice.edges.push(audio(401, 400));
        }
        12 => voice.nodes.push(with_defaults(500, NoteInput)),
        13 => voice.edges.push(audio(6, 7)),
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
        ValidationError::ZeroDelayCycle { .. } => 13,
        ValidationError::ResourceCapExceeded { .. } => 14,
        ValidationError::InputPortTaken { .. } => 15,
    }
}

const REFUSALS: usize = 16;

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
