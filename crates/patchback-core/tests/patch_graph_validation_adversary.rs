//! Adversary cases for story:patch-graph-validation (wave a3, pass 1). Each case drives the
//! implementation from a document the unit ships: DESIGN.md § 5.1 and § 12.1, the acceptance
//! scenarios in `spec/scenarios/`, and the doc comments of `patchback_core::patch::compile`.

use patchback_core::patch::{
    EdgeDecl, FORMAT_VERSION, NodeDecl, NodeKind, ParameterValue, Patch, PortRate,
    SUPPORTED_SAMPLE_RATE_HZ, ValidationError, compile, validate,
};

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

/// The voice of `spec/scenarios/patch-graph-validation-accepted.yaml`, field for field, with the
/// topology identifier the caller gives.
fn scenario_voice(topology_id: &str) -> Patch {
    use NodeKind::*;
    use PortRate::*;
    Patch {
        format_version: FORMAT_VERSION,
        sample_rate_hz: SUPPORTED_SAMPLE_RATE_HZ,
        topology_id: topology_id.to_owned(),
        nodes: vec![
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
        ],
        edges: vec![
            edge((1, "pitch", Control), (2, "pitch", Control)),
            edge((1, "gate", Event), (5, "gate", Event)),
            edge((2, "out", Audio), (3, "in", Audio)),
            edge((3, "out", Audio), (4, "in", Audio)),
            edge((4, "out", Audio), (6, "in", Audio)),
            edge((5, "env", Control), (6, "gain", Control)),
            edge((6, "out", Audio), (7, "in", Audio)),
        ],
    }
}

/// `patch-graph-validation-accepted.yaml` declares topology `7f0c...0011` named
/// `subtractive-mono/1` and loads a patch referencing it. A `.patch` file names its topology by
/// `TopologyName` (DESIGN.md § 5.1 `topology_id: String`, § 11); the Uuid is the model's identity
/// of the topology carrying that name (`spec/domains/patch.yaml` `LoadPatch`). So the declared
/// name validates, and a Uuid in that field names no topology: `UnknownTopology`.
#[test]
fn adversary_the_accepted_scenario_patch_validates_with_the_topology_id_it_names() {
    let named = scenario_voice("subtractive-mono/1");
    if let Err(error) = validate(&named) {
        panic!(
            "the accepted scenario's patch, by its topology's name, is refused: {} ({error})",
            error.spec_error()
        );
    }
    let uuid = "7f0c0000-0000-4000-8000-000000000011";
    assert_eq!(
        validate(&scenario_voice(uuid)).expect_err("a Uuid is no topology name"),
        ValidationError::UnknownTopology {
            topology_id: uuid.to_owned()
        }
    );
}

/// DESIGN.md § 12.1 Graph: "nonexistent ports/nodes, duplicates". The same edge twice is a
/// duplicate; it compiles to two inputs on one port reading one buffer, so a renderer that sums
/// an input's edges doubles the signal.
#[test]
fn adversary_a_duplicated_edge_is_refused() {
    let mut voice = scenario_voice("subtractive-mono/1");
    voice.edges.push(edge(
        (6, "out", PortRate::Audio),
        (7, "in", PortRate::Audio),
    ));
    match validate(&voice) {
        Err(_) => {}
        Ok(validated) => {
            let plan = compile(&validated);
            let output = plan
                .steps()
                .iter()
                .find(|s| s.node == 7)
                .expect("node 7 is scheduled");
            panic!(
                "a duplicated edge validated and compiled; node 7 reads {:?}",
                output.inputs
            );
        }
    }
}

/// `PlanStep::inputs`: "One per edge into the node, in the kind's input port order, then by
/// buffer." Here the envelope (node 2) has a lower identifier than the filter (node 4), so the
/// gain edge sorts before the audio edge canonically; the VCA must still list `in` then `gain`.
/// The suite's only test of a VCA's inputs has the audio source at the lower identifier, so the
/// port-order sort in `compile` is untested.
#[test]
fn adversary_inputs_are_in_port_order_when_the_gain_source_has_the_lower_id() {
    use NodeKind::*;
    use PortRate::*;
    let mut voice = scenario_voice("subtractive-mono/1");
    voice.nodes = vec![
        node(1, NoteInput, vec![]),
        node(
            2,
            AdsrEnvelope,
            vec![
                value("attack", 0.01),
                value("decay", 0.2),
                value("sustain", 0.6),
                value("release", 0.3),
            ],
        ),
        node(3, SawOscillator, vec![]),
        node(
            4,
            LowpassFilter,
            vec![value("filter_cutoff", 1200.0), value("resonance_q", 0.7)],
        ),
        // The fixed chain's drive stage (story:patch-review-fixes), between oscillator and filter.
        node(5, Drive, vec![value("drive", 6.0)]),
        node(6, Vca, vec![]),
        node(7, Output, vec![value("output_gain", -6.0)]),
    ];
    voice.edges = vec![
        edge((1, "pitch", Control), (3, "pitch", Control)),
        edge((1, "gate", Event), (2, "gate", Event)),
        edge((3, "out", Audio), (5, "in", Audio)),
        edge((5, "out", Audio), (4, "in", Audio)),
        edge((4, "out", Audio), (6, "in", Audio)),
        edge((2, "env", Control), (6, "gain", Control)),
        edge((6, "out", Audio), (7, "in", Audio)),
    ];
    let plan = compile(&validate(&voice).expect("valid"));
    let vca = plan
        .steps()
        .iter()
        .find(|s| s.node == 6)
        .expect("the VCA is scheduled");
    let ports: Vec<(&str, u32)> = vca.inputs.iter().map(|i| (i.port, i.from_node)).collect();
    assert_eq!(ports, [("in", 4), ("gain", 2)]);
}

/// The refusal line of an edge whose rates disagree is user-facing (`patchback patch validate`).
/// It reads "an {from} port cannot feed a {to} port" with the rate tokens `Audio`, `Control`,
/// `Event`, so Control into Audio prints "an Control port cannot feed a Audio port".
#[test]
fn adversary_the_incompatible_rates_message_reads_as_english() {
    use NodeKind::*;
    use PortRate::*;
    let patch = Patch {
        format_version: FORMAT_VERSION,
        sample_rate_hz: SUPPORTED_SAMPLE_RATE_HZ,
        topology_id: "subtractive-mono/1".to_owned(),
        nodes: vec![
            node(1, NoteInput, vec![]),
            node(3, Drive, vec![value("drive", 6.0)]),
        ],
        edges: vec![edge((1, "pitch", Control), (3, "in", Audio))],
    };
    let error = validate(&patch).expect_err("refused");
    assert!(matches!(
        error,
        ValidationError::IncompatiblePortRates { .. }
    ));
    let text = error.to_string();
    assert!(
        !text.contains("an Control") && !text.contains("a Audio"),
        "{text}"
    );
}
