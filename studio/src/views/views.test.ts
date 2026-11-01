// story:studio-fit-replay through the real views. The bundled sample has no rejected frame, so a
// rejected one is drawn from studio/fixtures/export-replayed-lines.json: the export
// crates/patchback-studio/tests/fit_replay.rs makes of its four-line run (one evaluation of each
// outcome, a restart, the fourth rejected) and holds equal to the file. A replay without
// evaluations is a valid closed trace (spec/domains/studio.yaml: CloseTrace asks for no frame),
// so the views draw it.
import { afterEach, describe, expect, it } from "vitest";
import { createApp, h, type App, type Component } from "vue";
import replayedLines from "../../fixtures/export-replayed-lines.json?raw";
import sampleText from "../../fixtures/sample-run.json?raw";
import { load, loaded, sample } from "../store";
import EvolutionView from "./EvolutionView.vue";
import ResultView from "./ResultView.vue";

let app: App | undefined;
afterEach(() => {
  app?.unmount();
  app = undefined;
  document.body.innerHTML = "";
  loaded.value = sample;
  window.history.replaceState(null, "", "/");
});

function mount(view: Component): { root: HTMLElement; errors: unknown[] } {
  const errors: unknown[] = [];
  const root = document.createElement("div");
  document.body.appendChild(root);
  app = createApp({ render: () => h(view) });
  app.config.errorHandler = (error) => {
    errors.push(error);
  };
  app.mount(root);
  return { root, errors };
}

const text = (root: HTMLElement, selector: string) =>
  Array.from(root.querySelectorAll(selector)).map((e) => e.textContent?.trim() ?? "");

describe("a rejected frame in the evolution view", () => {
  it("shows its outcome, no candidate loss, a cross on the loss chart and its table row", () => {
    expect(load("export-replayed-lines.json", replayedLines)).toBe(true);
    window.history.replaceState(null, "", "/?frame=3");
    const { root, errors } = mount(EvolutionView);
    expect(errors).toEqual([]);

    const values = text(root, ".stat .value");
    expect(values[0]).toBe("rejected, no loss");
    expect(values[1]).toBe("none");
    expect(values[2]).toBe("0.43");
    expect(values[4]).toBe("1");

    const crosses = root.querySelector(".wrap path.rej:not(.future)")?.getAttribute("d") ?? "";
    expect((crosses.match(/l 8 8/g) ?? []).length).toBe(1);
    expect(root.querySelector("circle.now")?.getAttribute("visibility")).toBe("hidden");

    const row = Array.from(root.querySelectorAll("tbody tr")).find((tr) => tr.classList.contains("current"));
    expect(Array.from(row?.querySelectorAll("td") ?? []).map((td) => td.textContent?.trim())).toEqual([
      "4",
      "4",
      "1",
      "rejected, no loss",
      "–",
      "0.43",
      "–",
      "–",
      "–",
      "–",
    ]);
  });
});

describe("a replay without evaluations", () => {
  const empty = () => {
    const document = JSON.parse(sampleText);
    document.trace.frames = [];
    expect(load("no-evaluations.json", JSON.stringify(document))).toBe(true);
  };

  it("is drawn by the evolution view as no evaluations", () => {
    empty();
    const { root, errors } = mount(EvolutionView);
    expect(errors).toEqual([]);
    expect(root.textContent).toContain("no evaluations");
  });

  it("has no end result: the result view says so", () => {
    empty();
    const { root, errors } = mount(ResultView);
    expect(errors).toEqual([]);
    expect(root.textContent).toContain("no retained patch");
  });
});
