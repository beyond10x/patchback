// story:studio-fit-replay, approval-record:wave-c1 Dc1-26: the loss chart and the trajectories draw
// each series as one path, so the number of DOM nodes does not grow with the number of frames;
// the loss chart marks a topology change.
import { afterEach, describe, expect, it } from "vitest";
import { createApp, h, type App, type Component } from "vue";
import topologyChange from "../../fixtures/trace-topology-change.json";
import type { FrameAppended } from "../generated/types";
import { checkReplay, replayOf } from "../document";
import { sample } from "../store";
import LossChart from "./LossChart.vue";
import Trajectories from "./Trajectories.vue";

let app: App | undefined;
afterEach(() => {
  app?.unmount();
  app = undefined;
  document.body.innerHTML = "";
});

function mount(component: Component, props: Record<string, unknown>): HTMLElement {
  const root = document.createElement("div");
  document.body.appendChild(root);
  app = createApp({ render: () => h(component, props) });
  app.mount(root);
  return root;
}

const nodes = (root: HTMLElement) => root.querySelectorAll("*").length;
const all = sample.document.trace.frames;
const paths = sample.document.trace.opened.parameter_paths;

/** n frames spread over the sample, its first and its highest-loss frame among them, so the
 * axes are the same and only the number of frames differs. */
function spread(n: number): FrameAppended[] {
  const top = all.reduce((m, f, i) => (Number(f.total_loss ?? 0) > Number(all[m]!.total_loss ?? 0) ? i : m), 0);
  const picks = Array.from({ length: n }, (_, k) => Math.round((k * (all.length - 1)) / (n - 1)));
  if (!picks.includes(top)) picks[1] = top;
  return picks.map((i) => all[i]!);
}

function counts(component: Component, extra: Record<string, unknown> = {}) {
  const sizes = [20, 100, all.length];
  return sizes.map((n) => {
    const frames = spread(n);
    const root = mount(component, { frames, index: Math.floor(n / 2), ...extra });
    const count = nodes(root);
    app?.unmount();
    app = undefined;
    document.body.innerHTML = "";
    return count;
  });
}

describe("charts over many frames", () => {
  it("the sample is long enough to tell", () => {
    expect(all.length).toBeGreaterThanOrEqual(400);
  });

  it("LossChart draws as many DOM nodes for 400 frames as for 20", () => {
    const [few, some, many] = counts(LossChart);
    expect(some).toBe(few);
    expect(many).toBe(few);
  });

  it("Trajectories draws as many DOM nodes for 400 frames as for 20", () => {
    const [few, some, many] = counts(Trajectories, { paths });
    expect(some).toBe(few);
    expect(many).toBe(few);
  });

  it("LossChart draws one dot per valid candidate up to the cursor in one path", () => {
    const frames = all.slice(0, 50);
    const root = mount(LossChart, { frames, index: 49 });
    const kept = root.querySelector(".wrap path.kept:not(.future)")!.getAttribute("d")!;
    const tried = root.querySelector(".wrap path.tried:not(.future)")!.getAttribute("d")!;
    const dots = (d: string) => (d.match(/h 0/g) ?? []).length;
    const valid = frames.filter((f) => f.total_loss !== undefined);
    expect(dots(kept) + dots(tried)).toBe(valid.length);
  });
});

describe("the topology-change marker", () => {
  it("is drawn where a frame's topology differs from the one before, and only there", () => {
    const { frames } = checkReplay(replayOf(topologyChange.trace));
    const root = mount(LossChart, { frames, index: frames.length - 1 });
    const marks = root.querySelector("path.topo:not(.future)")!.getAttribute("d")!;
    expect((marks.match(/M/g) ?? []).length).toBe(1);
    app?.unmount();
    const real = mount(LossChart, { frames: all, index: all.length - 1 });
    expect(real.querySelector("path.topo:not(.future)")!.getAttribute("d") ?? "").toBe("");
  });
});
