//! Adversarial cases for story:patch-file-format (wave a3). Each case names the clause it drives
//! the implementation from: the module documentation of `patch/file.rs`, DESIGN.md § 11, or the
//! ESS domain `spec/domains/patch.yaml`.

use patchback_core::patch::{
    EdgeDecl, FORMAT_VERSION, MAX_FILE_BYTES, MAX_LINE_BYTES, NodeDecl, NodeKind, ParameterValue,
    Patch, PatchFileError, PatchFileReason, PortRate, SUPPORTED_SAMPLE_RATE_HZ, parse_patch,
    read_patch, write_patch,
};

fn small_patch() -> Patch {
    Patch {
        format_version: FORMAT_VERSION,
        sample_rate_hz: SUPPORTED_SAMPLE_RATE_HZ,
        topology_id: "subtractive-mono/1".to_owned(),
        nodes: vec![
            NodeDecl {
                id: 2,
                kind: NodeKind::SawOscillator,
                parameters: vec![],
            },
            NodeDecl {
                id: 3,
                kind: NodeKind::Drive,
                parameters: vec![ParameterValue {
                    name: "drive".to_owned(),
                    value: 12.0,
                }],
            },
        ],
        edges: vec![EdgeDecl {
            from_node: 2,
            from_port: "out".to_owned(),
            from_rate: PortRate::Audio,
            to_node: 3,
            to_port: "in".to_owned(),
            to_rate: PortRate::Audio,
        }],
    }
}

/// `write_patch` documents: "the writer never emits a file the parser refuses; the refusal is
/// returned instead", and the module: "Write then parse gives the canonical patch bit for bit".
/// So for any patch, either the writer refuses it, or its output parses back to the canonical
/// patch.
fn assert_writer_refuses_or_round_trips(patch: &Patch) {
    match write_patch(patch) {
        Err(_) => {}
        Ok(text) => match parse_patch(&text) {
            Ok(back) => assert_eq!(
                back,
                patch.canonical(),
                "the writer accepted the patch and its output reads back as another patch:\n{text:?}"
            ),
            Err(error) => panic!(
                "the writer accepted the patch and emitted a file the parser refuses ({error}):\n{text:?}"
            ),
        },
    }
}

// ---------------------------------------------------------------------------------------------
// The writer validates the re-split tokens of the line it formatted, not the field values it
// was given, so whitespace inside a String field passes its check.

#[test]
fn writer_refuses_or_round_trips_a_topology_id_ending_in_a_newline() {
    let mut patch = small_patch();
    patch.topology_id = "subtractive-mono/1\n".to_owned();
    assert_writer_refuses_or_round_trips(&patch);
}

#[test]
fn writer_refuses_or_round_trips_a_parameter_name_ending_in_a_tab() {
    let mut patch = small_patch();
    patch.nodes[1].parameters[0].name = "drive\t".to_owned();
    assert_writer_refuses_or_round_trips(&patch);
}

#[test]
fn writer_refuses_or_round_trips_a_port_name_ending_in_a_newline() {
    let mut patch = small_patch();
    patch.edges[0].from_port = "out\n".to_owned();
    assert_writer_refuses_or_round_trips(&patch);
}

// ---------------------------------------------------------------------------------------------
// The line bound with CRLF. Module documentation: "a `\r` before the `\n` is ignored", and
// `MAX_LINE_BYTES`: "The longest line, in bytes without its `\n`". A line at the bound loads
// with LF; the same file with CRLF must load too if the `\r` is ignored.

#[test]
fn a_crlf_line_of_exactly_the_line_bound_loads() {
    let long = "a".repeat(MAX_LINE_BYTES - "topology_id ".len());
    let lf = format!("patchback-patch 1\nsample_rate_hz 48000\ntopology_id {long}\n");
    assert!(parse_patch(&lf).is_ok(), "the LF file at the bound loads");
    let crlf = lf.replace('\n', "\r\n");
    let result = parse_patch(&crlf);
    assert!(
        result.is_ok(),
        "the same file with CRLF is refused: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------------------------
// Precedence. ESS answers with the first declared refusal whose guard holds
// (ess guides/specify/guards-and-predicates.md, "Declaration order is the declared precedence").
// `LoadPatch` declares unknown-version, unsupported-rate, duplicate-id, duplicate-key in that
// order, and `PatchFileReason::spec_error` maps each parser refusal onto one of them. A file
// with two of these faults is answered by the spec with the first declared, by the parser with
// whichever line comes first.

#[test]
fn a_duplicate_node_id_wins_over_an_earlier_duplicate_parameter_as_the_spec_declares() {
    let text = "patchback-patch 1\nsample_rate_hz 48000\ntopology_id t\n\
                node 4 LowpassFilter\nparam 4 cutoff 800\nparam 4 cutoff 1200\nnode 4 Vca\n";
    let error = parse_patch(text).expect_err("refused");
    assert_eq!(
        error.reason.spec_error(),
        "patchback.patch.DuplicateNodeId",
        "{error}"
    );
}

#[test]
fn an_unsupported_sample_rate_wins_over_an_earlier_duplicate_parameter_as_the_spec_declares() {
    let text = "patchback-patch 1\ntopology_id t\n\
                node 4 LowpassFilter\nparam 4 cutoff 800\nparam 4 cutoff 1200\nsample_rate_hz 44100\n";
    let error = parse_patch(text).expect_err("refused");
    assert_eq!(
        error.reason.spec_error(),
        "patchback.patch.UnsupportedSampleRate",
        "{error}"
    );
}

// ---------------------------------------------------------------------------------------------
// Mutant catchers: green on the tree as handed over, red on a plausible mutant the unit's own
// suite does not detect (report: mutants run on a scratch copy).

/// `parse_bytes` refuses `bytes.len() > MAX_FILE_BYTES`; no test of the unit loads a file of
/// exactly the bound, so `>=` survives.
#[test]
fn a_file_of_exactly_the_file_bound_loads() {
    let mut text =
        String::from("patchback-patch 1\nsample_rate_hz 48000\ntopology_id t\nnode 1 Drive\n");
    let mut count = 0usize;
    while MAX_FILE_BYTES - text.len() > 200 {
        text.push_str(&format!("param 1 p{count} 0\n"));
        count += 1;
    }
    let prefix = "param 1 last ";
    let zeros = MAX_FILE_BYTES - text.len() - prefix.len() - 1;
    text.push_str(prefix);
    text.push_str(&"0".repeat(zeros));
    text.push('\n');
    assert_eq!(text.len(), MAX_FILE_BYTES);
    let patch = parse_patch(&text).expect("a file of exactly the bound loads");
    assert_eq!(patch.nodes[0].parameters.len(), count + 1);
    let patch = read_patch(text.as_bytes()).expect("and loads through the reader");
    assert_eq!(patch.nodes[0].parameters.len(), count + 1);
}

/// `Writer::finish` refuses a canonical form past `MAX_FILE_BYTES`; no test of the unit writes
/// such a patch, so dropping the check survives, and the writer then emits a file the parser
/// refuses.
#[test]
fn the_writer_refuses_a_patch_whose_canonical_form_is_past_the_file_bound() {
    let parameters: Vec<ParameterValue> = (0..80_000)
        .map(|i| ParameterValue {
            name: format!("p{i}"),
            value: 0.0,
        })
        .collect();
    let mut patch = small_patch();
    patch.nodes[1].parameters = parameters;
    assert_eq!(
        write_patch(&patch).map(|text| text.len()),
        Err(PatchFileError {
            line: None,
            reason: PatchFileReason::FileTooLarge
        })
    );
}

/// `Patch::canonical` orders edges by all six fields. The unit's order test reverses the § 5.2
/// voice, whose edges all differ in their from end, so a key that drops the to end or the from
/// rate survives, and fan-out from one port is then written in input order.
#[test]
fn edges_that_differ_only_in_the_to_end_or_a_rate_are_written_in_one_order() {
    let edge = |from_rate, to_node, to_port: &str, to_rate| EdgeDecl {
        from_node: 1,
        from_port: "out".to_owned(),
        from_rate,
        to_node,
        to_port: to_port.to_owned(),
        to_rate,
    };
    use PortRate::*;
    let edges = vec![
        edge(Audio, 2, "in", Audio),
        edge(Audio, 3, "in", Audio),
        edge(Audio, 3, "gain", Audio),
        edge(Audio, 3, "gain", Control),
        edge(Control, 2, "in", Audio),
    ];
    let mut patch = small_patch();
    patch.edges = edges.clone();
    let expected = write_patch(&patch).expect("writable");
    for rotation in 1..edges.len() {
        for reversed in [false, true] {
            let mut order = edges.clone();
            order.rotate_left(rotation);
            if reversed {
                order.reverse();
            }
            patch.edges = order;
            assert_eq!(
                write_patch(&patch).expect("writable"),
                expected,
                "rotation {rotation}, reversed {reversed}"
            );
        }
    }
}
