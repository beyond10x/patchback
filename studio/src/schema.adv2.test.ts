// Adversary pass 2 on story:studio-patch-graph and story:studio-fit-replay (correction 1, 9b150d1).
// crates/patchback-studio/src/export.rs: "Every member but `format` and `topology` has the shape of
// a type `ess generate types` makes from spec/domains/studio.yaml into studio/src/generated/".
// The generated source.schema.json is that shape, Decimal included: a string matching
// ^-?(0|[1-9][0-9]*)(\.[0-9]+)?$. The committed exports are checked against it with the subset of
// JSON Schema the file uses ($ref, type, properties, required, additionalProperties, items, enum,
// pattern, minimum).
import { afterEach, describe, expect, it } from "vitest";
import { createApp, h, type App } from "vue";
import replayedLines from "../fixtures/export-replayed-lines.json?raw";
import sampleText from "../fixtures/sample-run.json?raw";
import LossChart from "./components/LossChart.vue";
import schemaText from "./generated/source.schema.json?raw";
import { load, loaded, sample } from "./store";
import EvolutionView from "./views/EvolutionView.vue";
import ResultView from "./views/ResultView.vue";

type Schema = Record<string, unknown>;
const schema = JSON.parse(schemaText) as { $defs: Record<string, Schema> };

function validate(value: unknown, s: Schema, at: string, out: string[]): void {
  if (typeof s.$ref === "string") {
    const name = s.$ref.replace("#/$defs/", "");
    const target = schema.$defs[name];
    if (!target) {
      out.push(`${at}: unknown $ref ${s.$ref}`);
      return;
    }
    validate(value, target, at, out);
    return;
  }
  switch (s.type) {
    case "object": {
      if (typeof value !== "object" || value === null || Array.isArray(value)) {
        out.push(`${at}: not an object`);
        return;
      }
      const o = value as Record<string, unknown>;
      const props = (s.properties ?? {}) as Record<string, Schema>;
      for (const key of (s.required ?? []) as string[]) if (!(key in o)) out.push(`${at}.${key}: required`);
      for (const [key, v] of Object.entries(o)) {
        if (props[key]) validate(v, props[key], `${at}.${key}`, out);
        else if (s.additionalProperties === false) out.push(`${at}.${key}: not a property of ${String(s.title)}`);
      }
      return;
    }
    case "array":
      if (!Array.isArray(value)) {
        out.push(`${at}: not an array`);
        return;
      }
      value.forEach((v, i) => validate(v, s.items as Schema, `${at}[${i}]`, out));
      return;
    case "integer":
      if (typeof value !== "number" || !Number.isInteger(value)) out.push(`${at}: not an integer`);
      else if (typeof s.minimum === "number" && value < s.minimum) out.push(`${at}: below ${s.minimum}`);
      return;
    case "boolean":
      if (typeof value !== "boolean") out.push(`${at}: not a boolean`);
      return;
    case "string":
      if (typeof value !== "string") {
        out.push(`${at}: not a string`);
        return;
      }
      if (Array.isArray(s.enum) && !s.enum.includes(value)) out.push(`${at}: ${value} not in ${s.enum.join("|")}`);
      if (typeof s.pattern === "string" && !new RegExp(s.pattern).test(value))
        out.push(`${at}: ${JSON.stringify(value)} does not match ${s.pattern}`);
      return;
    default:
      out.push(`${at}: schema type ${String(s.type)} not handled by this checker`);
  }
}

const ref = (name: string): Schema => ({ $ref: `#/$defs/patchback.studio.${name}` });

function violations(text: string): string[] {
  const doc = JSON.parse(text);
  const out: string[] = [];
  validate(doc.graph, ref("GraphDrawn"), "graph", out);
  (doc.modules as unknown[]).forEach((m, i) => validate(m, ref("ModuleSpec"), `modules[${i}]`, out));
  validate(doc.trace.opened, ref("TraceOpened"), "trace.opened", out);
  (doc.trace.frames as unknown[]).forEach((f, i) => validate(f, ref("FrameAppended"), `trace.frames[${i}]`, out));
  validate(doc.trace.closed, ref("TraceClosedEvent"), "trace.closed", out);
  return out;
}

describe("adversary 2: the exported documents against the ESS-generated schema", () => {
  it("every member of the committed exports validates against studio/src/generated/source.schema.json", () => {
    expect({
      "sample-run.json": violations(sampleText),
      "export-replayed-lines.json": violations(replayedLines),
    }).toEqual({ "sample-run.json": [], "export-replayed-lines.json": [] });
  });
});

let app: App | undefined;
afterEach(() => {
  app?.unmount();
  app = undefined;
  document.body.innerHTML = "";
  loaded.value = sample;
  window.history.replaceState(null, "", "/");
});

function mount(render: () => ReturnType<typeof h>): unknown[] {
  const errors: unknown[] = [];
  const root = document.createElement("div");
  document.body.appendChild(root);
  app = createApp({ render });
  app.config.errorHandler = (error) => {
    errors.push(error);
  };
  app.config.warnHandler = () => {};
  try {
    app.mount(root);
  } catch (error) {
    errors.push(error);
  }
  return errors;
}

describe("adversary 2: a replay whose every frame was rejected", () => {
  // checkDocument accepts it (a rejected frame without a loss or best is consistent), so the
  // views must draw it: no frame has a loss, no frame is retained.
  const allRejected = () => {
    const doc = JSON.parse(sampleText);
    for (const f of doc.trace.frames) {
      f.outcome = "Rejected";
      delete f.total_loss;
      delete f.components;
      delete f.best_loss;
      delete f.patch_id;
    }
    expect(load("all-rejected.json", JSON.stringify(doc))).toBe(true);
  };

  it("is drawn by the evolution view and the result view without an error", () => {
    allRejected();
    window.history.replaceState(null, "", "/?frame=5");
    const evolution = mount(() => h(EvolutionView));
    app?.unmount();
    app = undefined;
    const result = mount(() => h(ResultView));
    expect({ evolution: evolution.map(String), result: result.map(String) }).toEqual({ evolution: [], result: [] });
  });
});

describe("adversary 2: the loss chart over a long run", () => {
  // `patchback fit --max-renders` takes any count; one series per path is meant to keep a long
  // run drawable. 130 000 frames, each a copy of a sample frame with its own sequence.
  it("draws 130 000 frames without an error", () => {
    const base = sample.document.trace.frames;
    const frames = Array.from({ length: 130_000 }, (_, i) => ({ ...base[i % base.length]!, sequence: i + 1 }));
    const errors = mount(() => h(LossChart, { frames, index: 0 }));
    expect(errors.map(String)).toEqual([]);
  });
});
