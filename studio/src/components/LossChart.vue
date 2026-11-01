<script setup lang="ts">
// The loss over a fit run's evaluations. Each series is one SVG path, whatever the number of
// frames (approval-record:wave-c1 Dc1-26): retained candidates, candidates that did not improve,
// rejected candidates (a cross on the axis, no loss), the best retained loss, and the topology
// changes, each split at the cursor into what has been replayed and what is still ahead.
import { computed, ref } from "vue";
import type { FrameAppended } from "../generated/types";
import { decimal, ticks } from "../model";

const props = defineProps<{ frames: FrameAppended[]; index: number }>();
const emit = defineEmits<{ seek: [index: number] }>();

const W = 720;
const H = 240;
const M = { l: 48, r: 92, t: 16, b: 34 };
const iw = W - M.l - M.r;
const ih = H - M.t - M.b;

const yMax = computed(() => {
  const m = Math.max(0, ...props.frames.map((f) => decimal(f.total_loss) ?? 0));
  return m > 0 ? Math.ceil(m * 10) / 10 : 1;
});
const n = computed(() => props.frames.length);
const x = (i: number) => M.l + (n.value <= 1 ? 0 : (i / (n.value - 1)) * iw);
const y = (v: number) => M.t + ih - (v / yMax.value) * ih;
const yTicks = computed(() => {
  const step = yMax.value > 0.6 ? 0.2 : 0.1;
  const out: number[] = [];
  for (let v = 0; v <= yMax.value + 1e-9; v += step) out.push(Math.round(v * 10) / 10);
  return out;
});
const xTicks = computed(() => ticks(n.value, 8));

/** A dot as a zero-length segment, drawn by a round line cap. */
const dot = (i: number, v: number) => `M ${x(i)} ${y(v)} h 0`;
const cross = (i: number) =>
  `M ${x(i) - 4} ${M.t + ih - 10} l 8 8 M ${x(i) + 4} ${M.t + ih - 10} l -8 8`;

interface Series {
  past: string;
  ahead: string;
}

const series = computed(() => {
  const kept: Series = { past: "", ahead: "" };
  const tried: Series = { past: "", ahead: "" };
  const rejected: Series = { past: "", ahead: "" };
  const topology: Series = { past: "", ahead: "" };
  props.frames.forEach((f, i) => {
    const side: keyof Series = i <= props.index ? "past" : "ahead";
    const total = decimal(f.total_loss);
    if (f.outcome === "Rejected") rejected[side] += cross(i);
    else if (total === undefined) return;
    else if (f.outcome === "NotImproved") tried[side] += dot(i, total);
    else kept[side] += dot(i, total);
    if (i > 0 && f.topology_id !== props.frames[i - 1]?.topology_id)
      topology[side] += `M ${x(i) - 0.5} ${M.t} V ${M.t + ih}`;
  });
  return { kept, tried, rejected, topology };
});

const bestPath = computed(() => {
  let d = "";
  props.frames.forEach((f, i) => {
    const b = decimal(f.best_loss);
    if (b === undefined) return;
    d += d ? ` H ${x(i)} V ${y(b)}` : `M ${x(i)} ${y(b)}`;
  });
  return d;
});
const lastBest = computed(() => decimal(props.frames[props.frames.length - 1]?.best_loss));
const current = computed(() => props.frames[props.index]);
const currentLoss = computed(() => decimal(current.value?.total_loss));

const hover = ref<number | null>(null);
const svg = ref<SVGSVGElement | null>(null);

function nearest(ev: MouseEvent): number {
  const r = svg.value?.getBoundingClientRect();
  if (!r || r.width === 0) return 0;
  const px = ((ev.clientX - r.left) / r.width) * W;
  const i = Math.round(((px - M.l) / iw) * (n.value - 1));
  return Math.min(n.value - 1, Math.max(0, i));
}

const hovered = computed(() => (hover.value === null ? undefined : props.frames[hover.value]));
const tipLeft = computed(() => (hover.value === null ? 0 : (x(hover.value) / W) * 100));
</script>

<template>
  <figure class="chart">
    <figcaption>
      <span class="title">Loss over evaluations</span>
      <span class="legend">
        <span class="key"><svg width="14" height="10"><path d="M7 5 h 0" class="kept" /></svg>retained candidate</span>
        <span class="key"><svg width="14" height="10"><path d="M7 5 h 0" class="tried" /></svg>not improved</span>
        <span class="key"><svg width="18" height="10"><path d="M1 5 H17" class="best" /></svg>best retained</span>
        <span class="key"><svg width="14" height="10"><path d="M3 1 L11 9 M11 1 L3 9" class="rej" /></svg>rejected (no loss)</span>
      </span>
    </figcaption>
    <div class="wrap">
      <svg
        ref="svg"
        :viewBox="`0 0 ${W} ${H}`"
        role="img"
        aria-label="Candidate loss and best retained loss per evaluation"
        @mousemove="hover = nearest($event)"
        @mouseleave="hover = null"
        @click="emit('seek', nearest($event))"
      >
        <g class="grid">
          <line v-for="t in yTicks" :key="t" :x1="M.l" :x2="M.l + iw" :y1="y(t)" :y2="y(t)" />
        </g>
        <g class="axis">
          <text v-for="t in yTicks" :key="t" :x="M.l - 8" :y="y(t) + 4" text-anchor="end">{{ t.toFixed(1) }}</text>
          <text v-for="i in xTicks" :key="'x' + i" :x="x(i)" :y="H - 12" text-anchor="middle">
            {{ frames[i]?.evaluation }}
          </text>
          <text :x="M.l + iw / 2" :y="H" text-anchor="middle" class="axis-title">evaluation</text>
        </g>
        <path :d="series.topology.past" class="topo" />
        <path :d="series.topology.ahead" class="topo future" />
        <line :x1="x(index)" :x2="x(index)" :y1="M.t" :y2="M.t + ih" class="cursor" />
        <path :d="bestPath" class="best" fill="none" />
        <text v-if="lastBest !== undefined" :x="x(n - 1) + 8" :y="y(lastBest) + 4" class="direct">
          best {{ lastBest.toFixed(4) }}
        </text>
        <path :d="series.tried.ahead" class="tried future" />
        <path :d="series.kept.ahead" class="kept future" />
        <path :d="series.rejected.ahead" class="rej future" />
        <path :d="series.tried.past" class="tried" />
        <path :d="series.kept.past" class="kept" />
        <path :d="series.rejected.past" class="rej" />
        <circle
          :cx="x(index)"
          :cy="y(currentLoss ?? 0)"
          r="6"
          class="now"
          :visibility="currentLoss === undefined ? 'hidden' : 'visible'"
        />
        <line v-if="hover !== null" :x1="x(hover)" :x2="x(hover)" :y1="M.t" :y2="M.t + ih" class="crosshair" />
      </svg>
      <div v-if="hovered" class="tooltip" :class="{ flip: tipLeft > 60 }" :style="{ left: tipLeft + '%', top: '8px' }">
        <div class="strong">evaluation {{ hovered.evaluation }} · restart {{ hovered.restart }} · {{ hovered.outcome }}</div>
        <div>candidate {{ hovered.total_loss ?? "none (rejected)" }}</div>
        <div>best {{ hovered.best_loss ?? "none" }}</div>
        <template v-if="hovered.components">
          <div v-for="(v, k) in hovered.components" :key="k" class="muted">{{ k }} {{ v }}</div>
        </template>
      </div>
    </div>
  </figure>
</template>


<style scoped>
.chart {
  margin: 0;
}
figcaption {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 18px;
  align-items: baseline;
  margin-bottom: 6px;
}
.title {
  font: 700 11px/1.2 var(--font-mono);
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--pb-cyan);
}
.legend {
  display: flex;
  gap: 14px;
  color: var(--text-secondary);
  font-size: 12px;
}
.key {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.wrap {
  position: relative;
}
.key svg {
  flex: none;
}
.wrap > svg {
  width: 100%;
  height: auto;
  display: block;
  cursor: pointer;
}
.grid line {
  stroke: var(--grid);
}
.axis text {
  fill: var(--text-muted);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}
.axis-title {
  fill: var(--text-muted);
}
.kept,
.tried {
  fill: none;
  stroke-linecap: round;
}
.kept {
  stroke: var(--series-candidate);
  stroke-width: 7;
}
.tried {
  stroke: color-mix(in srgb, var(--series-candidate) 45%, transparent);
  stroke-width: 5;
}
.now {
  fill: none;
  stroke: var(--pb-amber);
  stroke-width: 2;
}
.future {
  opacity: 0.25;
}
.best {
  stroke: var(--series-best);
  stroke-width: 2.4;
  filter: drop-shadow(0 0 4px rgb(255 79 216 / 60%));
  stroke-linejoin: round;
}
.rej {
  stroke: var(--text-muted);
  stroke-width: 2;
  stroke-linecap: round;
  fill: none;
}
.direct {
  fill: var(--text-primary);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}
.cursor {
  stroke: var(--pb-amber);
  stroke-width: 1;
}
.crosshair {
  stroke: var(--text-muted);
  stroke-dasharray: 3 3;
}
.topo {
  stroke: var(--text-secondary);
  stroke-dasharray: 1 3;
}
.tooltip {
  transform: translateX(12px);
}
.tooltip.flip {
  transform: translateX(calc(-100% - 12px));
}
</style>
