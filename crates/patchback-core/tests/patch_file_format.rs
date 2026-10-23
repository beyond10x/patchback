//! The numeric and structural obligations of story:patch-file-format (DESIGN.md § 5.1, § 5.2,
//! § 11, § 12.3). The ESS scenarios `spec/scenarios/patch-file-format-*.yaml` name the command
//! outcomes; these tests hold the grammar, the bounds and the roundtrip they cannot express.

use std::collections::BTreeSet;
use std::io::{self, Read};

use patchback_core::parameter::Topology;
use patchback_core::patch::{
    EdgeDecl, FORMAT_VERSION, LOAD_PATCH_PRECEDENCE, MAX_EDGES, MAX_FILE_BYTES, MAX_LINE_BYTES,
    MAX_NODES, NodeDecl, NodeKind, ParameterValue, Patch, PatchFileError, PatchFileReason,
    PortRate, ReadPatchError, SUPPORTED_SAMPLE_RATE_HZ, format_decimal, parse_decimal, parse_patch,
    read_patch, read_patch_file, write_patch,
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

/// The § 5.2 voice with every parameter at its contract default, wired by the § 5.1 port table,
/// in canonical order.
fn default_voice() -> Patch {
    use NodeKind::*;
    use PortRate::*;
    let topology = Topology::subtractive_mono();
    let default = |path: &str| {
        let contract = topology.contract(path).expect("a § 5.2 parameter");
        value(path, contract.default_physical())
    };
    Patch {
        format_version: FORMAT_VERSION,
        sample_rate_hz: SUPPORTED_SAMPLE_RATE_HZ,
        topology_id: topology.name().to_owned(),
        nodes: vec![
            node(1, NoteInput, vec![]),
            node(2, SawOscillator, vec![]),
            node(3, Drive, vec![default("drive")]),
            node(
                4,
                LowpassFilter,
                vec![default("filter_cutoff"), default("resonance_q")],
            ),
            node(
                5,
                AdsrEnvelope,
                vec![
                    default("attack"),
                    default("decay"),
                    default("release"),
                    default("sustain"),
                ],
            ),
            node(6, Vca, vec![]),
            node(7, Output, vec![default("output_gain")]),
        ],
        edges: vec![
            edge((1, "gate", Event), (5, "gate", Event)),
            edge((1, "pitch", Control), (2, "pitch", Control)),
            edge((2, "out", Audio), (3, "in", Audio)),
            edge((3, "out", Audio), (4, "in", Audio)),
            edge((4, "out", Audio), (6, "in", Audio)),
            edge((5, "env", Control), (6, "gain", Control)),
            edge((6, "out", Audio), (7, "in", Audio)),
        ],
    }
}

/// Asserts two patches are equal with every parameter value bit-exact (`PartialEq` on `f64`
/// would let `-0.0` equal `0.0`).
fn assert_bit_exact(left: &Patch, right: &Patch) {
    assert_eq!(left, right);
    for (l, r) in left.nodes.iter().zip(&right.nodes) {
        for (lp, rp) in l.parameters.iter().zip(&r.parameters) {
            assert_eq!(
                lp.value.to_bits(),
                rp.value.to_bits(),
                "node {} parameter {}: {} and {} differ in bits",
                l.id,
                lp.name,
                lp.value,
                rp.value
            );
        }
    }
}

fn roundtrip(patch: &Patch) -> Patch {
    let text = write_patch(patch).expect("the patch is writable");
    parse_patch(&text).unwrap_or_else(|e| panic!("the writer's output parses: {e}\n{text}"))
}

/// A file of header and keys, then `body`.
fn with_body(body: &str) -> String {
    format!("patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n{body}")
}

fn refusal(text: &str) -> PatchFileError {
    match parse_patch(text) {
        Ok(patch) => panic!("expected a refusal, loaded {patch:?}"),
        Err(error) => error,
    }
}

// ---------------------------------------------------------------------------------------------
// Roundtrip (numeric obligation 1: write then parse is identity, § 11)

#[test]
fn default_voice_write_then_parse_is_identity() {
    let patch = default_voice();
    assert_bit_exact(&roundtrip(&patch), &patch);
}

#[test]
fn contract_range_edges_write_then_parse_is_identity() {
    let topology = Topology::subtractive_mono();
    let mut parameters = Vec::new();
    for contract in topology.contracts() {
        let (low, high) = (contract.physical_at_zero, contract.physical_at_one);
        let candidates = [
            low,
            high,
            next_up(low),
            next_down(low),
            next_up(high),
            next_down(high),
            contract.to_physical(0.0),
            contract.to_physical(1.0),
            contract.to_physical(f64::EPSILON),
            contract.to_physical(1.0 - f64::EPSILON),
            contract.default_physical(),
        ];
        for (i, v) in candidates.into_iter().enumerate() {
            parameters.push(value(&format!("{}_{i}", contract.path), v));
        }
    }
    parameters.sort_by(|a, b| a.name.cmp(&b.name));
    let mut patch = default_voice();
    patch.nodes = vec![node(1, NodeKind::AdsrEnvelope, parameters)];
    patch.edges = vec![];
    assert_bit_exact(&roundtrip(&patch), &patch);
}

#[test]
fn decimal_format_then_parse_is_bit_exact_on_every_class_of_finite_f64() {
    let mut values = vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.1,
        800.0,
        16000.0,
        1e-300,
        1e300,
        f64::MAX,
        f64::MIN,
        f64::MIN_POSITIVE,
        -f64::MIN_POSITIVE,
        f64::from_bits(1),
        -f64::from_bits(1),
        f64::from_bits(0x000f_ffff_ffff_ffff),
        f64::EPSILON,
        1.0 / 3.0,
        std::f64::consts::PI,
    ];
    // A fixed LCG over raw bit patterns: every exponent and sign, finite ones kept.
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    while values.len() < 200_000 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let v = f64::from_bits(state);
        if v.is_finite() {
            values.push(v);
        }
    }
    for v in values {
        let text = format_decimal(v);
        let back = parse_decimal(&text).unwrap_or_else(|| panic!("{text} ({v:?}) is refused"));
        assert_eq!(back.to_bits(), v.to_bits(), "{v:?} wrote {text}");
        assert!(text.len() <= 24, "{v:?} wrote {text}, {} bytes", text.len());
    }
}

#[test]
fn canonical_file_parse_then_write_is_byte_identical() {
    let canonical = write_patch(&default_voice()).expect("writable");
    let again = write_patch(&parse_patch(&canonical).expect("parses")).expect("writable");
    assert_eq!(again, canonical);
    let expected = "patchback-patch 1
sample_rate_hz 48000
topology_id subtractive-mono/1
node 1 NoteInput
node 2 SawOscillator
node 3 Drive
param 3 drive 12
node 4 LowpassFilter
param 4 filter_cutoff 800
param 4 resonance_q 6.25
node 5 AdsrEnvelope
param 5 attack 0.044721359549995794
param 5 decay 0.1
param 5 release 0.1
param 5 sustain 0.5
node 6 Vca
node 7 Output
param 7 output_gain -12
edge 1 gate Event 5 gate Event
edge 1 pitch Control 2 pitch Control
edge 2 out Audio 3 in Audio
edge 3 out Audio 4 in Audio
edge 4 out Audio 6 in Audio
edge 5 env Control 6 gain Control
edge 6 out Audio 7 in Audio
";
    assert_eq!(canonical, expected);
}

#[test]
fn writer_emits_canonical_order_whatever_the_patch_order() {
    let canonical = default_voice();
    let mut shuffled = canonical.clone();
    shuffled.nodes.reverse();
    for n in &mut shuffled.nodes {
        n.parameters.reverse();
    }
    shuffled.edges.reverse();
    assert_eq!(
        write_patch(&shuffled).expect("writable"),
        write_patch(&canonical).expect("writable")
    );
    assert_bit_exact(&shuffled.canonical(), &canonical);
    assert_bit_exact(&roundtrip(&shuffled), &canonical);
}

#[test]
fn parser_keeps_file_order_and_accepts_records_after_the_header_in_any_order() {
    let text = "patchback-patch 1
node 9 Output
param 9 output_gain 0
edge 2 out Audio 9 in Audio
topology_id 7f0c0000-0000-4000-8000-000000000021
node 2 SawOscillator
sample_rate_hz 48000
";
    let patch = parse_patch(text).expect("loads");
    assert_eq!(patch.topology_id, "7f0c0000-0000-4000-8000-000000000021");
    assert_eq!(
        patch.nodes.iter().map(|n| n.id).collect::<Vec<_>>(),
        vec![9, 2]
    );
    assert_eq!(patch.edges.len(), 1);
}

#[test]
fn a_file_without_a_final_newline_and_with_crlf_or_tabs_loads() {
    let text = "patchback-patch 1\r\nsample_rate_hz\t48000\r\ntopology_id  t";
    let patch = parse_patch(text).expect("loads");
    assert_eq!(patch.topology_id, "t");
}

#[test]
fn edges_whose_endpoints_are_not_declared_load_unvalidated() {
    // Graph validation is story:patch-graph-validation's: the parser checks structure only.
    let patch = parse_patch(&with_body("edge 40 nowhere Audio 41 nothing Event\n"))
        .expect("an unvalidated patch loads");
    assert_eq!(patch.edges[0].from_node, 40);
}

// ---------------------------------------------------------------------------------------------
// The number grammar (numeric obligation 2: finite decimals only, § 11, § 2 invariant 4)

#[test]
fn decimal_grammar_accepts_sign_digits_fraction_and_exponent() {
    for (token, expected) in [
        ("0", 0.0),
        ("12", 12.0),
        ("+12", 12.0),
        ("-12", -12.0),
        ("0.5", 0.5),
        ("-0.25", -0.25),
        ("8e2", 800.0),
        ("8E2", 800.0),
        ("1.5e-3", 0.0015),
        ("1.5e+3", 1500.0),
        ("007", 7.0),
        ("1e-400", 0.0),
    ] {
        assert_eq!(parse_decimal(token), Some(expected), "{token}");
    }
    assert_eq!(
        parse_decimal("-0").map(f64::to_bits),
        Some((-0.0f64).to_bits())
    );
}

#[test]
fn decimal_grammar_refuses_nan_and_infinity_words() {
    for token in ["nan", "NaN", "inf", "-inf", "+inf", "infinity", "Infinity"] {
        assert_eq!(parse_decimal(token), None, "{token}");
    }
}

#[test]
fn decimal_grammar_refuses_a_value_that_overflows_to_infinity() {
    for token in [
        "1e400",
        "-1e400",
        "1e309",
        "179769313486231580793728971405303415079934132710037826936173778980444968292764750946649017977587207096330286416692887910946555547851940402630657488671505820681908902000708383676273854845817711531764475730270069855571366959622842914819860834936475292719074168444365510704342711559699508093042880177904174497792",
    ] {
        assert_eq!(parse_decimal(token), None, "{token}");
    }
}

#[test]
fn decimal_grammar_refuses_hex_and_other_radix_forms() {
    for token in ["0x10", "0X1p3", "0b1", "0o7", "1p3"] {
        assert_eq!(parse_decimal(token), None, "{token}");
    }
}

#[test]
fn decimal_grammar_refuses_the_empty_token_and_lone_signs_or_points() {
    for token in ["", "+", "-", ".", "e", "e5", "+-1", "--1"] {
        assert_eq!(parse_decimal(token), None, "{token:?}");
    }
}

#[test]
fn decimal_grammar_refuses_a_point_without_digits_on_both_sides() {
    for token in [".5", "5.", "-.5", "5.e3", "1..2", "1.2.3"] {
        assert_eq!(parse_decimal(token), None, "{token}");
    }
}

#[test]
fn decimal_grammar_refuses_an_exponent_without_digits() {
    for token in ["1e", "1e+", "1e-", "1.5E", "1e2.5", "1e2e3"] {
        assert_eq!(parse_decimal(token), None, "{token}");
    }
}

#[test]
fn decimal_grammar_refuses_separators_spaces_and_trailing_text() {
    for token in ["1_000", "1,5", "1 ", " 1", "1f", "1.0f64", "12dB", "١٢"] {
        assert_eq!(parse_decimal(token), None, "{token:?}");
    }
}

#[test]
fn a_parameter_value_that_is_not_a_finite_decimal_refuses_the_file() {
    for token in ["nan", "inf", "1e400", "0x10", "", "1.", "abc"] {
        let text = with_body(&format!("node 1 Drive\nparam 1 drive {token}\n"));
        let error = refusal(&text);
        if token.is_empty() {
            assert_eq!(error.line, Some(5));
            assert!(matches!(
                error.reason,
                PatchFileReason::WrongFieldCount { .. }
            ));
        } else {
            assert_eq!(
                error,
                PatchFileError {
                    line: Some(5),
                    reason: PatchFileReason::MalformedNumber {
                        token: token.to_owned()
                    }
                },
                "{token}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Each refusal, from a hand-built string (§ 11: refuse, name the line, load nothing)

fn refused_with(text: &str, line: Option<usize>, reason: PatchFileReason) {
    assert_eq!(refusal(text), PatchFileError { line, reason }, "{text}");
}

#[test]
fn refuses_an_empty_file_as_missing_the_format_version() {
    refused_with("", Some(1), PatchFileReason::MissingFormatVersion);
}

#[test]
fn refuses_a_first_line_that_is_not_the_header() {
    refused_with(
        "sample_rate_hz 48000\npatchback-patch 1\n",
        Some(1),
        PatchFileReason::MissingFormatVersion,
    );
    refused_with(
        "patchback-patch\n",
        Some(1),
        PatchFileReason::MissingFormatVersion,
    );
}

#[test]
fn refuses_an_unknown_format_version() {
    refused_with(
        "patchback-patch 2\nsample_rate_hz 48000\ntopology_id t\n",
        Some(1),
        PatchFileReason::UnknownFormatVersion { format_version: 2 },
    );
    refused_with(
        "patchback-patch 0\n",
        Some(1),
        PatchFileReason::UnknownFormatVersion { format_version: 0 },
    );
}

#[test]
fn refuses_a_format_version_that_is_not_an_integer() {
    refused_with(
        "patchback-patch 1.0\n",
        Some(1),
        PatchFileReason::MalformedNumber {
            token: "1.0".to_owned(),
        },
    );
    refused_with(
        "patchback-patch 99999999999\n",
        Some(1),
        PatchFileReason::MalformedNumber {
            token: "99999999999".to_owned(),
        },
    );
}

#[test]
fn refuses_an_unsupported_sample_rate() {
    refused_with(
        "patchback-patch 1\nsample_rate_hz 44100\ntopology_id t\n",
        Some(2),
        PatchFileReason::UnsupportedSampleRate {
            sample_rate_hz: 44100,
        },
    );
}

#[test]
fn refuses_a_missing_patch_key() {
    refused_with(
        "patchback-patch 1\ntopology_id t\n",
        None,
        PatchFileReason::MissingKey {
            key: "sample_rate_hz".to_owned(),
        },
    );
    refused_with(
        "patchback-patch 1\nsample_rate_hz 48000\n",
        None,
        PatchFileReason::MissingKey {
            key: "topology_id".to_owned(),
        },
    );
}

#[test]
fn refuses_a_duplicate_patch_key() {
    refused_with(
        "patchback-patch 1\nsample_rate_hz 48000\ntopology_id t\nsample_rate_hz 48000\n",
        Some(4),
        PatchFileReason::DuplicateKey {
            key: "sample_rate_hz".to_owned(),
        },
    );
    refused_with(
        "patchback-patch 1\ntopology_id t\ntopology_id u\n",
        Some(3),
        PatchFileReason::DuplicateKey {
            key: "topology_id".to_owned(),
        },
    );
    refused_with(
        "patchback-patch 1\npatchback-patch 1\n",
        Some(2),
        PatchFileReason::DuplicateKey {
            key: "patchback-patch".to_owned(),
        },
    );
}

#[test]
fn refuses_an_unknown_record() {
    for (line, record) in [
        ("# a comment", "#"),
        ("volume 3", "volume"),
        ("Node 1 Vca", "Node"),
    ] {
        refused_with(
            &with_body(&format!("{line}\n")),
            Some(4),
            PatchFileReason::UnknownRecord {
                record: record.to_owned(),
            },
        );
    }
}

#[test]
fn refuses_a_blank_line() {
    refused_with(
        &with_body("\nnode 1 Vca\n"),
        Some(4),
        PatchFileReason::BlankLine,
    );
    refused_with(&with_body("   \n"), Some(4), PatchFileReason::BlankLine);
}

#[test]
fn refuses_a_record_with_the_wrong_field_count() {
    refused_with(
        &with_body("node 1\n"),
        Some(4),
        PatchFileReason::WrongFieldCount {
            record: "node".to_owned(),
            expected: 3,
            found: 2,
        },
    );
    refused_with(
        &with_body("node 1 Vca\nedge 1 out Audio 2 in\n"),
        Some(5),
        PatchFileReason::WrongFieldCount {
            record: "edge".to_owned(),
            expected: 7,
            found: 6,
        },
    );
}

#[test]
fn refuses_a_duplicate_node_id() {
    refused_with(
        &with_body("node 4 LowpassFilter\nparam 4 cutoff 800\nnode 4 Vca\n"),
        Some(6),
        PatchFileReason::DuplicateNodeId { node: 4 },
    );
}

#[test]
fn refuses_a_duplicate_parameter_on_a_node() {
    refused_with(
        &with_body("node 4 LowpassFilter\nparam 4 cutoff 800\nparam 4 cutoff 1200\n"),
        Some(6),
        PatchFileReason::DuplicateParameter {
            node: 4,
            name: "cutoff".to_owned(),
        },
    );
}

#[test]
fn the_same_parameter_name_on_two_nodes_is_not_a_duplicate() {
    let patch = parse_patch(&with_body(
        "node 1 Drive\nparam 1 level 1\nnode 2 Drive\nparam 2 level 2\n",
    ))
    .expect("loads");
    assert_eq!(patch.nodes.len(), 2);
}

#[test]
fn refuses_a_parameter_for_a_node_not_declared_above_it() {
    refused_with(
        &with_body("param 3 drive 12\nnode 3 Drive\n"),
        Some(4),
        PatchFileReason::UnknownParameterNode { node: 3 },
    );
}

#[test]
fn refuses_an_unknown_node_kind() {
    for kind in ["FmOscillator", "sawoscillator", "Saw"] {
        refused_with(
            &with_body(&format!("node 1 {kind}\n")),
            Some(4),
            PatchFileReason::UnknownNodeKind {
                kind: kind.to_owned(),
            },
        );
    }
}

#[test]
fn every_node_kind_of_the_spec_is_read_and_written_by_its_name() {
    for kind in NodeKind::ALL {
        let text = with_body(&format!("node 1 {}\n", kind.name()));
        let patch = parse_patch(&text).expect("loads");
        assert_eq!(patch.nodes[0].kind, kind);
        assert_eq!(write_patch(&patch).expect("writable"), text);
    }
}

#[test]
fn refuses_an_unsupported_port_rate_token() {
    for rate in ["audio", "Signal", "Midi"] {
        refused_with(
            &with_body(&format!("edge 1 out {rate} 2 in Audio\n")),
            Some(4),
            PatchFileReason::UnsupportedPortRate {
                rate: rate.to_owned(),
            },
        );
    }
}

#[test]
fn refuses_a_node_id_that_is_not_a_u32() {
    for token in ["-1", "1.0", "4294967296", "x"] {
        refused_with(
            &with_body(&format!("node {token} Vca\n")),
            Some(4),
            PatchFileReason::MalformedNumber {
                token: token.to_owned(),
            },
        );
    }
}

#[test]
fn refuses_a_malformed_name_or_topology_id() {
    for name in ["Cutoff", "1st", "cut-off", "cut.off"] {
        refused_with(
            &with_body(&format!("node 1 Drive\nparam 1 {name} 1\n")),
            Some(5),
            PatchFileReason::MalformedName {
                name: name.to_owned(),
            },
        );
    }
    refused_with(
        &with_body("edge 1 Out Audio 2 in Audio\n"),
        Some(4),
        PatchFileReason::MalformedName {
            name: "Out".to_owned(),
        },
    );
    refused_with(
        "patchback-patch 1\nsample_rate_hz 48000\ntopology_id a\"b\n",
        Some(3),
        PatchFileReason::MalformedName {
            name: "a\"b".to_owned(),
        },
    );
}

#[test]
fn refuses_a_line_that_is_not_utf8() {
    let mut bytes = with_body("node 1 Drive\n").into_bytes();
    bytes.extend_from_slice(b"param 1 dr\xffve 1\n");
    let error = match read_patch(&bytes[..]) {
        Err(ReadPatchError::Format(error)) => error,
        other => panic!("expected a format refusal, got {other:?}"),
    };
    assert_eq!(
        error,
        PatchFileError {
            line: Some(5),
            reason: PatchFileReason::NotUtf8
        }
    );
}

#[test]
fn refuses_a_line_past_the_line_bound() {
    let long = "a".repeat(MAX_LINE_BYTES - "topology_id ".len());
    let at_bound = format!("patchback-patch 1\nsample_rate_hz 48000\ntopology_id {long}\n");
    assert!(
        parse_patch(&at_bound).is_ok(),
        "a line of exactly the bound loads"
    );
    let past = format!("patchback-patch 1\nsample_rate_hz 48000\ntopology_id {long}a\n");
    refused_with(
        &past,
        Some(3),
        PatchFileReason::LineTooLong {
            bytes: MAX_LINE_BYTES + 1,
        },
    );
}

#[test]
fn refuses_a_node_count_past_the_node_bound() {
    let mut body = String::new();
    for id in 0..MAX_NODES {
        body.push_str(&format!("node {id} Vca\n"));
    }
    assert_eq!(
        parse_patch(&with_body(&body))
            .expect("the bound loads")
            .nodes
            .len(),
        MAX_NODES
    );
    body.push_str(&format!("node {MAX_NODES} Vca\n"));
    refused_with(
        &with_body(&body),
        Some(3 + MAX_NODES + 1),
        PatchFileReason::TooManyNodes,
    );
}

#[test]
fn refuses_an_edge_count_past_the_edge_bound() {
    let mut body = String::new();
    for i in 0..MAX_EDGES {
        body.push_str(&format!("edge {i} out Audio {i} in Audio\n"));
    }
    assert_eq!(
        parse_patch(&with_body(&body))
            .expect("the bound loads")
            .edges
            .len(),
        MAX_EDGES
    );
    body.push_str("edge 0 out Audio 1 in Audio\n");
    refused_with(
        &with_body(&body),
        Some(3 + MAX_EDGES + 1),
        PatchFileReason::TooManyEdges,
    );
}

#[test]
fn refuses_text_past_the_file_bound() {
    let text = "a".repeat(MAX_FILE_BYTES + 1);
    refused_with(&text, None, PatchFileReason::FileTooLarge);
}

/// An endless reader that counts what is taken from it.
struct Endless {
    taken: usize,
}

impl Read for Endless {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        buf.fill(b'a');
        self.taken += buf.len();
        Ok(buf.len())
    }
}

#[test]
fn an_endless_stream_is_refused_after_reading_at_most_the_bound_plus_one() {
    let mut endless = Endless { taken: 0 };
    let result = read_patch(&mut endless);
    assert!(
        matches!(
            result,
            Err(ReadPatchError::Format(PatchFileError {
                line: None,
                reason: PatchFileReason::FileTooLarge
            }))
        ),
        "{result:?}"
    );
    assert!(
        endless.taken <= MAX_FILE_BYTES + 1 + 64 * 1024,
        "read {} bytes",
        endless.taken
    );
}

#[test]
fn a_file_past_the_size_bound_is_refused_without_reading_it_whole() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-core-patch-file");
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let path = dir.join("huge.patch");
    // A sparse file of 4 GiB: reading it whole would take seconds and the memory to hold it.
    let file = std::fs::File::create(&path).expect("create");
    file.set_len(4 << 30).expect("sparse length");
    drop(file);
    let result = read_patch_file(&path);
    std::fs::remove_file(&path).expect("remove");
    assert!(
        matches!(
            result,
            Err(ReadPatchError::Format(PatchFileError {
                line: None,
                reason: PatchFileReason::FileTooLarge
            }))
        ),
        "{result:?}"
    );
}

#[test]
fn a_patch_file_on_disk_loads() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-core-patch-file");
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let path = dir.join("voice.patch");
    std::fs::write(&path, write_patch(&default_voice()).expect("writable")).expect("write");
    let loaded = read_patch_file(&path).expect("loads");
    std::fs::remove_file(&path).expect("remove");
    assert_bit_exact(&loaded, &default_voice());
    assert!(matches!(
        read_patch_file(dir.join("absent.patch")),
        Err(ReadPatchError::Io(_))
    ));
}

#[test]
fn the_writer_refuses_a_patch_it_could_not_read_back() {
    let mut patch = default_voice();
    patch.nodes[2].parameters[0].value = f64::NAN;
    assert!(matches!(
        write_patch(&patch),
        Err(PatchFileError {
            reason: PatchFileReason::MalformedNumber { .. },
            ..
        })
    ));
    let mut patch = default_voice();
    patch.sample_rate_hz = 44_100;
    assert!(write_patch(&patch).is_err());
    let mut patch = default_voice();
    patch.format_version = 2;
    assert!(write_patch(&patch).is_err());
    let mut patch = default_voice();
    patch.topology_id = "has space".to_owned();
    assert!(write_patch(&patch).is_err());
    let mut patch = default_voice();
    patch.nodes.push(node(1, NodeKind::Vca, vec![]));
    assert!(matches!(
        write_patch(&patch),
        Err(PatchFileError {
            reason: PatchFileReason::DuplicateNodeId { node: 1 },
            ..
        })
    ));
}

// ---------------------------------------------------------------------------------------------
// The class: every reason the parser can give is reached by a test above and names an error of
// the ESS domain, so no refusal is unreachable to a reader of the contract.

/// Exhaustive on purpose: a new reason does not compile until it is listed here.
fn ordinal(reason: &PatchFileReason) -> usize {
    match reason {
        PatchFileReason::MissingFormatVersion => 0,
        PatchFileReason::UnknownFormatVersion { .. } => 1,
        PatchFileReason::UnknownRecord { .. } => 2,
        PatchFileReason::BlankLine => 3,
        PatchFileReason::WrongFieldCount { .. } => 4,
        PatchFileReason::MissingKey { .. } => 5,
        PatchFileReason::DuplicateKey { .. } => 6,
        PatchFileReason::DuplicateNodeId { .. } => 7,
        PatchFileReason::DuplicateParameter { .. } => 8,
        PatchFileReason::UnknownParameterNode { .. } => 9,
        PatchFileReason::MalformedNumber { .. } => 10,
        PatchFileReason::MalformedName { .. } => 11,
        PatchFileReason::UnsupportedSampleRate { .. } => 12,
        PatchFileReason::UnsupportedPortRate { .. } => 13,
        PatchFileReason::UnknownNodeKind { .. } => 14,
        PatchFileReason::NotUtf8 => 15,
        PatchFileReason::LineTooLong { .. } => 16,
        PatchFileReason::FileTooLarge => 17,
        PatchFileReason::TooManyNodes => 18,
        PatchFileReason::TooManyEdges => 19,
    }
}

const REASONS: usize = 20;

#[test]
fn every_refusal_reason_names_an_error_of_the_spec_domain() {
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
    let mut bytes = with_body("").into_bytes();
    bytes.extend_from_slice(b"\xff\n");
    let not_utf8 = match read_patch(&bytes[..]) {
        Err(ReadPatchError::Format(error)) => error.reason,
        other => panic!("{other:?}"),
    };
    let samples = [
        refusal(""),
        refusal("patchback-patch 2\n"),
        refusal(&with_body("volume 1\n")),
        refusal(&with_body("\n")),
        refusal(&with_body("node 1\n")),
        refusal("patchback-patch 1\n"),
        refusal("patchback-patch 1\npatchback-patch 1\n"),
        refusal(&with_body("node 1 Vca\nnode 1 Vca\n")),
        refusal(&with_body("node 1 Vca\nparam 1 a 1\nparam 1 a 1\n")),
        refusal(&with_body("param 1 a 1\n")),
        refusal(&with_body("node x Vca\n")),
        refusal(&with_body("node 1 Vca\nparam 1 A 1\n")),
        refusal("patchback-patch 1\nsample_rate_hz 1\n"),
        refusal(&with_body("edge 1 out audio 2 in Audio\n")),
        refusal(&with_body("node 1 Fm\n")),
        PatchFileError {
            line: Some(4),
            reason: not_utf8,
        },
        refusal(&"a".repeat(MAX_LINE_BYTES + 1)),
        refusal(&"a\n".repeat(MAX_FILE_BYTES)),
        refusal(&with_body(
            &(0..=MAX_NODES)
                .map(|i| format!("node {i} Vca\n"))
                .collect::<String>(),
        )),
        refusal(&with_body(
            &"edge 1 o Audio 2 i Audio\n".repeat(MAX_EDGES + 1),
        )),
    ];
    let mut reached = BTreeSet::new();
    for error in &samples {
        reached.insert(ordinal(&error.reason));
        let name = error.reason.spec_error();
        assert!(
            declared.contains(name),
            "{:?} names {name}, which spec/domains/patch.yaml does not declare",
            error.reason
        );
        assert!(
            error.to_string().contains(&error.reason.to_string()),
            "{error}"
        );
    }
    assert_eq!(reached, (0..REASONS).collect::<BTreeSet<_>>());
}

#[test]
fn a_refusal_names_its_line() {
    let error = refusal(&with_body("node 1 Vca\nnode 1 Vca\n"));
    assert_eq!(error.to_string(), format!("line 5: {}", error.reason));
    let error = refusal("patchback-patch 1\n");
    assert!(!error.to_string().starts_with("line"), "{error}");
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

// ---------------------------------------------------------------------------------------------
// Precedence: a file with several faults is answered with the one whose error `LoadPatch`
// declares first (ESS: declaration order is the declared precedence); among faults of one rank,
// the earliest line.

#[test]
fn load_patch_precedence_is_the_declaration_order_of_the_spec() {
    let domain = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../spec/domains/patch.yaml"
    ))
    .expect("spec/domains/patch.yaml");
    let mut declared: Vec<&str> = Vec::new();
    for error in domain
        .lines()
        .skip_while(|l| l.trim() != "- name: patchback.patch.LoadPatch")
        .skip(1)
        .take_while(|l| !l.starts_with("  - name: "))
        .filter_map(|l| l.trim().strip_prefix("error: "))
    {
        if LOAD_PATCH_PRECEDENCE.contains(&error) && !declared.contains(&error) {
            declared.push(error);
        }
    }
    assert_eq!(declared, LOAD_PATCH_PRECEDENCE);
    for reason in [
        PatchFileReason::UnknownFormatVersion { format_version: 2 },
        PatchFileReason::DuplicateParameter {
            node: 1,
            name: "a".to_owned(),
        },
        PatchFileReason::NotUtf8,
        PatchFileReason::TooManyEdges,
    ] {
        assert_eq!(
            LOAD_PATCH_PRECEDENCE[reason.precedence()],
            reason.spec_error()
        );
    }
}

#[test]
fn a_fault_of_the_header_line_stops_the_scan() {
    refused_with(
        "sample_rate_hz 44100\nnode 1 Vca\nnode 1 Vca\n",
        Some(1),
        PatchFileReason::MissingFormatVersion,
    );
    refused_with(
        "patchback-patch x\nsample_rate_hz 44100\n",
        Some(1),
        PatchFileReason::MalformedNumber {
            token: "x".to_owned(),
        },
    );
}

/// The header line's shapes (story:patch-review-fixes): a header with too few or too many fields
/// and a version this parser does not read is answered by the highest-ranked of its faults, as
/// `LoadPatch` declares `unknown-version` before `malformed-record` and `malformed-number`.
#[test]
fn the_header_line_answers_its_highest_ranked_fault() {
    let wrong_count = |found| PatchFileReason::WrongFieldCount {
        record: "patchback-patch".to_owned(),
        expected: 2,
        found,
    };
    let cases = [
        // A missing field: no version to rank.
        ("patchback-patch\n", PatchFileReason::MissingFormatVersion),
        // An extra field with the known version: the field count alone.
        ("patchback-patch 1 extra\n", wrong_count(3)),
        ("patchback-patch 1 extra more\n", wrong_count(4)),
        // An extra field and an unknown version: the version outranks the field count.
        (
            "patchback-patch 2 extra\n",
            PatchFileReason::UnknownFormatVersion { format_version: 2 },
        ),
        // Two extra fields and an unknown version.
        (
            "patchback-patch 2 extra more\n",
            PatchFileReason::UnknownFormatVersion { format_version: 2 },
        ),
        (
            "patchback-patch 0 1\n",
            PatchFileReason::UnknownFormatVersion { format_version: 0 },
        ),
        // An extra field and a version that is not an integer: the field count outranks the
        // malformed number.
        ("patchback-patch x extra\n", wrong_count(3)),
        ("patchback-patch 99999999999 extra\n", wrong_count(3)),
    ];
    for (header, reason) in cases {
        let text = format!("{header}sample_rate_hz 48000\ntopology_id subtractive-mono/1\n");
        refused_with(&text, Some(1), reason);
    }
    // With an unsupported rate on line 2: a header that reads as version 1 is ranked with the
    // later lines, so the rate outranks its field count; any other header fault is answered alone.
    let rate =
        |header: &str| format!("{header}sample_rate_hz 44100\ntopology_id subtractive-mono/1\n");
    let unsupported = PatchFileReason::UnsupportedSampleRate {
        sample_rate_hz: 44_100,
    };
    refused_with(
        &rate("patchback-patch 1 extra\n"),
        Some(2),
        unsupported.clone(),
    );
    refused_with(
        &rate("patchback-patch 1 extra more\n"),
        Some(2),
        unsupported,
    );
    refused_with(
        &rate("patchback-patch 2 extra\n"),
        Some(1),
        PatchFileReason::UnknownFormatVersion { format_version: 2 },
    );
    refused_with(&rate("patchback-patch x extra\n"), Some(1), wrong_count(3));
    refused_with(
        &rate("patchback-patch\n"),
        Some(1),
        PatchFileReason::MissingFormatVersion,
    );
    // A version-1 header with an extra field and nothing else wrong: the field count, line 1.
    refused_with(
        &rate("patchback-patch 1 extra\n").replace("44100", "48000"),
        Some(1),
        wrong_count(3),
    );
}

#[test]
fn past_the_file_bound_no_other_fault_is_ranked() {
    let mut text = String::from("patchback-patch 2\n");
    text.push_str(&"a\n".repeat(MAX_FILE_BYTES / 2));
    refused_with(&text, None, PatchFileReason::FileTooLarge);
}

#[test]
fn faults_of_one_rank_are_answered_by_the_earliest_line() {
    refused_with(
        &with_body("node 1 Fm\nvolume 3\n"),
        Some(4),
        PatchFileReason::UnknownNodeKind {
            kind: "Fm".to_owned(),
        },
    );
}

#[test]
fn a_line_with_two_faults_is_answered_by_the_higher_ranked() {
    refused_with(
        &with_body("node x Fm\n"),
        Some(4),
        PatchFileReason::UnknownNodeKind {
            kind: "Fm".to_owned(),
        },
    );
    refused_with(
        &with_body("node 1 Vca\nnode 1 Fm\n"),
        Some(5),
        PatchFileReason::DuplicateNodeId { node: 1 },
    );
    refused_with(
        &with_body("sample_rate_hz 44100\n"),
        Some(4),
        PatchFileReason::UnsupportedSampleRate {
            sample_rate_hz: 44100,
        },
    );
}

#[test]
fn a_refused_key_or_node_line_is_not_also_missing_or_undeclared() {
    refused_with(
        "patchback-patch 1\nsample_rate_hz 4.8e4\ntopology_id t\n",
        Some(2),
        PatchFileReason::MalformedNumber {
            token: "4.8e4".to_owned(),
        },
    );
    refused_with(
        &with_body("node x Vca\nparam 1 a 1\n"),
        Some(4),
        PatchFileReason::MalformedNumber {
            token: "x".to_owned(),
        },
    );
}

#[test]
fn a_line_past_the_line_bound_ranks_last_and_the_scan_goes_on() {
    let long = format!("node 1 {}\n", "A".repeat(MAX_LINE_BYTES));
    refused_with(
        &with_body(&format!("{long}node 2 Vca\nnode 2 Vca\n")),
        Some(6),
        PatchFileReason::DuplicateNodeId { node: 2 },
    );
    refused_with(
        &with_body(&format!("{long}node 2 Vca\n")),
        Some(4),
        PatchFileReason::LineTooLong {
            bytes: MAX_LINE_BYTES + 7,
        },
    );
}

#[test]
fn past_the_node_bound_the_scan_goes_on_without_storing() {
    let mut body = String::new();
    for id in 0..=MAX_NODES {
        body.push_str(&format!("node {id} Vca\n"));
    }
    body.push_str(&format!("param {MAX_NODES} gain 1\nnode 0 Vca\n"));
    refused_with(
        &with_body(&body),
        Some(3 + MAX_NODES + 3),
        PatchFileReason::DuplicateNodeId { node: 0 },
    );
}

#[test]
fn the_writer_checks_each_field_before_formatting_it() {
    let mut patch = default_voice();
    patch.topology_id = "t u".to_owned();
    assert_eq!(
        write_patch(&patch),
        Err(PatchFileError {
            line: Some(3),
            reason: PatchFileReason::MalformedName {
                name: "t u".to_owned()
            }
        })
    );
    let mut patch = default_voice();
    patch.edges[0].to_port = "in\nnode 9 Vca".to_owned();
    assert!(matches!(
        write_patch(&patch),
        Err(PatchFileError {
            reason: PatchFileReason::MalformedName { .. },
            ..
        })
    ));
}
