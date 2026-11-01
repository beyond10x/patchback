//! story:studio-patch-graph: the drawn graph of `patchback-studio export` against the four
//! authored scenarios of `patchback.studio.DrawGraph`, and the exporter's refusals.

mod common;

use std::fs;
use std::process::Command;

use common::{demo_patch, is_uuid_v8, json, run_dir, scenario, scratch};
use patchback_core::patch::{EdgeDecl, NodeDecl, NodeKind, Patch, PortRate};
use patchback_studio::StudioError;
use patchback_studio::export::export_run;
use patchback_studio::graph::{self, GraphDrawn, GraphEdge, GraphNode};

fn node(node_id: u32, kind: NodeKind, rank: u32) -> GraphNode {
    GraphNode {
        node_id,
        kind,
        rank,
    }
}

fn edge(
    from_node: u32,
    from_port: &str,
    to_node: u32,
    to_port: &str,
    rate: PortRate,
    delayed_feedback: bool,
) -> GraphEdge {
    GraphEdge {
        from_node,
        from_port: from_port.to_owned(),
        to_node,
        to_port: to_port.to_owned(),
        rate,
        delayed_feedback,
    }
}

fn drawn(nodes: Vec<GraphNode>, edges: Vec<GraphEdge>) -> GraphDrawn {
    GraphDrawn {
        graph_view_id: "5a0d0000-0000-4000-8000-000000000201".to_owned(),
        patch_id: "5a0d0000-0000-4000-8000-000000000301".to_owned(),
        topology_id: "5a0d0000-0000-4000-8000-000000000101".to_owned(),
        nodes,
        edges,
    }
}

fn decl(id: u32, kind: NodeKind) -> NodeDecl {
    NodeDecl {
        id,
        kind,
        parameters: Vec::new(),
    }
}

fn audio(from_node: u32, to_node: u32) -> EdgeDecl {
    EdgeDecl {
        from_node,
        from_port: "out".to_owned(),
        from_rate: PortRate::Audio,
        to_node,
        to_port: "in".to_owned(),
        to_rate: PortRate::Audio,
    }
}

fn code_patch(nodes: Vec<NodeDecl>, edges: Vec<EdgeDecl>) -> Patch {
    Patch {
        format_version: 1,
        sample_rate_hz: 48_000,
        topology_id: "subtractive-mono/1".to_owned(),
        nodes,
        edges,
    }
}

/// The ranks and feedback marks of a drawing, by node id and by edge.
fn ranks(graph: &GraphDrawn) -> Vec<(u32, u32)> {
    graph.nodes.iter().map(|n| (n.node_id, n.rank)).collect()
}

fn feedback(graph: &GraphDrawn) -> Vec<(u32, u32)> {
    graph
        .edges
        .iter()
        .filter(|e| e.delayed_feedback)
        .map(|e| (e.from_node, e.to_node))
        .collect()
}

#[test]
fn studio_patch_graph_subtractive_voice_scenario_is_drawn() {
    scenario("studio-patch-graph-subtractive-voice");
    let input = drawn(
        vec![
            node(1, NodeKind::NoteInput, 0),
            node(2, NodeKind::SawOscillator, 1),
            node(3, NodeKind::Drive, 2),
            node(4, NodeKind::LowpassFilter, 3),
            node(5, NodeKind::AdsrEnvelope, 1),
            node(6, NodeKind::Vca, 4),
            node(7, NodeKind::Output, 5),
        ],
        vec![
            edge(1, "note", 2, "note", PortRate::Event, false),
            edge(1, "gate", 5, "gate", PortRate::Event, false),
            edge(2, "out", 3, "in", PortRate::Audio, false),
            edge(3, "out", 4, "in", PortRate::Audio, false),
            edge(4, "out", 6, "in", PortRate::Audio, false),
            edge(5, "env", 6, "gain", PortRate::Control, false),
            edge(6, "out", 7, "in", PortRate::Audio, false),
        ],
    );
    assert_eq!(graph::check(&input), Ok(()));
}

#[test]
fn studio_patch_graph_subtractive_voice_export_ranks_the_fixed_chain() {
    scenario("studio-patch-graph-subtractive-voice");
    let dir = run_dir("voice", &demo_patch(), None, None);
    let document = json(&export_run(&dir).expect("the demo voice exports"));
    let graph = &document["graph"];
    let nodes: Vec<(u64, &str, u64)> = graph["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|n| {
            (
                n["node_id"].as_u64().expect("node_id"),
                n["kind"].as_str().expect("kind"),
                n["rank"].as_u64().expect("rank"),
            )
        })
        .collect();
    assert_eq!(
        nodes,
        [
            (1, "NoteInput", 0),
            (2, "SawOscillator", 1),
            (3, "Drive", 2),
            (4, "LowpassFilter", 3),
            (5, "AdsrEnvelope", 1),
            (6, "Vca", 4),
            (7, "Output", 5),
        ]
    );
    let edges = graph["edges"].as_array().expect("edges");
    assert_eq!(edges.len(), 7);
    assert!(edges.iter().all(|e| e["delayed_feedback"] == false));
    let rates: Vec<&str> = edges
        .iter()
        .map(|e| e["rate"].as_str().expect("rate"))
        .collect();
    assert_eq!(
        rates,
        [
            "Event", "Control", "Audio", "Audio", "Audio", "Control", "Audio"
        ]
    );
    for key in ["graph_view_id", "patch_id", "topology_id"] {
        let id = graph[key].as_str().expect(key);
        assert!(is_uuid_v8(id), "{key} {id}");
    }
    assert_eq!(document["topology"], "subtractive-mono/1");

    // what the graph view needs beside GraphDrawn: each kind's ports, rates and contracts
    let modules = document["modules"].as_array().expect("modules");
    let kinds: Vec<&str> = modules
        .iter()
        .map(|m| m["kind"].as_str().expect("kind"))
        .collect();
    assert_eq!(
        kinds,
        [
            "NoteInput",
            "SawOscillator",
            "Drive",
            "LowpassFilter",
            "AdsrEnvelope",
            "Vca",
            "Output"
        ]
    );
    let vca = &modules[5];
    let ports: Vec<(&str, &str, &str)> = vca["ports"]
        .as_array()
        .expect("ports")
        .iter()
        .map(|p| {
            (
                p["name"].as_str().expect("name"),
                p["direction"].as_str().expect("direction"),
                p["rate"].as_str().expect("rate"),
            )
        })
        .collect();
    assert_eq!(
        ports,
        [
            ("in", "Input", "Audio"),
            ("gain", "Input", "Control"),
            ("out", "Output", "Audio")
        ]
    );
    let filter = &modules[3]["parameters"];
    assert_eq!(filter[0]["path"], "filter_cutoff");
    assert_eq!(filter[0]["index"], 4);
    assert_eq!(filter[0]["mapping"], "Exponential");
    assert_eq!(filter[0]["physical_at_zero"], "40");
    assert_eq!(filter[0]["physical_at_one"], "16000");
}

#[test]
fn studio_patch_graph_delayed_feedback_scenario_is_drawn() {
    scenario("studio-patch-graph-delayed-feedback");
    let input = drawn(
        vec![node(3, NodeKind::Drive, 0), node(10, NodeKind::Delay, 1)],
        vec![
            edge(3, "out", 10, "in", PortRate::Audio, false),
            edge(10, "out", 3, "in", PortRate::Audio, true),
        ],
    );
    assert_eq!(graph::check(&input), Ok(()));
}

#[test]
fn studio_patch_graph_delayed_feedback_drawing_marks_the_edge_leaving_the_delay() {
    scenario("studio-patch-graph-delayed-feedback");
    // The scenario's loop: Drive -> Delay -> Drive.
    let patch = code_patch(
        vec![decl(3, NodeKind::Drive), decl(10, NodeKind::Delay)],
        vec![audio(3, 10), audio(10, 3)],
    );
    let drawing = graph::drawing(&patch, b"loop");
    assert_eq!(ranks(&drawing), [(3, 0), (10, 1)]);
    assert_eq!(feedback(&drawing), [(10, 3)]);
    assert_eq!(graph::check(&drawing), Ok(()));

    // A chain with the loop inside it: only the edge that closes the cycle is feedback; the
    // Delay's forward edge to the output is not, and the rest ranks as a chain.
    let patch = code_patch(
        vec![
            decl(2, NodeKind::SawOscillator),
            decl(3, NodeKind::Drive),
            decl(7, NodeKind::Output),
            decl(10, NodeKind::Delay),
        ],
        vec![audio(2, 3), audio(3, 10), audio(10, 3), audio(10, 7)],
    );
    let drawing = graph::drawing(&patch, b"chain");
    assert_eq!(ranks(&drawing), [(2, 0), (3, 1), (7, 3), (10, 2)]);
    assert_eq!(feedback(&drawing), [(10, 3)]);
    assert_eq!(graph::check(&drawing), Ok(()));
}

#[test]
fn studio_patch_graph_delayed_feedback_is_not_drawable_until_a_topology_admits_delay() {
    // DESIGN.md 5.4: no topology admits a Delay node yet, so validation refuses the loop and
    // the exporter draws only validated patches.
    let text = "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n\
                node 3 Drive\nparam 3 drive 2\nnode 10 Delay\n\
                edge 3 out Audio 10 in Audio\nedge 10 out Audio 3 in Audio\n";
    let dir = run_dir("delay-loop", text, None, None);
    let error = export_run(&dir).expect_err("a Delay loop is not validated");
    assert_eq!(error.name(), "patchback.studio.PatchNotDrawable", "{error}");
}

#[test]
fn studio_patch_graph_feedback_not_at_delay_is_refused() {
    scenario("studio-patch-graph-feedback-not-at-delay");
    let input = drawn(
        vec![
            node(3, NodeKind::Drive, 0),
            node(4, NodeKind::LowpassFilter, 1),
        ],
        vec![edge(3, "out", 4, "in", PortRate::Audio, true)],
    );
    let error = graph::check(&input).expect_err("feedback leaves a Drive");
    assert_eq!(error.name(), "patchback.studio.FeedbackNotAtDelay");
    assert!(matches!(error, StudioError::FeedbackNotAtDelay { .. }));
}

#[test]
fn studio_patch_graph_a_drawing_never_marks_an_edge_that_leaves_another_kind() {
    // An instantaneous Drive -> Lowpass -> Drive cycle has no Delay: nothing is marked, and the
    // cycle is left to validation, which refuses it.
    let patch = code_patch(
        vec![decl(3, NodeKind::Drive), decl(4, NodeKind::LowpassFilter)],
        vec![audio(3, 4), audio(4, 3)],
    );
    let drawing = graph::drawing(&patch, b"instantaneous");
    assert!(feedback(&drawing).is_empty());
}

#[test]
fn studio_patch_graph_endpoint_missing_is_refused() {
    scenario("studio-patch-graph-endpoint-missing");
    let input = drawn(
        vec![node(6, NodeKind::Vca, 0)],
        vec![edge(6, "out", 9, "in", PortRate::Audio, false)],
    );
    let error = graph::check(&input).expect_err("node 9 is not drawn");
    assert_eq!(error.name(), "patchback.studio.GraphEndpointMissing");

    // through the exporter: a patch file whose edge names node 9, which it does not declare
    let text = demo_patch().replace("edge 6 out Audio 7 in Audio", "edge 6 out Audio 9 in Audio");
    let dir = run_dir("endpoint", &text, None, None);
    let error = export_run(&dir).expect_err("node 9 is not declared");
    assert_eq!(
        error.name(),
        "patchback.studio.GraphEndpointMissing",
        "{error}"
    );
}

#[test]
fn studio_patch_graph_a_duplicate_node_is_refused() {
    let input = drawn(
        vec![node(6, NodeKind::Vca, 0), node(6, NodeKind::Output, 1)],
        Vec::new(),
    );
    let error = graph::check(&input).expect_err("node 6 twice");
    assert_eq!(error.name(), "patchback.studio.DuplicateGraphNode");
}

#[test]
fn studio_patch_graph_an_unvalidated_patch_is_not_drawn() {
    // a parameter outside its contract: the patch loads and fails validation
    let text = demo_patch().replace("param 4 filter_cutoff 5000", "param 4 filter_cutoff 99999");
    let dir = run_dir("unvalidated", &text, None, None);
    let error = export_run(&dir).expect_err("cutoff above 16 kHz");
    assert_eq!(error.name(), "patchback.studio.PatchNotDrawable", "{error}");
    assert!(
        error.to_string().contains("ParameterContractViolation"),
        "{error}"
    );

    // a file the parser refuses has not been validated either
    let dir = run_dir("unparsed", "patchback-patch 9\n", None, None);
    let error = export_run(&dir).expect_err("version 9");
    assert_eq!(error.name(), "patchback.studio.PatchNotDrawable", "{error}");

    // and a run without its patch file
    let dir = scratch("no-patch");
    let error = export_run(&dir).expect_err("no patch.patch");
    assert_eq!(error.name(), "patchback.studio.PatchNotDrawable", "{error}");
}

#[test]
fn studio_patch_graph_an_export_is_byte_reproducible() {
    let dir = run_dir("twice", &demo_patch(), None, None);
    let first = export_run(&dir).expect("first export");
    let second = export_run(&dir).expect("second export");
    assert_eq!(first, second);
    assert!(first.ends_with('\n'));

    // another patch is another identity
    let text = demo_patch().replace("param 4 filter_cutoff 5000", "param 4 filter_cutoff 4000");
    let other = export_run(&run_dir("other", &text, None, None)).expect("other export");
    assert_ne!(
        json(&first)["graph"]["patch_id"],
        json(&other)["graph"]["patch_id"]
    );
}

#[test]
fn studio_patch_graph_the_cli_writes_the_document_and_refuses_an_existing_file() {
    let dir = run_dir("cli", &demo_patch(), None, None);
    let out = dir.join("export.json");
    let binary = env!("CARGO_BIN_EXE_patchback-studio");
    let run = || {
        Command::new(binary)
            .args(["export", "--run"])
            .arg(&dir)
            .arg("--out")
            .arg(&out)
            .output()
            .expect("run patchback-studio")
    };
    let first = run();
    assert!(first.status.success(), "{first:?}");
    assert_eq!(
        fs::read_to_string(&out).expect("the export"),
        export_run(&dir).expect("export")
    );

    let second = run();
    assert_eq!(second.status.code(), Some(1), "{second:?}");
    let stderr = String::from_utf8(second.stderr).expect("UTF-8");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with("patchback-studio: export refused: "),
        "{stderr}"
    );

    let refused = Command::new(binary)
        .args(["export", "--run"])
        .arg(scratch("cli-empty"))
        .arg("--out")
        .arg(dir.join("never.json"))
        .output()
        .expect("run patchback-studio");
    assert_eq!(refused.status.code(), Some(1));
    assert!(!dir.join("never.json").exists());
    let stderr = String::from_utf8(refused.stderr).expect("UTF-8");
    assert!(
        stderr.starts_with("patchback-studio: export refused: patchback.studio.PatchNotDrawable: "),
        "{stderr}"
    );
}
