import type {
  CataloguePublished,
  FrameAppended,
  GraphDrawn,
  ModuleSpec,
  NodeKind,
  ParameterContract,
  PortRate,
  ResultComposed,
  TraceClosedEvent,
  TraceOpened,
} from "./generated/types";
import catalogueJson from "../fixtures/catalogue.json";
import voiceJson from "../fixtures/graph-subtractive-mono-1.json";
import feedbackJson from "../fixtures/graph-delayed-feedback.json";
import traceJson from "../fixtures/trace.json";
import resultJson from "../fixtures/result.json";

export interface Fixture<T> {
  note: string;
  event: T;
}

export interface TraceFixture {
  note: string;
  opened: TraceOpened;
  frames: FrameAppended[];
  closed: TraceClosedEvent;
}

export interface ResultFixture extends Fixture<ResultComposed> {
  reference_parameters: { path: string; normalized_value: string }[];
}

export const catalogue = catalogueJson as unknown as Fixture<CataloguePublished>;
export const graphs: { label: string; fixture: Fixture<GraphDrawn> }[] = [
  { label: "subtractive-mono/1", fixture: voiceJson as unknown as Fixture<GraphDrawn> },
  { label: "delayed feedback", fixture: feedbackJson as unknown as Fixture<GraphDrawn> },
];
export const trace = checkTrace(traceJson as unknown as TraceFixture);
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

function checkTrace(t: TraceFixture): TraceFixture {
  let best = Infinity;
  t.frames.forEach((f, i) => {
    if (f.sequence !== i) throw new Error(`trace frame ${i} has sequence ${f.sequence}`);
    if (f.outcome === "Rejected" && f.total_loss !== undefined)
      throw new Error(`trace frame ${i} is rejected and carries a loss`);
    const b = decimal(f.best_loss);
    if (b !== undefined) {
      if (b > best) throw new Error(`trace frame ${i} raises the best loss`);
      best = b;
    }
    if (retained(f) && f.total_loss !== f.best_loss)
      throw new Error(`trace frame ${i} is retained but its loss is not the best`);
  });
  return t;
}

export function retainedAt(frames: FrameAppended[], index: number): FrameAppended | undefined {
  for (let i = index; i >= 0; i--) {
    const f = frames[i];
    if (f && retained(f)) return f;
  }
  return undefined;
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
