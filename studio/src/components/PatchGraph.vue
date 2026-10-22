<script setup lang="ts">
import { computed, ref } from "vue";
import type { GraphDrawn, GraphEdge, GraphNode, PortSpec } from "../generated/types";
import { MODULE_COLOR, RATE_COLOR, RATE_DASH, contractByPath, formatPhysical, moduleByKind } from "../model";

const props = defineProps<{
  graph: GraphDrawn;
  values?: Record<string, number>;
  highlight?: string;
}>();

const W = 172;
const HEAD = 34;
const ROW = 20;
const KROW = 62;
const COL = 232;
const GAP = 34;
const PAD = 24;
const KR = 12;
const ARC = (KR + 3.5) * 1.5 * Math.PI;

interface Placed {
  node: GraphNode;
  x: number;
  y: number;
  h: number;
  inputs: PortSpec[];
  outputs: PortSpec[];
  params: string[];
  portRows: number;
}

const placed = computed<Placed[]>(() => {
  const byRank = new Map<number, GraphNode[]>();
  for (const n of props.graph.nodes) byRank.set(n.rank, [...(byRank.get(n.rank) ?? []), n]);
  const out: Placed[] = [];
  for (const [rank, nodes] of [...byRank.entries()].sort((a, b) => a[0] - b[0])) {
    let y = PAD;
    for (const node of nodes) {
      const spec = moduleByKind.get(node.kind);
      const inputs = spec?.ports.filter((p) => p.direction === "Input") ?? [];
      const outputs = spec?.ports.filter((p) => p.direction === "Output") ?? [];
      const params = props.values ? (spec?.parameters.map((p) => p.path) ?? []) : [];
      const portRows = Math.max(inputs.length, outputs.length, 1);
      const h = HEAD + portRows * ROW + Math.ceil(params.length / 2) * KROW + 12;
      out.push({ node, x: PAD + rank * COL, y, h, inputs, outputs, params, portRows });
      y += h + GAP;
    }
  }
  return out;
});

const byId = computed(() => new Map(placed.value.map((p) => [p.node.node_id, p])));
const bottom = computed(() => Math.max(...placed.value.map((p) => p.y + p.h)));
const hasFeedback = computed(() => props.graph.edges.some((e) => e.delayed_feedback));
const width = computed(() => Math.max(...placed.value.map((p) => p.x + W)) + PAD);
const height = computed(() => bottom.value + (hasFeedback.value ? 70 : PAD));

function portY(p: Placed, list: PortSpec[], name: string) {
  const i = Math.max(0, list.findIndex((s) => s.name === name));
  return p.y + HEAD + i * ROW + ROW / 2;
}

interface Drawn {
  edge: GraphEdge;
  d: string;
  label?: { x: number; y: number };
}

const edges = computed<Drawn[]>(() =>
  props.graph.edges.flatMap((edge) => {
    const a = byId.value.get(edge.from_node);
    const b = byId.value.get(edge.to_node);
    if (!a || !b) return [];
    const x1 = a.x + W;
    const y1 = portY(a, a.outputs, edge.from_port);
    const x2 = b.x;
    const y2 = portY(b, b.inputs, edge.to_port);
    if (edge.delayed_feedback || x2 <= x1) {
      const yb = bottom.value + 36;
      const d = `M ${x1} ${y1} C ${x1 + 60} ${y1}, ${x1 + 60} ${yb}, ${x1} ${yb} L ${x2} ${yb} C ${x2 - 60} ${yb}, ${x2 - 60} ${y2}, ${x2} ${y2}`;
      return [{ edge, d, label: { x: (x1 + x2) / 2, y: yb - 8 } }];
    }
    const between = placed.value.filter((p) => p.node.rank > a.node.rank && p.node.rank < b.node.rank);
    const lane = between.length ? Math.max(...between.map((p) => p.y + p.h)) + 16 : -Infinity;
    if (lane > Math.min(y1, y2)) {
      const d = `M ${x1} ${y1} C ${x1 + 40} ${y1}, ${x1 + 40} ${lane}, ${x1 + 80} ${lane} L ${x2 - 80} ${lane} C ${x2 - 40} ${lane}, ${x2 - 40} ${y2}, ${x2} ${y2}`;
      return [{ edge, d }];
    }
    const dx = Math.max(40, (x2 - x1) / 2);
    return [{ edge, d: `M ${x1} ${y1} C ${x1 + dx} ${y1}, ${x2 - dx} ${y2}, ${x2} ${y2}` }];
  }),
);

const tip = ref<{ x: number; y: number; lines: string[] } | null>(null);
const root = ref<HTMLDivElement | null>(null);

function show(ev: MouseEvent, lines: string[]) {
  const r = root.value?.getBoundingClientRect();
  if (!r) return;
  tip.value = { x: ev.clientX - r.left + 14, y: ev.clientY - r.top + 14, lines };
}

function nodeLines(p: Placed): string[] {
  const spec = moduleByKind.get(p.node.kind);
  const ports = spec?.ports.map((s) => `${s.direction === "Input" ? "in " : "out"} ${s.name} · ${s.rate}`) ?? [];
  return [`${p.node.kind} #${p.node.node_id}`, `rank ${p.node.rank}`, ...ports];
}

function edgeLines(e: GraphEdge): string[] {
  const a = byId.value.get(e.from_node)?.node.kind;
  const b = byId.value.get(e.to_node)?.node.kind;
  return [
    `${a}.${e.from_port} → ${b}.${e.to_port}`,
    `${e.rate} rate`,
    ...(e.delayed_feedback ? ["delayed feedback (≥ 1 sample)"] : []),
  ];
}

function paramLabel(path: string) {
  const c = contractByPath.get(path);
  const v = props.values?.[path] ?? 0;
  return c ? formatPhysical(c, v) : v.toFixed(3);
}

function knob(p: Placed, i: number) {
  const cx = p.x + (i % 2 ? W * 0.74 : W * 0.26);
  const cy = p.y + HEAD + p.portRows * ROW + 6 + Math.floor(i / 2) * KROW + KR + 4;
  return { cx, cy };
}

function track(cx: number, cy: number) {
  const r = KR + 3.5;
  const rad = (deg: number) => ((deg - 90) * Math.PI) / 180;
  const sx = cx + r * Math.cos(rad(-135));
  const sy = cy + r * Math.sin(rad(-135));
  const ex = cx + r * Math.cos(rad(135));
  const ey = cy + r * Math.sin(rad(135));
  return `M ${sx.toFixed(2)} ${sy.toFixed(2)} A ${r} ${r} 0 1 1 ${ex.toFixed(2)} ${ey.toFixed(2)}`;
}

function level(path: string) {
  return Math.min(1, Math.max(0, props.values?.[path] ?? 0));
}
</script>

<template>
  <div ref="root" class="graph-root" @mouseleave="tip = null">
    <svg
      :viewBox="`0 0 ${width} ${height}`"
      :width="width"
      :height="height"
      role="img"
      :aria-label="`Patch graph with ${graph.nodes.length} nodes and ${graph.edges.length} edges`"
    >
      <defs>
        <linearGradient id="pg-fade" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#fff" stop-opacity="0.05" />
          <stop offset="0.4" stop-color="#fff" stop-opacity="0" />
        </linearGradient>
      </defs>
      <g class="edges">
        <g v-for="(e, i) in edges" :key="i" class="cable" :class="'r-' + e.edge.rate" :style="{ '--c': RATE_COLOR[e.edge.rate] }">
          <path :d="e.d" class="cable-glow" />
          <path :d="e.d" class="cable-core" :stroke-dasharray="RATE_DASH[e.edge.rate] || undefined" />
          <path v-if="e.edge.rate === 'Audio'" :d="e.d" class="cable-pulse" />
          <path
            :d="e.d"
            fill="none"
            stroke="transparent"
            stroke-width="14"
            class="hit"
            @mousemove="show($event, edgeLines(e.edge))"
            @mouseleave="tip = null"
          />
          <text v-if="e.label" :x="e.label.x" :y="e.label.y" class="fb-label" text-anchor="middle">
            z⁻¹ delayed feedback
          </text>
        </g>
      </g>
      <g
        v-for="p in placed"
        :key="p.node.node_id"
        class="node"
        :class="{ lit: highlight && p.params.includes(highlight) }"
        :style="{ '--c': MODULE_COLOR[p.node.kind] }"
        @mousemove="show($event, nodeLines(p))"
        @mouseleave="tip = null"
      >
        <rect :x="p.x" :y="p.y" :width="W" :height="p.h" rx="12" class="body" />
        <rect :x="p.x" :y="p.y" :width="W" :height="p.h" rx="12" fill="url(#pg-fade)" />
        <rect :x="p.x + 1" :y="p.y + 1" :width="W - 2" height="3" rx="1.5" class="strip" />
        <text :x="p.x + 12" :y="p.y + 22" class="kind">{{ p.node.kind }}</text>
        <text :x="p.x + W - 24" :y="p.y + 22" class="nid" text-anchor="end">#{{ p.node.node_id }}</text>
        <circle :cx="p.x + W - 13" :cy="p.y + 18" r="3.5" class="led" />
        <g v-for="(s, i) in p.inputs" :key="'i' + s.name">
          <circle :cx="p.x" :cy="p.y + HEAD + i * ROW + ROW / 2" r="6" class="jack" />
          <circle :cx="p.x" :cy="p.y + HEAD + i * ROW + ROW / 2" r="2.8" class="jack-hole" :stroke="RATE_COLOR[s.rate]" />
          <text :x="p.x + 12" :y="p.y + HEAD + i * ROW + ROW / 2 + 4" class="port-label">{{ s.name }}</text>
        </g>
        <g v-for="(s, i) in p.outputs" :key="'o' + s.name">
          <circle :cx="p.x + W" :cy="p.y + HEAD + i * ROW + ROW / 2" r="6" class="jack" />
          <circle :cx="p.x + W" :cy="p.y + HEAD + i * ROW + ROW / 2" r="2.8" class="jack-hole" :stroke="RATE_COLOR[s.rate]" />
          <text :x="p.x + W - 12" :y="p.y + HEAD + i * ROW + ROW / 2 + 4" class="port-label" text-anchor="end">
            {{ s.name }}
          </text>
        </g>
        <line
          v-if="p.params.length"
          :x1="p.x + 10"
          :x2="p.x + W - 10"
          :y1="p.y + HEAD + p.portRows * ROW + 2"
          :y2="p.y + HEAD + p.portRows * ROW + 2"
          class="divider"
        />
        <g v-for="(path, i) in p.params" :key="path" class="knob" :class="{ changed: path === highlight }">
          <path :d="track(knob(p, i).cx, knob(p, i).cy)" class="knob-track" />
          <path
            :d="track(knob(p, i).cx, knob(p, i).cy)"
            class="knob-arc"
            :style="{ strokeDasharray: `${(level(path) * ARC).toFixed(2)} ${(ARC + 2).toFixed(2)}` }"
          />
          <circle :cx="knob(p, i).cx" :cy="knob(p, i).cy" :r="KR" class="knob-body" />
          <circle :cx="knob(p, i).cx" :cy="knob(p, i).cy" :r="KR - 3" class="knob-cap" />
          <line
            :x1="knob(p, i).cx"
            :y1="knob(p, i).cy - 2"
            :x2="knob(p, i).cx"
            :y2="knob(p, i).cy - KR + 2.5"
            class="knob-pointer"
            :style="{
              transform: `rotate(${(-135 + 270 * level(path)).toFixed(1)}deg)`,
              transformOrigin: `${knob(p, i).cx}px ${knob(p, i).cy}px`,
            }"
          />
          <text :x="knob(p, i).cx" :y="knob(p, i).cy + KR + 14" class="knob-label" text-anchor="middle">{{ path }}</text>
          <text :x="knob(p, i).cx" :y="knob(p, i).cy + KR + 25" class="knob-value" text-anchor="middle">{{ paramLabel(path) }}</text>
        </g>
      </g>
    </svg>
    <div v-if="tip" class="tooltip" :style="{ left: tip.x + 'px', top: tip.y + 'px' }">
      <div v-for="(l, i) in tip.lines" :key="i" :class="{ strong: i === 0 }">{{ l }}</div>
    </div>
  </div>
</template>

<style scoped>
.graph-root {
  position: relative;
  overflow-x: auto;
}
svg {
  display: block;
  max-width: 100%;
  height: auto;
  overflow: visible;
}
.cable path {
  fill: none;
  stroke: var(--c);
  stroke-linecap: round;
}
.cable .hit {
  stroke: transparent;
}
.cable-glow {
  stroke-width: 8;
  opacity: 0.14;
}
.cable-core {
  stroke-width: 2.4;
}
.r-Control .cable-core,
.r-Event .cable-core {
  animation: march 1.2s linear infinite;
}
.r-Event .cable-core {
  stroke-width: 3;
}
.cable .cable-pulse {
  stroke: #f4feff;
  stroke-width: 3.4;
  stroke-dasharray: 0.1 22;
  animation: flow 1.1s linear infinite;
  opacity: 0.75;
  filter: drop-shadow(0 0 3px var(--c));
}
@keyframes flow {
  to { stroke-dashoffset: -22.1; }
}
@keyframes march {
  to { stroke-dashoffset: -26; }
}
.body {
  fill: rgb(17 13 46 / 96%);
  stroke: color-mix(in srgb, var(--c) 40%, transparent);
  stroke-width: 1.2;
  transition: stroke 0.2s ease, filter 0.2s ease;
}
.node.lit .body {
  stroke: var(--c);
  stroke-width: 2;
  filter: drop-shadow(0 0 12px color-mix(in srgb, var(--c) 65%, transparent));
}
.strip {
  fill: var(--c);
  opacity: 0.85;
}
.led {
  fill: rgb(255 255 255 / 14%);
  transition: fill 0.2s ease;
}
.node.lit .led {
  fill: var(--pb-amber);
  filter: drop-shadow(0 0 5px var(--pb-amber));
}
.kind {
  fill: var(--c);
  font: 700 11px var(--font-mono);
  letter-spacing: 0.06em;
}
.nid {
  fill: rgb(220 215 255 / 45%);
  font: 400 10px var(--font-mono);
}
.jack {
  fill: #1b1640;
  stroke: rgb(255 255 255 / 22%);
}
.jack-hole {
  fill: #050312;
  stroke-width: 1.6;
}
.port-label {
  fill: var(--text-secondary);
  font: 400 10.5px var(--font-mono);
}
.divider {
  stroke: rgb(255 255 255 / 6%);
}
.knob-track {
  fill: none;
  stroke: rgb(255 255 255 / 11%);
  stroke-width: 3;
  stroke-linecap: round;
}
.knob-arc {
  fill: none;
  stroke: var(--c);
  stroke-width: 3;
  stroke-linecap: round;
  transition: stroke-dasharray 320ms ease;
}
.knob-body {
  fill: #1c1744;
  stroke: rgb(255 255 255 / 16%);
}
.knob-cap {
  fill: #29225e;
}
.knob-pointer {
  stroke: var(--c);
  stroke-width: 2.4;
  stroke-linecap: round;
  transition: transform 320ms ease;
}
.knob-label {
  fill: rgb(220 215 255 / 55%);
  font: 600 8.5px var(--font-mono);
}
.knob-value {
  fill: #fff;
  font: 600 9.5px var(--font-mono);
  font-variant-numeric: tabular-nums;
}
.knob.changed .knob-arc,
.knob.changed .knob-pointer {
  stroke: var(--pb-amber);
  filter: drop-shadow(0 0 4px var(--pb-amber));
}
.knob.changed .knob-value,
.knob.changed .knob-label {
  fill: var(--pb-amber);
}
.fb-label {
  fill: var(--c);
  font: 500 10.5px var(--font-mono);
  stroke: none;
}
.hit {
  cursor: help;
}
</style>
