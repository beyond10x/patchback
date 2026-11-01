// Adversary pass 1 on story:studio-fit-replay. document.ts promises that "a file picked by hand is
// refused, not drawn wrongly". A replay with no frames passes every check of checkDocument (and
// `patchback-studio export` writes one from a trace.events with no evaluation line), so the loader
// accepts it; the evolution view must then draw it, or the loader must refuse it.
import { afterEach, describe, expect, it } from "vitest";
import { createApp, h, type App } from "vue";
import sampleText from "../fixtures/sample-run.json?raw";
import { load, loaded, sample } from "./store";
import EvolutionView from "./views/EvolutionView.vue";

let app: App | undefined;
afterEach(() => {
  app?.unmount();
  app = undefined;
  document.body.innerHTML = "";
  loaded.value = sample;
});

describe("adversary 1: a replay without frames", () => {
  it("is refused by the loader or drawn by the evolution view without an error", () => {
    const text = JSON.parse(sampleText);
    text.trace.frames = [];
    const accepted = load("no-frames.json", JSON.stringify(text));
    if (!accepted) return;

    const errors: unknown[] = [];
    const root = document.createElement("div");
    document.body.appendChild(root);
    app = createApp({ render: () => h(EvolutionView) });
    app.config.errorHandler = (error) => {
      errors.push(error);
    };
    app.config.warnHandler = () => {};
    try {
      app.mount(root);
    } catch (error) {
      errors.push(error);
    }
    expect(errors.map(String)).toEqual([]);
  });
});
