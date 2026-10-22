<script setup lang="ts">
import { computed, ref } from "vue";
import type { FrameAppended } from "../generated/types";
import { decimal } from "../model";

const props = defineProps<{ frames: FrameAppended[]; index: number }>();
const emit = defineEmits<{ seek: [index: number] }>();

const W = 720;
const H = 240;
const M = { l: 48, r: 92, t: 16, b: 34 };
const iw = W - M.l - M.r;
const ih = H - M.t - M.b;

const yMax = computed(() => {
  const m = Math.max(...props.frames.map((f) => decimal(f.total_loss) ?? 0));
  return Math.ceil(m * 10) / 10;
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

const bestPath = computed(() => {
  let d = "";
  props.frames.forEach((f, i) => {
    const b = decimal(f.best_loss);
    if (b === undefined) return;
    d += d ? ` H ${x(i)} V ${y(b)}` : `M ${x(i)} ${y(b)}`;
  });
  return d;
});
const lastBest = computed(() => {
  const f = props.frames[props.frames.length - 1];
  return decimal(f?.best_loss);
});

const topologyChanges = computed(() =>
  props.frames.flatMap((f, i) => (i > 0 && f.topology_id !== props.frames[i - 1]?.topology_id ? [i] : [])),
);

const hover = ref<number | null>(null);
const svg = ref<SVGSVGElement | null>(null);

function nearest(ev: MouseEvent): number {
  const r = svg.value?.getBoundingClientRect();
  if (!r) return 0;
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
        <span class="key"><svg width="14" height="10"><circle cx="7" cy="5" r="4" class="cand" /></svg>candidate loss</span>
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
          <text
            v-for="(f, i) in frames"
            v-show="i % 2 === 0 || i === frames.length - 1"
            :key="f.frame_id"
            :x="x(i)"
            :y="H - 12"
            text-anchor="middle"
          >{{ f.evaluation }}</text>
          <text :x="M.l + iw / 2" :y="H" text-anchor="middle" class="axis-title">evaluation</text>
        </g>
        <line
          v-for="i in topologyChanges"
          :key="'t' + i"
          :x1="x(i) - 6"
          :x2="x(i) - 6"
          :y1="M.t"
          :y2="M.t + ih"
          class="topo"
        />
        <line :x1="x(index)" :x2="x(index)" :y1="M.t" :y2="M.t + ih" class="cursor" />
        <path :d="bestPath" class="best" fill="none" />
        <text v-if="lastBest !== undefined" :x="x(n - 1) + 8" :y="y(lastBest) + 4" class="direct">
          best {{ lastBest.toFixed(4) }}
        </text>
        <g v-for="(f, i) in frames" :key="f.frame_id">
          <circle
            v-if="f.total_loss !== undefined"
            :cx="x(i)"
            :cy="y(Number(f.total_loss))"
            :r="i === index ? 6 : 4.5"
            class="cand"
            :class="{ hollow: f.outcome === 'NotImproved', future: i > index }"
          />
          <path
            v-else
            :d="`M ${x(i) - 4} ${M.t + ih - 10} L ${x(i) + 4} ${M.t + ih - 2} M ${x(i) + 4} ${M.t + ih - 10} L ${x(i) - 4} ${M.t + ih - 2}`"
            class="rej"
            :class="{ future: i > index }"
          />
        </g>
        <line v-if="hover !== null" :x1="x(hover)" :x2="x(hover)" :y1="M.t" :y2="M.t + ih" class="crosshair" />
      </svg>
      <div v-if="hovered" class="tooltip" :class="{ flip: tipLeft > 60 }" :style="{ left: tipLeft + '%', top: '8px' }">
        <div class="strong">evaluation {{ hovered.evaluation }} · {{ hovered.outcome }}</div>
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
.cand {
  fill: var(--series-candidate);
  stroke: var(--surface-1);
  stroke-width: 2;
  transition: r 200ms ease;
}
.cand.hollow {
  fill: var(--surface-1);
  stroke: var(--series-candidate);
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
