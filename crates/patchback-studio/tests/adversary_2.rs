//! Adversary pass 2 on story:studio-patch-graph and story:studio-fit-replay (correction 1, 9b150d1):
//! the exporter's refusals read against `spec/domains/studio.yaml` `AppendFrame` (its outcomes in
//! document order: `rejected-with-loss`, `valid-without-loss`, `retained-not-best`, then
//! `out-of-order`, then `best-worsened`), the `TraceFrame` invariants, DESIGN.md § 16 ("a trace
//! that breaks an invariant (a sequence below 1) is `invariant` in both") and the order the
//! module documentation of `crates/patchback-studio/src/export.rs` promises.

mod common;

use common::{TOPOLOGY, default_run_meta, fitted_patch, run_dir, trace_text};
use patchback_studio::export::export_run;

fn line(sequence: &str, restart: &str, outcome: &str, losses: &str, best: &str) -> String {
    format!(
        "evaluation {sequence} {sequence} {restart} {outcome} {TOPOLOGY} {losses} {best} \
         0.5 0.5 0.5 0.5 0.5 0.5 0.5 0.5"
    )
}

fn refused_text(name: &str, text: &str) -> (String, String) {
    let dir = run_dir(name, &fitted_patch(), Some(default_run_meta()), Some(text));
    let error = export_run(&dir).expect_err("the run is refused");
    (error.name().to_owned(), error.to_string())
}

fn refused(name: &str, lines: &[String]) -> (String, String) {
    refused_text(name, &trace_text(lines, "ImprovementBelowThreshold"))
}

/// DESIGN.md § 16: "a trace that breaks an invariant (a sequence below 1) is `invariant` in
/// both". The loader answers a frame of sequence -1 `invariant` (`studio/src/document.ts`
/// `bounded`); the exporter must too.
#[test]
fn adversary_2_a_negative_sequence_is_invariant() {
    let lines = [line(
        "-1",
        "0",
        "first-retained",
        "0.8 0.6 0.4 0.5 0.645",
        "0.645",
    )];
    let (name, message) = refused("adv2-negative-sequence", &lines);
    assert_eq!(name, "invariant", "{message}");
}

/// Correction 1's commit: "a frame that breaks a domain invariant (sequence >= 1, evaluation >= 1,
/// restart >= 0 ...) is one error, invariant, in the exporter and the loader". The loader answers
/// restart -1 `invariant` (`studio/src/replay.test.ts`); the exporter must too.
#[test]
fn adversary_2_a_negative_restart_is_invariant() {
    let lines = [line(
        "1",
        "-1",
        "first-retained",
        "0.8 0.6 0.4 0.5 0.645",
        "0.645",
    )];
    let (name, message) = refused("adv2-negative-restart", &lines);
    assert_eq!(name, "invariant", "{message}");
}

/// A rejected candidate that carries a loss and repeats a held sequence: `AppendFrame` tries
/// `rejected-with-loss` before `out-of-order`, so the domain (and the studio's loader,
/// `checkReplay`) answer `InconsistentFrame`.
#[test]
fn adversary_2_a_rejected_frame_with_a_loss_is_inconsistent_before_out_of_order() {
    let lines = [
        line(
            "1",
            "0",
            "first-retained",
            "0.8 0.6 0.4 0.5 0.645",
            "0.645",
        ),
        line("2", "0", "not-improved", "0.9 0.7 0.5 0.6 0.745", "0.645"),
        line("2", "0", "rejected", "0.7", "0.645"),
    ];
    let (name, message) = refused("adv2-rejected-repeat", &lines);
    assert_eq!(name, "patchback.studio.InconsistentFrame", "{message}");
}

/// A retained candidate whose loss (0.5) is not its best loss (0.9), the best rising above the
/// held 0.645: `AppendFrame` tries `retained-not-best` before `best-worsened`, so the domain (and
/// `checkReplay`) answer `InconsistentFrame`.
#[test]
fn adversary_2_a_retained_frame_whose_loss_is_not_its_best_is_inconsistent_before_best_worsened()
{
    let lines = [
        line(
            "1",
            "0",
            "first-retained",
            "0.8 0.6 0.4 0.5 0.645",
            "0.645",
        ),
        line("2", "0", "improved", "0.5 0.4 0.3 0.3 0.5", "0.9"),
    ];
    let (name, message) = refused("adv2-retained-rising", &lines);
    assert_eq!(name, "patchback.studio.InconsistentFrame", "{message}");
}

/// `trace.events` fields are separated by ASCII whitespace (`parse_trace` splits them so), so the
/// same trace with a tab after `evaluation` is the same trace. A repeated sequence is
/// `FrameOutOfOrder` with spaces; `held_sequence` only counts lines that start with
/// `"evaluation "`, so with tabs it holds nothing and answers `trace`.
#[test]
fn adversary_2_a_repeated_sequence_is_out_of_order_whatever_the_field_separator() {
    let spaced = [
        line(
            "1",
            "0",
            "first-retained",
            "0.8 0.6 0.4 0.5 0.645",
            "0.645",
        ),
        line("2", "0", "not-improved", "0.9 0.7 0.5 0.6 0.745", "0.645"),
        line("2", "0", "not-improved", "0.9 0.7 0.5 0.6 0.745", "0.645"),
    ];
    let (name, message) = refused("adv2-spaced", &spaced);
    assert_eq!(name, "patchback.studio.FrameOutOfOrder", "{message}");

    let tabbed: Vec<String> = spaced
        .iter()
        .map(|l| l.replacen("evaluation ", "evaluation\t", 1))
        .collect();
    let (name, message) = refused("adv2-tabbed", &tabbed);
    assert_eq!(name, "patchback.studio.FrameOutOfOrder", "{message}");
}

/// `export.rs` module documentation: "answered in this order", refusal 4 `run` ("the trace is
/// of another topology or parameter list than the patch's") before refusal 7 `trace` ("an
/// unclosed trace"). A trace of another topology that is also unclosed is `run`.
#[test]
fn adversary_2_a_trace_of_another_topology_is_run_before_its_grammar() {
    let text = "patchback-fit-trace 1\ntopology other/1 a b\n";
    let (name, message) = refused_text("adv2-other-unclosed", text);
    assert_eq!(name, "run", "{message}");
}
