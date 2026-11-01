// story:studio-fit-replay: the replay in an exported document, its refusals as the view enforces
// them for a file picked by hand (the same order crates/patchback-studio answers them), and the
// bundled sample, a real fit of demos/pluck.
import { describe, expect, it } from "vitest";
import sampleText from "../fixtures/sample-run.json?raw";
import topologyChange from "../fixtures/trace-topology-change.json";
import { DocumentError, checkReplay, parseDocument, replayOf } from "./document";
import { sample } from "./store";

function refusal(run: () => unknown): DocumentError {
  try {
    run();
  } catch (error) {
    if (error instanceof DocumentError) return error;
    throw error;
  }
  throw new Error("not refused");
}

const sampleTrace = () => structuredClone(JSON.parse(sampleText).trace);
const check = (trace: unknown) => checkReplay(replayOf(trace));

describe("the bundled sample's replay", () => {
  it("is a completed run whose frames count from 1 and whose best loss never rises", () => {
    const { trace } = sample.document;
    expect(trace.frames.length).toBeGreaterThanOrEqual(100);
    expect(trace.frames.map((f) => f.sequence)).toEqual(trace.frames.map((_, i) => i + 1));
    expect(trace.closed.ended).toBe("Completed");
    expect(trace.closed.stop_reason).toBeDefined();
    expect(trace.opened.source).toBe("Replay");
    expect(trace.opened.topology_name).toBe(sample.document.topology);
    expect(trace.opened.topology_id).toBe(sample.document.graph.topology_id);
    const bests = trace.frames.flatMap((f) => (f.best_loss === undefined ? [] : [Number(f.best_loss)]));
    bests.forEach((b, i) => expect(b).toBeLessThanOrEqual(bests[i - 1] ?? Infinity));
    expect(trace.frames.every((f) => f.parameters.length === trace.opened.parameter_paths.length)).toBe(true);
  });
});

describe("patchback.studio.AppendFrame and CloseTrace refusals", () => {
  it("studio-fit-replay-out-of-order: a repeated sequence", () => {
    const trace = sampleTrace();
    trace.frames[1].sequence = 1;
    expect(refusal(() => check(trace)).error).toBe("patchback.studio.FrameOutOfOrder");
    const text = JSON.parse(sampleText);
    text.trace = trace;
    expect(refusal(() => parseDocument(JSON.stringify(text))).error).toBe("patchback.studio.FrameOutOfOrder");
  });

  it("studio-fit-replay-best-worsened: a best loss above an earlier one", () => {
    const trace = sampleTrace();
    const f = trace.frames.find((x: { outcome: string }, i: number) => i > 0 && x.outcome === "NotImproved");
    f.best_loss = "0.9";
    expect(refusal(() => check(trace)).error).toBe("patchback.studio.BestLossWorsened");
  });

  it("studio-fit-replay-inconsistent-frame: a rejected frame with a loss, a retained one off the best", () => {
    const rejected = sampleTrace();
    rejected.frames[3] = { ...rejected.frames[3], outcome: "Rejected" };
    expect(refusal(() => check(rejected)).error).toBe("patchback.studio.InconsistentFrame");

    const improved = sampleTrace();
    improved.frames[0].best_loss = String(Number(improved.frames[0].total_loss) + 0.01);
    expect(refusal(() => check(improved)).error).toBe("patchback.studio.InconsistentFrame");
  });

  it("answers an inconsistent frame before its sequence, as the domain's precedence order does", () => {
    const trace = sampleTrace();
    trace.frames[2] = { ...trace.frames[2], sequence: 1, outcome: "Rejected" };
    expect(refusal(() => check(trace)).error).toBe("patchback.studio.InconsistentFrame");
  });

  it("studio-fit-replay-inconsistent-end: a completed end without a stop reason, a failed one with one", () => {
    const completed = sampleTrace();
    delete completed.closed.stop_reason;
    expect(refusal(() => check(completed)).error).toBe("patchback.studio.InconsistentEnd");
    const failed = sampleTrace();
    failed.closed.ended = "Failed";
    expect(refusal(() => check(failed)).error).toBe("patchback.studio.InconsistentEnd");
  });

  it("refuses a frame of sequence 0 as malformed: sequences count from 1", () => {
    const trace = sampleTrace();
    trace.frames[0].sequence = 0;
    expect(refusal(() => check(trace)).error).toBe("malformed");
  });

  it("refuses a path named twice", () => {
    const trace = sampleTrace();
    trace.opened.parameter_paths[1] = trace.opened.parameter_paths[0];
    expect(refusal(() => check(trace)).error).toBe("patchback.studio.DuplicateParameterPath");
  });
});

describe("the topology-change fixture", () => {
  it("is labelled illustrative and passes the replay's checks", () => {
    expect(topologyChange.note).toMatch(/^Illustrative, not a Patchback run/);
    const trace = check(topologyChange.trace);
    expect(new Set(trace.frames.map((f) => f.topology_name)).size).toBe(2);
  });
});
