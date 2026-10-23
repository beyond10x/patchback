//! Adversarial cases for story:patch-review-fixes (wave a4). Each case names the clause it drives
//! the implementation from: the field summaries of `patchback.patch.FixedChainMismatch` in
//! `spec/domains/patch.yaml` and the CHANGELOG line ("naming the missing kinds, extra nodes and
//! missing edges"), or the precedence rule of `patch/file.rs` and `LoadPatch`'s declared order.

use patchback_core::patch::{
    LoadPatchError, NodeKind, PatchFileReason, ValidationError, load_patch, parse_patch,
};

/// The § 5.2 voice as a `.patch` file, every edge of the fixed chain present.
const VOICE: &str = "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n\
node 1 NoteInput\nnode 2 SawOscillator\nnode 3 Drive\nparam 3 drive 6\n\
node 4 LowpassFilter\nparam 4 filter_cutoff 1200\nparam 4 resonance_q 0.7\n\
node 5 AdsrEnvelope\nparam 5 attack 0.01\nparam 5 decay 0.2\nparam 5 sustain 0.6\n\
param 5 release 0.3\nnode 6 Vca\nnode 7 Output\nparam 7 output_gain -6\n\
edge 1 pitch Control 2 pitch Control\nedge 1 gate Event 5 gate Event\n\
edge 2 out Audio 3 in Audio\nedge 3 out Audio 4 in Audio\nedge 4 out Audio 6 in Audio\n\
edge 5 env Control 6 gain Control\nedge 6 out Audio 7 in Audio\n";

/// A file holding the whole wired voice plus one stray, unconnected `Vca` (node 0) is refused,
/// rightly. But the refusal must not tell the operator that the chain edges the file has are
/// missing, nor that the wired VCA (node 6) is the extra node: the file has
/// `edge 4 out Audio 6 in Audio`, `edge 5 env Control 6 gain Control` and
/// `edge 6 out Audio 7 in Audio`. The stray node is the extra one and no chain edge is missing.
#[test]
fn adversary_a_stray_lower_numbered_duplicate_does_not_report_present_chain_edges_as_missing() {
    let text = VOICE.replacen("node 6 Vca\n", "node 0 Vca\nnode 6 Vca\n", 1);
    let error = match load_patch(text.as_bytes()) {
        Err(LoadPatchError::Graph(error)) => error,
        other => panic!("expected a graph refusal, got {other:?}"),
    };
    let ValidationError::FixedChainMismatch {
        missing_kinds,
        extra_nodes,
        missing_edges,
    } = &error
    else {
        panic!("expected FixedChainMismatch, got {error}");
    };
    assert!(missing_kinds.is_empty(), "{error}");
    assert!(
        missing_edges.is_empty(),
        "every chain edge is in the file, yet the refusal names as missing: {error}"
    );
    assert_eq!(
        extra_nodes,
        &vec![(0, NodeKind::Vca)],
        "the unconnected node 0 is the extra one, not the wired node 6: {error}"
    );
}

/// `LoadPatch` declares `unsupported-rate` before `malformed-record`, and `patch/file.rs` says the
/// highest-ranked fault wins. Its one exception for line 1 is argued as "without a version-1
/// header no later line has a meaning to be faulty in". This header does state version 1 (the
/// parser reads the token as 1); only a field too many is wrong. The model, which sees
/// `format_version: 1` and `sample_rate_hz: 44100`, answers `unsupported-rate`.
#[test]
fn adversary_a_version_1_header_with_an_extra_field_does_not_outrank_an_unsupported_rate() {
    let text = "patchback-patch 1 extra\nsample_rate_hz 44100\ntopology_id subtractive-mono/1\n";
    let error = parse_patch(text).expect_err("two faults");
    assert_eq!(
        (error.line, error.reason.clone()),
        (
            Some(2),
            PatchFileReason::UnsupportedSampleRate {
                sample_rate_hz: 44_100
            }
        ),
        "{error}"
    );
}
