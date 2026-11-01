// story:studio-patch-graph: loading an exported document and the refusals of
// patchback.studio.DrawGraph, as the view enforces them for a file picked by hand.
import { describe, expect, it } from "vitest";
import sampleText from "../fixtures/sample-run.json?raw";
import feedbackJson from "../fixtures/graph-delayed-feedback.json";
import { DocumentError, checkGraph, graphDrawn, parseDocument } from "./document";
import { SAMPLE_LABEL, load, loadError, loaded, sample } from "./store";

function refusal(run: () => unknown): DocumentError {
  try {
    run();
  } catch (error) {
    if (error instanceof DocumentError) return error;
    throw error;
  }
  throw new Error("not refused");
}

const sampleGraph = () => structuredClone(JSON.parse(sampleText).graph);

describe("the bundled sample", () => {
  it("is a real export of the subtractive voice, loaded through the parser", () => {
    expect(loaded.value).toBe(sample);
    expect(sample.label).toBe(SAMPLE_LABEL);
    const { document } = sample;
    expect(document.topology).toBe("subtractive-mono/1");
    // studio-patch-graph-subtractive-voice: seven ranked nodes, seven edges, none feedback
    expect(document.graph.nodes.map((n) => [n.kind, n.rank])).toEqual([
      ["NoteInput", 0],
      ["SawOscillator", 1],
      ["Drive", 2],
      ["LowpassFilter", 3],
      ["AdsrEnvelope", 1],
      ["Vca", 4],
      ["Output", 5],
    ]);
    expect(document.graph.edges).toHaveLength(7);
    expect(document.graph.edges.some((e) => e.delayed_feedback)).toBe(false);
    expect(document.modules.map((m) => m.kind)).toEqual(document.graph.nodes.map((n) => n.kind));
  });
});

describe("loading a picked file", () => {
  it("replaces the shown document when it loads", () => {
    expect(load("again.json", sampleText)).toBe(true);
    expect(loaded.value.label).toBe("again.json");
    expect(loadError.value).toBeNull();
  });

  it("keeps the shown document and names the refusal when it does not", () => {
    const before = loaded.value;
    expect(load("notes.txt", "patchback-notes 1\n")).toBe(false);
    expect(loaded.value).toBe(before);
    expect(loadError.value).toMatch(/^notes\.txt: malformed: not JSON/);

    expect(load("other.json", JSON.stringify({ format: "something-else/1" }))).toBe(false);
    expect(loadError.value).toMatch(/malformed: format is not "patchback-studio-export\/1"/);

    const text = JSON.parse(sampleText);
    text.graph.nodes[0].rank = -1;
    expect(load("negative.json", JSON.stringify(text))).toBe(false);
    expect(loadError.value).toMatch(/graph\.nodes\[0\]\.rank is not an integer of at least 0/);
  });
});

describe("patchback.studio.DrawGraph refusals", () => {
  it("studio-patch-graph-endpoint-missing: an edge into a node the drawing does not hold", () => {
    const graph = sampleGraph();
    graph.edges[6].to_node = 9;
    const error = refusal(() => checkGraph(graphDrawn(graph)));
    expect(error.error).toBe("patchback.studio.GraphEndpointMissing");
    const text = JSON.parse(sampleText);
    text.graph = graph;
    expect(refusal(() => parseDocument(JSON.stringify(text))).error).toBe("patchback.studio.GraphEndpointMissing");
  });

  it("studio-patch-graph-feedback-not-at-delay: feedback that leaves a Drive node", () => {
    const graph = sampleGraph();
    const drive = graph.edges.find((e: { from_node: number }) => e.from_node === 3);
    drive.delayed_feedback = true;
    expect(refusal(() => checkGraph(graphDrawn(graph))).error).toBe("patchback.studio.FeedbackNotAtDelay");
  });

  it("a node drawn twice", () => {
    const graph = sampleGraph();
    graph.nodes[1].node_id = graph.nodes[0].node_id;
    expect(refusal(() => checkGraph(graphDrawn(graph))).error).toBe("patchback.studio.DuplicateGraphNode");
  });

  it("answers in the domain's order: a missing endpoint before feedback not at a Delay", () => {
    const graph = sampleGraph();
    graph.edges[0].delayed_feedback = true;
    graph.edges[6].to_node = 9;
    expect(refusal(() => checkGraph(graphDrawn(graph))).error).toBe("patchback.studio.GraphEndpointMissing");
  });

  it("studio-patch-graph-delayed-feedback: feedback leaving a Delay node is drawn", () => {
    const graph = checkGraph(graphDrawn(feedbackJson.graph));
    expect(graph.edges.filter((e) => e.delayed_feedback).map((e) => [e.from_node, e.to_node])).toEqual([[10, 3]]);
  });
});
