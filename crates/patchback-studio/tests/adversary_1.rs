//! Adversary pass 1 on story:studio-patch-graph and story:studio-fit-replay: the exporter's
//! refusals read against `spec/domains/studio.yaml` `AppendFrame`, whose `out-of-order` outcome
//! is "a held frame of this trace has a sequence >= the input's", and whose `TraceFrame`
//! invariant is `sequence >= 1`.

mod common;

use common::{TOPOLOGY, default_run_meta, fitted_patch, run_dir, trace_text};
use patchback_studio::export::export_run;

fn line(sequence: u64, outcome: &str, losses: &str, best: &str) -> String {
    format!(
        "evaluation {sequence} {sequence} 0 {outcome} {TOPOLOGY} {losses} {best} \
         0.5 0.5 0.5 0.5 0.5 0.5 0.5 0.5"
    )
}

fn refused(name: &str, lines: &[String]) -> (String, String) {
    let text = trace_text(lines, "ImprovementBelowThreshold");
    let dir = run_dir(name, &fitted_patch(), Some(default_run_meta()), Some(&text));
    let error = export_run(&dir).expect_err("the trace is refused");
    (error.name().to_owned(), error.to_string())
}

/// Sequences 1, 2, 4: the frame of sequence 4 is above every sequence the trace holds, so the
/// domain's `out-of-order` outcome does not apply to it. `trace.events` may refuse a gap, but
/// naming it `FrameOutOfOrder` ("not above every sequence the trace holds") states something
/// false about the input.
#[test]
fn adversary_1_a_gap_in_the_sequence_is_not_frame_out_of_order() {
    let lines = [
        line(1, "first-retained", "0.8 0.6 0.4 0.5 0.645", "0.645"),
        line(2, "not-improved", "0.9 0.7 0.5 0.6 0.745", "0.645"),
        line(4, "not-improved", "0.9 0.7 0.5 0.6 0.745", "0.645"),
    ];
    let (name, message) = refused("adv1-gap", &lines);
    assert_ne!(name, "patchback.studio.FrameOutOfOrder", "{message}");
}

/// A first frame of sequence 0: no frame is held, so `out-of-order` cannot apply; it breaks the
/// `TraceFrame` invariant `sequence >= 1`. The studio's loader answers the same input as
/// `malformed` (`studio/src/replay.test.ts`, "refuses a frame of sequence 0 as malformed"); the
/// exporter answers `FrameOutOfOrder`, so the two halves name one input differently.
#[test]
fn adversary_1_a_first_frame_of_sequence_zero_is_not_frame_out_of_order() {
    let lines = [line(0, "first-retained", "0.8 0.6 0.4 0.5 0.645", "0.645")];
    let (name, message) = refused("adv1-zero", &lines);
    assert_ne!(name, "patchback.studio.FrameOutOfOrder", "{message}");
}
