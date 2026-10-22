<script setup lang="ts">
import { computed, ref } from "vue";

const props = defineProps<{
  reference: { hz: number; db: number }[];
  render: { hz: number; db: number }[];
  fftSize: number;
}>();

const W = 640;
const H = 260;
const M = { l: 48, r: 70, t: 14, b: 36 };
const iw = W - M.l - M.r;
const ih = H - M.t - M.b;
const F0 = 100;
const F1 = 16000;
const D0 = -90;
const D1 = 10;

const x = (hz: number) => M.l + (Math.log(hz / F0) / Math.log(F1 / F0)) * iw;
const y = (db: number) => M.t + ((D1 - Math.max(D0, Math.min(D1, db))) / (D1 - D0)) * ih;
const line = (pts: { hz: number; db: number }[]) =>
  pts.map((p, i) => `${i ? "L" : "M"} ${x(p.hz).toFixed(1)} ${y(p.db).toFixed(1)}`).join(" ");

const xTicks = [100, 300, 1000, 3000, 10000];
const yTicks = [0, -20, -40, -60, -80];
const fmtHz = (hz: number) => (hz >= 1000 ? `${hz / 1000}k` : `${hz}`);

const refPath = computed(() => line(props.reference));
const renPath = computed(() => line(props.render));
const lastRef = computed(() => props.reference[props.reference.length - 1]);
const lastRen = computed(() => props.render[props.render.length - 1]);

const labelY = computed(() => {
  const a = lastRef.value ? y(lastRef.value.db) : 0;
  const b = lastRen.value ? y(lastRen.value.db) : 0;
  if (Math.abs(a - b) >= 14) return { ref: a + 4, ren: b + 4 };
  const mid = (a + b) / 2;
  return a > b ? { ref: mid + 11, ren: mid - 3 } : { ref: mid - 3, ren: mid + 11 };
});

const hover = ref<number | null>(null);
const svg = ref<SVGSVGElement | null>(null);
function move(ev: MouseEvent) {
  const r = svg.value?.getBoundingClientRect();
  if (!r) return;
  const px = ((ev.clientX - r.left) / r.width) * W;
  let best = 0;
  let dist = Infinity;
  props.reference.forEach((p, i) => {
    const d = Math.abs(x(p.hz) - px);
    if (d < dist) {
      dist = d;
      best = i;
    }
  });
  hover.value = best;
}
const hRef = computed(() => (hover.value === null ? undefined : props.reference[hover.value]));
const hRen = computed(() => (hover.value === null ? undefined : props.render[hover.value]));
</script>

<template>
  <figure class="chart">
    <figcaption>
      <span class="title">Harmonic spectrum, FFT {{ fftSize }}</span>
      <span class="legend">
        <span class="key"><svg width="18" height="10"><path d="M1 5 H17" class="ref" /></svg>reference</span>
        <span class="key"><svg width="18" height="10"><path d="M1 5 H17" class="ren" /></svg>render of retained patch</span>
      </span>
    </figcaption>
    <div class="wrap">
      <svg
        ref="svg"
        :viewBox="`0 0 ${W} ${H}`"
        role="img"
        aria-label="Reference and render spectra in decibels over log frequency"
        @mousemove="move"
        @mouseleave="hover = null"
      >
        <g class="grid">
          <line v-for="t in yTicks" :key="'y' + t" :x1="M.l" :x2="M.l + iw" :y1="y(t)" :y2="y(t)" />
          <line v-for="t in xTicks" :key="'x' + t" :x1="x(t)" :x2="x(t)" :y1="M.t" :y2="M.t + ih" />
        </g>
        <g class="axis">
          <text v-for="t in yTicks" :key="'yl' + t" :x="M.l - 8" :y="y(t) + 4" text-anchor="end">{{ t }}</text>
          <text v-for="t in xTicks" :key="'xl' + t" :x="x(t)" :y="H - 16" text-anchor="middle">{{ fmtHz(t) }}</text>
          <text :x="M.l + iw / 2" :y="H - 2" text-anchor="middle">Hz (log) · dB</text>
        </g>
        <path :d="refPath" class="ref" fill="none" />
        <path :d="renPath" class="ren" fill="none" />
        <text v-if="lastRef" :x="x(lastRef.hz) + 8" :y="labelY.ref" class="direct">reference</text>
        <text v-if="lastRen" :x="x(lastRen.hz) + 8" :y="labelY.ren" class="direct">render</text>
        <g v-if="hRef && hRen">
          <line :x1="x(hRef.hz)" :x2="x(hRef.hz)" :y1="M.t" :y2="M.t + ih" class="crosshair" />
          <circle :cx="x(hRef.hz)" :cy="y(hRef.db)" r="4.5" class="ref-dot" />
          <circle :cx="x(hRen.hz)" :cy="y(hRen.db)" r="4.5" class="ren-dot" />
        </g>
      </svg>
      <div
        v-if="hRef && hRen"
        class="tooltip"
        :class="{ flip: x(hRef.hz) > W * 0.6 }"
        :style="{ left: (x(hRef.hz) / W) * 100 + '%', top: '8px' }"
      >
        <div class="strong">{{ hRef.hz.toFixed(0) }} Hz</div>
        <div>reference {{ hRef.db.toFixed(1) }} dB</div>
        <div>render {{ hRen.db.toFixed(1) }} dB</div>
        <div class="muted">difference {{ (hRen.db - hRef.db).toFixed(1) }} dB</div>
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
}
.grid line {
  stroke: var(--grid);
}
.axis text {
  fill: var(--text-muted);
  font-size: 11px;
}
.ref {
  stroke: var(--series-reference);
  stroke-width: 2;
  stroke-linejoin: round;
}
.ren {
  stroke: var(--series-best);
  stroke-width: 2;
  stroke-linejoin: round;
}
.ref-dot {
  fill: var(--series-reference);
  stroke: var(--surface-1);
  stroke-width: 2;
}
.ren-dot {
  fill: var(--series-best);
  stroke: var(--surface-1);
  stroke-width: 2;
}
.direct {
  fill: var(--text-primary);
  font-size: 11px;
}
.crosshair {
  stroke: var(--text-muted);
  stroke-dasharray: 3 3;
}
.tooltip {
  transform: translateX(12px);
}
.tooltip.flip {
  transform: translateX(calc(-100% - 12px));
}
</style>
