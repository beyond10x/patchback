// The document `patchback-studio export --run <dir> --out <file>` writes from a fit run
// (crates/patchback-studio, approval-record:wave-c1 Dc1-21): the run's patch drawn as a graph, the
// modules it uses and the replay of its trace.events. The studio reads only this document; Rust is
// the only reader of Patchback's own files. Loading checks the document's shape and the refusals of
// patchback.studio.DrawGraph, OpenTrace, AppendFrame and CloseTrace, so a file picked by hand is
// refused, not drawn wrongly.
import type {
  FrameAppended,
  GraphDrawn,
  GraphEdge,
  GraphNode,
  LossComponents,
  ModuleSpec,
  ParameterContract,
  PortSpec,
  TraceClosedEvent,
  TraceOpened,
} from "./generated/types";

export const FORMAT = "patchback-studio-export/1";

export interface StudioDocument {
  format: typeof FORMAT;
  topology: string;
  graph: GraphDrawn;
  modules: ModuleSpec[];
  trace: Replay;
}

/** A refused document: the patchback.studio error it is an instance of, or `malformed`. */
export class DocumentError extends Error {
  constructor(
    readonly error: string,
    message: string,
  ) {
    super(`${error}: ${message}`);
    this.name = "DocumentError";
  }
}

const NODE_KINDS = [
  "NoteInput",
  "SawOscillator",
  "Drive",
  "LowpassFilter",
  "AdsrEnvelope",
  "Vca",
  "Output",
  "SquareOscillator",
  "SineOscillator",
  "HighpassFilter",
  "BandpassFilter",
  "Lfo",
  "Delay",
  "Noise",
] as const;
const RATES = ["Audio", "Control", "Event"] as const;

type Json = unknown;

function malformed(at: string, what: string): never {
  throw new DocumentError("malformed", `${at} is not ${what}`);
}

function object(value: Json, at: string): Record<string, Json> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) malformed(at, "an object");
  return value as Record<string, Json>;
}

function array(value: Json, at: string): Json[] {
  if (!Array.isArray(value)) malformed(at, "a list");
  return value;
}

function string(value: Json, at: string): string {
  if (typeof value !== "string") malformed(at, "a string");
  return value;
}

function integer(value: Json, at: string, min = 0): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min)
    malformed(at, `an integer of at least ${min}`);
  return value;
}

function boolean(value: Json, at: string): boolean {
  if (typeof value !== "boolean") malformed(at, "true or false");
  return value;
}

export function decimal(value: Json, at: string): string {
  const text = string(value, at);
  if (!/^-?(\d+)(\.\d+)?([eE]-?\d+)?$/.test(text) || !Number.isFinite(Number(text))) malformed(at, "a decimal");
  return text;
}

function oneOf<T extends string>(value: Json, at: string, options: readonly T[]): T {
  if (typeof value !== "string" || !(options as readonly string[]).includes(value))
    malformed(at, `one of ${options.join(", ")}`);
  return value as T;
}

function uuid(value: Json, at: string): string {
  const text = string(value, at);
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(text)) malformed(at, "a UUID");
  return text;
}

function node(value: Json, at: string): GraphNode {
  const o = object(value, at);
  return {
    node_id: integer(o.node_id, `${at}.node_id`),
    kind: oneOf(o.kind, `${at}.kind`, NODE_KINDS),
    rank: integer(o.rank, `${at}.rank`),
  };
}

function edge(value: Json, at: string): GraphEdge {
  const o = object(value, at);
  return {
    from_node: integer(o.from_node, `${at}.from_node`),
    from_port: string(o.from_port, `${at}.from_port`),
    to_node: integer(o.to_node, `${at}.to_node`),
    to_port: string(o.to_port, `${at}.to_port`),
    rate: oneOf(o.rate, `${at}.rate`, RATES),
    delayed_feedback: boolean(o.delayed_feedback, `${at}.delayed_feedback`),
  };
}

/** A GraphDrawn event, its shape checked. */
export function graphDrawn(value: Json, at = "graph"): GraphDrawn {
  const o = object(value, at);
  return {
    graph_view_id: uuid(o.graph_view_id, `${at}.graph_view_id`),
    patch_id: uuid(o.patch_id, `${at}.patch_id`),
    topology_id: uuid(o.topology_id, `${at}.topology_id`),
    nodes: array(o.nodes, `${at}.nodes`).map((n, i) => node(n, `${at}.nodes[${i}]`)),
    edges: array(o.edges, `${at}.edges`).map((e, i) => edge(e, `${at}.edges[${i}]`)),
  };
}

function port(value: Json, at: string): PortSpec {
  const o = object(value, at);
  return {
    name: string(o.name, `${at}.name`),
    direction: oneOf(o.direction, `${at}.direction`, ["Input", "Output"] as const),
    rate: oneOf(o.rate, `${at}.rate`, RATES),
    meaning: string(o.meaning, `${at}.meaning`),
  };
}

function contract(value: Json, at: string): ParameterContract {
  const o = object(value, at);
  return {
    path: string(o.path, `${at}.path`),
    index: integer(o.index, `${at}.index`),
    unit: oneOf(o.unit, `${at}.unit`, ["Seconds", "LinearAmplitude", "Hertz", "Dimensionless", "Decibels"] as const),
    mapping: oneOf(o.mapping, `${at}.mapping`, ["Exponential", "Linear"] as const),
    physical_at_zero: decimal(o.physical_at_zero, `${at}.physical_at_zero`),
    physical_at_one: decimal(o.physical_at_one, `${at}.physical_at_one`),
    default_normalized: decimal(o.default_normalized, `${at}.default_normalized`),
    clamp: oneOf(o.clamp, `${at}.clamp`, ["ClampToUnitInterval"] as const),
  };
}

/** A list of ModuleSpec, its shape checked. */
export function moduleSpecs(value: Json, at = "modules"): ModuleSpec[] {
  return array(value, at).map((m, i) => {
    const o = object(m, `${at}[${i}]`);
    const spec: ModuleSpec = {
      kind: oneOf(o.kind, `${at}[${i}].kind`, NODE_KINDS),
      ports: array(o.ports, `${at}[${i}].ports`).map((p, j) => port(p, `${at}[${i}].ports[${j}]`)),
      parameters: array(o.parameters, `${at}[${i}].parameters`).map((c, j) =>
        contract(c, `${at}[${i}].parameters[${j}]`),
      ),
    };
    if (o.family !== undefined)
      spec.family = oneOf(o.family, `${at}[${i}].family`, ["SquareSine", "HighpassBandpass", "Lfo", "Delay", "Noise"] as const);
    return spec;
  });
}

/**
 * patchback.studio.DrawGraph's refusals, in the order the domain declares them: a node drawn
 * twice, an edge whose end is not a drawn node, an edge marked as delayed feedback that does not
 * leave a Delay node (DESIGN.md 5.4).
 */
export function checkGraph(graph: GraphDrawn): GraphDrawn {
  const ids = new Set<number>();
  for (const n of graph.nodes) {
    if (ids.has(n.node_id))
      throw new DocumentError("patchback.studio.DuplicateGraphNode", `node ${n.node_id} is drawn twice`);
    ids.add(n.node_id);
  }
  for (const e of graph.edges) {
    if (!ids.has(e.from_node) || !ids.has(e.to_node))
      throw new DocumentError(
        "patchback.studio.GraphEndpointMissing",
        `the edge ${e.from_node} -> ${e.to_node} names a node the drawing does not hold`,
      );
  }
  const kind = new Map(graph.nodes.map((n) => [n.node_id, n.kind]));
  for (const e of graph.edges) {
    if (e.delayed_feedback && kind.get(e.from_node) !== "Delay")
      throw new DocumentError(
        "patchback.studio.FeedbackNotAtDelay",
        `the edge ${e.from_node} -> ${e.to_node} is marked as delayed feedback and leaves a ${kind.get(e.from_node)} node`,
      );
  }
  return graph;
}

const OUTCOMES = ["FirstRetained", "Improved", "NotImproved", "Rejected"] as const;
const STOP_REASONS = [
  "RenderBudgetExhausted",
  "TimeBudgetExhausted",
  "RestartsExhausted",
  "ImprovementBelowThreshold",
] as const;

function optional<T>(value: Json, read: (v: Json) => T): T | undefined {
  return value === undefined ? undefined : read(value);
}

function opened(value: Json, at: string): TraceOpened {
  const o = object(value, at);
  return {
    trace_id: uuid(o.trace_id, `${at}.trace_id`),
    fit_run_id: uuid(o.fit_run_id, `${at}.fit_run_id`),
    source: oneOf(o.source, `${at}.source`, ["Replay", "Live"] as const),
    topology_id: uuid(o.topology_id, `${at}.topology_id`),
    topology_name: string(o.topology_name, `${at}.topology_name`),
    parameter_paths: array(o.parameter_paths, `${at}.parameter_paths`).map((p, i) =>
      string(p, `${at}.parameter_paths[${i}]`),
    ),
  };
}

function components(value: Json, at: string): LossComponents {
  const o = object(value, at);
  return {
    log_spectral: decimal(o.log_spectral, `${at}.log_spectral`),
    envelope: decimal(o.envelope, `${at}.envelope`),
    transient: decimal(o.transient, `${at}.transient`),
    band_energy: decimal(o.band_energy, `${at}.band_energy`),
  };
}

function frame(value: Json, at: string): FrameAppended {
  const o = object(value, at);
  const f: FrameAppended = {
    frame_id: uuid(o.frame_id, `${at}.frame_id`),
    trace_id: uuid(o.trace_id, `${at}.trace_id`),
    sequence: integer(o.sequence, `${at}.sequence`, 1),
    evaluation: integer(o.evaluation, `${at}.evaluation`, 1),
    restart: integer(o.restart, `${at}.restart`),
    outcome: oneOf(o.outcome, `${at}.outcome`, OUTCOMES),
    topology_id: uuid(o.topology_id, `${at}.topology_id`),
    topology_name: string(o.topology_name, `${at}.topology_name`),
    parameters: array(o.parameters, `${at}.parameters`).map((p, i) => {
      const s = object(p, `${at}.parameters[${i}]`);
      return {
        path: string(s.path, `${at}.parameters[${i}].path`),
        normalized_value: decimal(s.normalized_value, `${at}.parameters[${i}].normalized_value`),
      };
    }),
  };
  const patchId = optional(o.patch_id, (v) => uuid(v, `${at}.patch_id`));
  const total = optional(o.total_loss, (v) => decimal(v, `${at}.total_loss`));
  const parts = optional(o.components, (v) => components(v, `${at}.components`));
  const best = optional(o.best_loss, (v) => decimal(v, `${at}.best_loss`));
  if (patchId !== undefined) f.patch_id = patchId;
  if (total !== undefined) f.total_loss = total;
  if (parts !== undefined) f.components = parts;
  if (best !== undefined) f.best_loss = best;
  return f;
}

function closed(value: Json, at: string): TraceClosedEvent {
  const o = object(value, at);
  const c: TraceClosedEvent = {
    trace_id: uuid(o.trace_id, `${at}.trace_id`),
    ended: oneOf(o.ended, `${at}.ended`, ["Completed", "Failed"] as const),
  };
  const reason = optional(o.stop_reason, (v) => oneOf(v, `${at}.stop_reason`, STOP_REASONS));
  if (reason !== undefined) c.stop_reason = reason;
  return c;
}

/** A replayed trace: TraceOpened, its frames in order, TraceClosedEvent. */
export interface Replay {
  opened: TraceOpened;
  frames: FrameAppended[];
  closed: TraceClosedEvent;
}

/** A replay, its shape checked. */
export function replayOf(value: Json, at = "trace"): Replay {
  const o = object(value, at);
  return {
    opened: opened(o.opened, `${at}.opened`),
    frames: array(o.frames, `${at}.frames`).map((f, i) => frame(f, `${at}.frames[${i}]`)),
    closed: closed(o.closed, `${at}.closed`),
  };
}

/**
 * The refusals of patchback.studio.OpenTrace, AppendFrame and CloseTrace over a replay, in the
 * domain's precedence order for each command, as crates/patchback-studio answers them: a path
 * named twice; a rejected frame with a loss, a valid one without, a retained one whose loss is
 * not the best (InconsistentFrame); a frame of another trace; a sequence not above every earlier
 * one (FrameOutOfOrder); a best loss above an earlier one (BestLossWorsened); a completed end
 * without a stop reason or a failed one with one (InconsistentEnd). Decimals compare as numbers.
 */
export function checkReplay(replay: Replay): Replay {
  const { opened: o, frames, closed: c } = replay;
  const id = o.trace_id;
  if (new Set(o.parameter_paths).size !== o.parameter_paths.length)
    throw new DocumentError("patchback.studio.DuplicateParameterPath", `trace ${id} names a path twice`);
  let last = 0;
  let best = Infinity;
  for (const f of frames) {
    const at = `trace ${id}, sequence ${f.sequence}`;
    const rejected = f.outcome === "Rejected";
    const kept = f.outcome === "FirstRetained" || f.outcome === "Improved";
    const total = f.total_loss === undefined ? undefined : Number(f.total_loss);
    const fb = f.best_loss === undefined ? undefined : Number(f.best_loss);
    if ((rejected && total !== undefined) || (!rejected && total === undefined) || (kept && total !== fb))
      throw new DocumentError("patchback.studio.InconsistentFrame", `${at}: the outcome disagrees with its losses`);
    if (f.trace_id !== id) throw new DocumentError("patchback.studio.TraceNotFound", `${at}: the frame is of trace ${f.trace_id}`);
    if (f.sequence <= last)
      throw new DocumentError("patchback.studio.FrameOutOfOrder", `${at}: not above the sequence ${last} before it`);
    if (fb !== undefined && fb > best)
      throw new DocumentError("patchback.studio.BestLossWorsened", `${at}: the best loss rose above ${best}`);
    last = f.sequence;
    if (fb !== undefined) best = fb;
  }
  if (c.trace_id !== id) throw new DocumentError("patchback.studio.TraceNotFound", `the close is of trace ${c.trace_id}`);
  if ((c.ended === "Completed") !== (c.stop_reason !== undefined))
    throw new DocumentError(
      "patchback.studio.InconsistentEnd",
      `trace ${id}: a completed run names a stop reason and a failed run none`,
    );
  return replay;
}

/** A document already parsed from JSON, checked. */
export function checkDocument(value: Json): StudioDocument {
  const o = object(value, "the document");
  if (o.format !== FORMAT) malformed("format", `"${FORMAT}"`);
  return {
    format: FORMAT,
    topology: string(o.topology, "topology"),
    graph: checkGraph(graphDrawn(o.graph)),
    modules: moduleSpecs(o.modules),
    trace: checkReplay(replayOf(o.trace)),
  };
}

/** The text of an exported document, parsed and checked. */
export function parseDocument(text: string): StudioDocument {
  let value: Json;
  try {
    value = JSON.parse(text);
  } catch (error) {
    throw new DocumentError("malformed", `not JSON (${(error as Error).message})`);
  }
  return checkDocument(value);
}
