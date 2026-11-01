import type {
  CataloguePublished,
  FrameAppended,
  ModuleSpec,
  NodeKind,
  ParameterContract,
  PortRate,
  ResultComposed,
} from "./generated/types";
import catalogueJson from "../fixtures/catalogue.json";
import feedbackJson from "../fixtures/graph-delayed-feedback.json";
import resultJson from "../fixtures/result.json";
import { checkGraph, graphDrawn, moduleSpecs } from "./document";

export interface Fixture<T> {
  note: string;
  event: T;
}

export interface ResultFixture extends Fixture<ResultComposed> {
  reference_parameters: { path: string; normalized_value: string }[];
}

export const catalogue = catalogueJson as unknown as Fixture<CataloguePublished>;
// DESIGN.md 5.4: no topology admits a Delay node yet, so no exported run shows delayed feedback; this
// labelled illustrative graph does, checked like an export.
export const feedbackExample = {
  note: feedbackJson.note,
  graph: checkGraph(graphDrawn(feedbackJson.graph)),
  modules: moduleSpecs(feedbackJson.modules),
};
export const result = resultJson as unknown as ResultFixture;

export function decimal(value: string | undefined): number | undefined {
  return value === undefined ? undefined : Number(value);
}

export const RATE_COLOR: Record<PortRate, string> = {
  Audio: "var(--rate-audio)",
  Control: "var(--rate-control)",
  Event: "var(--rate-event)",
};

export const RATE_DASH: Record<PortRate, string> = {
  Audio: "",
  Control: "8 5",
  Event: "1 5",
};

export const RATES: PortRate[] = ["Audio", "Control", "Event"];

export const MODULE_COLOR: Record<NodeKind, string> = {
  NoteInput: "var(--pb-lime)",
  SawOscillator: "var(--pb-cyan)",
  SquareOscillator: "var(--pb-cyan)",
  SineOscillator: "var(--pb-cyan)",
  Noise: "var(--pb-cyan)",
  Drive: "var(--pb-orange)",
  LowpassFilter: "var(--pb-blue)",
  HighpassFilter: "var(--pb-blue)",
  BandpassFilter: "var(--pb-blue)",
  AdsrEnvelope: "var(--pb-purple)",
  Lfo: "var(--pb-purple)",
  Vca: "var(--pb-pink)",
  Delay: "var(--pb-magenta)",
  Output: "var(--pb-magenta)",
};

export const moduleByKind = new Map<NodeKind, ModuleSpec>(
  catalogue.event.modules.map((m) => [m.kind, m]),
);

export const contractByPath = new Map<string, ParameterContract>(
  catalogue.event.modules.flatMap((m) => m.parameters.map((p) => [p.path, p] as const)),
);

export function physical(contract: ParameterContract, normalized: number): number {
  const p = Math.min(1, Math.max(0, normalized));
  const lo = Number(contract.physical_at_zero);
  const hi = Number(contract.physical_at_one);
  return contract.mapping === "Exponential" ? lo * Math.pow(hi / lo, p) : lo + (hi - lo) * p;
}

export function formatPhysical(contract: ParameterContract, normalized: number): string {
  const v = physical(contract, normalized);
  switch (contract.unit) {
    case "Seconds":
      return v < 1 ? `${(v * 1000).toFixed(v < 0.01 ? 1 : 0)} ms` : `${v.toFixed(2)} s`;
    case "Hertz":
      return v < 1000 ? `${v.toFixed(0)} Hz` : `${(v / 1000).toFixed(2)} kHz`;
    case "Decibels":
      return `${v.toFixed(1)} dB`;
    case "LinearAmplitude":
      return v.toFixed(2);
    default:
      return `Q ${v.toFixed(2)}`;
  }
}

export function samplesOf(frame: { parameters: { path: string; normalized_value: string }[] }) {
  return Object.fromEntries(frame.parameters.map((s) => [s.path, Number(s.normalized_value)]));
}

export function retained(frame: FrameAppended): boolean {
  return frame.outcome === "FirstRetained" || frame.outcome === "Improved";
}

export function retainedAt(frames: FrameAppended[], index: number): FrameAppended | undefined {
  for (let i = index; i >= 0; i--) {
    const f = frames[i];
    if (f && retained(f)) return f;
  }
  return undefined;
}

/** For each frame, the index of the last retained frame at or before it, or -1: one pass. */
export function retainedIndex(frames: FrameAppended[]): Int32Array {
  const out = new Int32Array(frames.length);
  let last = -1;
  frames.forEach((f, i) => {
    if (retained(f)) last = i;
    out[i] = last;
  });
  return out;
}

/** min(n, count) evenly spaced frame indices from 0 to n - 1, both ends included. */
export function ticks(n: number, count: number): number[] {
  const k = Math.min(n, count);
  if (k <= 1) return k === 1 ? [0] : [];
  return Array.from({ length: k }, (_, j) => Math.round((j * (n - 1)) / (k - 1)));
}

export function changedPath(frames: FrameAppended[], index: number): string | undefined {
  const frame = frames[index];
  const base = retainedAt(frames, index - 1);
  if (!frame || !base) return undefined;
  const a = samplesOf(base);
  return frame.parameters.find((s) => Number(s.normalized_value) !== a[s.path])?.path;
}

export function saw(values: Record<string, number>, f0 = 110) {
  const cutoff = contractByPath.get("filter_cutoff");
  const q = contractByPath.get("resonance_q");
  const gain = contractByPath.get("output_gain");
  const fc = cutoff ? physical(cutoff, values.filter_cutoff ?? 0.5) : 1000;
  const Q = q ? physical(q, values.resonance_q ?? 0.5) : 0.7;
  const g = gain ? physical(gain, values.output_gain ?? 0.5) : 0;
  const points: { hz: number; db: number }[] = [];
  for (let n = 1; n * f0 <= 16000; n++) {
    const f = n * f0;
    const r = f / fc;
    const h = 1 / Math.sqrt((1 - r * r) ** 2 + (r / Q) ** 2);
    points.push({ hz: f, db: 20 * Math.log10(h / n) + g });
  }
  return points;
}
