// story:studio-patch-graph: the drawn graph. Delayed feedback is routed as a labelled return path
// under every node (DESIGN.md 16, "a visible z⁻¹ return path"); forward edges are not.
import { afterEach, describe, expect, it } from "vitest";
import { createApp, h, type App } from "vue";
import PatchGraph from "./PatchGraph.vue";
import { feedbackExample } from "../model";
import { sample } from "../store";
import type { GraphDrawn, ModuleSpec } from "../generated/types";

let app: App | undefined;
afterEach(() => {
  app?.unmount();
  app = undefined;
  document.body.innerHTML = "";
});

function mount(props: { graph: GraphDrawn; modules: ModuleSpec[] }): HTMLElement {
  const root = document.createElement("div");
  document.body.appendChild(root);
  app = createApp({ render: () => h(PatchGraph, props) });
  app.mount(root);
  return root;
}

/** Every y coordinate a path's `d` names (x and y alternate after each command letter). */
function ys(d: string): number[] {
  const numbers = d
    .replace(/[MCL,]/g, " ")
    .trim()
    .split(/\s+/)
    .map(Number);
  return numbers.filter((_, i) => i % 2 === 1);
}

function nodeBottom(root: HTMLElement): number {
  return Math.max(
    ...[...root.querySelectorAll("g.node rect.body")].map(
      (r) => Number(r.getAttribute("y")) + Number(r.getAttribute("height")),
    ),
  );
}

describe("PatchGraph", () => {
  it("routes the edge leaving the Delay under every node, labelled as delayed feedback", () => {
    const root = mount({ graph: feedbackExample.graph, modules: feedbackExample.modules });
    const bottom = nodeBottom(root);
    const cables = [...root.querySelectorAll<SVGGElement>("g.cable")];
    expect(cables).toHaveLength(feedbackExample.graph.edges.length);
    const back = cables.filter((c) => c.dataset.from === "10" && c.dataset.to === "3");
    expect(back).toHaveLength(1);
    expect(back[0]!.querySelector("text.fb-label")?.textContent?.trim()).toBe("z⁻¹ delayed feedback");
    const d = back[0]!.querySelector("path.cable-core")!.getAttribute("d")!;
    expect(Math.max(...ys(d))).toBeGreaterThan(bottom);

    // the Delay's forward edge to the output and the rest of the chain stay above the lane
    for (const cable of cables.filter((c) => c !== back[0])) {
      expect(cable.querySelector("text.fb-label")).toBeNull();
      const forward = cable.querySelector("path.cable-core")!.getAttribute("d")!;
      expect(Math.max(...ys(forward))).toBeLessThanOrEqual(bottom);
    }
  });

  it("routes a backward edge that is not feedback under the nodes without the feedback label", () => {
    const graph = structuredClone(sample.document.graph);
    const output = graph.nodes.find((n) => n.kind === "Output")!;
    output.rank = 0;
    const root = mount({ graph, modules: sample.document.modules });
    const bottom = nodeBottom(root);
    const into = [...root.querySelectorAll<SVGGElement>("g.cable")].find((c) => c.dataset.to === String(output.node_id))!;
    const d = into.querySelector("path.cable-core")!.getAttribute("d")!;
    expect(Math.max(...ys(d))).toBeGreaterThan(bottom);
    expect(root.querySelectorAll("text.fb-label")).toHaveLength(0);
    expect(Number(root.querySelector("svg")!.getAttribute("height"))).toBeGreaterThan(Math.max(...ys(d)));
  });

  it("draws the exported voice with a node per node, a cable per edge and no return path", () => {
    const { graph, modules } = sample.document;
    const root = mount({ graph, modules });
    expect(root.querySelectorAll("g.node")).toHaveLength(graph.nodes.length);
    expect(root.querySelectorAll("g.cable")).toHaveLength(graph.edges.length);
    expect(root.querySelectorAll("text.fb-label")).toHaveLength(0);
    // ports come from the export, so every edge lands on a declared jack
    const jacks = [...root.querySelectorAll("text.port-label")].map((t) => t.textContent?.trim());
    for (const e of graph.edges) {
      expect(jacks).toContain(e.from_port);
      expect(jacks).toContain(e.to_port);
    }
  });
});
