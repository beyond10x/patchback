<script setup lang="ts">
// The trajectory of each parameter over a fit run: its retained value as a step line, and the
// values candidates tried as dots. Each series is one SVG path per parameter, whatever the number
// of frames (approval-record:wave-c1 Dc1-26), built in one pass over the frames.
import { computed } from "vue";
import type { FrameAppended } from "../generated/types";
import { contractByPath, formatPhysical, retainedIndex, samplesOf } from "../model";

const props = defineProps<{
  frames: FrameAppended[];
  paths: string[];
  index: number;
  highlight?: string;
}>();

const W = 200;
const H = 70;
const M = { l: 4, r: 4, t: 6, b: 6 };
const x = (i: number) => M.l + (i / Math.max(1, props.frames.length - 1)) * (W - M.l - M.r);
const y = (v: number) => M.t + (1 - v) * (H - M.t - M.b);

const kept = computed(() => retainedIndex(props.frames));
const values = computed(() => props.frames.map((f) => samplesOf(f)));

const series = computed(() =>
  props.paths.map((path) => {
    let d = "";
    let full = "";
    let tried = "";
    props.frames.forEach((_, i) => {
      const k = kept.value[i]!;
      if (k >= 0) {
        const v = values.value[k]![path] ?? 0;
        const seg = ` H ${x(i)} V ${y(v)}`;
        const start = `M ${x(i)} ${y(v)}`;
        full += full ? seg : start;
        if (i <= props.index) d += d ? seg : start;
      }
      if (i > props.index) return;
      const v = values.value[i]![path] ?? 0;
      const base = i > 0 ? kept.value[i - 1]! : -1;
      if (base >= 0 && values.value[base]![path] !== v) tried += `M ${x(i)} ${y(v)} h 0`;
    });
    return { path, d, full, tried };
  }),
);

const current = computed(() => {
  const k = kept.value[props.index] ?? -1;
  return k >= 0 ? values.value[k]! : {};
});

function label(path: string) {
  const c = contractByPath.get(path);
  const v = current.value[path] ?? 0;
  return c ? formatPhysical(c, v) : v.toFixed(3);
}
</script>

<template>
  <figure class="traj">
    <figcaption>
      <span class="title">Parameter trajectories</span>
      <span class="legend">
        <span class="key"><svg width="18" height="10"><path d="M1 5 H17" class="kept" /></svg>retained value (normalized 0–1)</span>
        <span class="key"><svg width="12" height="10"><path d="M6 5 h 0" class="tried" /></svg>value a candidate tried</span>
      </span>
    </figcaption>
    <div class="grid">
      <div v-for="s in series" :key="s.path" class="cell" :class="{ lit: s.path === highlight }">
        <div class="head">
          <span>{{ s.path }}</span>
          <span class="value">{{ label(s.path) }}</span>
        </div>
        <svg :viewBox="`0 0 ${W} ${H}`" role="img" :aria-label="`Trajectory of ${s.path}`">
          <line :x1="M.l" :x2="W - M.r" :y1="y(0.5)" :y2="y(0.5)" class="mid" />
          <line :x1="x(index)" :x2="x(index)" :y1="M.t" :y2="H - M.b" class="cursor" />
          <path :d="s.full" class="kept ghost" fill="none" />
          <path :d="s.d" class="kept" fill="none" />
          <path :d="s.tried" class="tried" />
        </svg>
      </div>
    </div>
  </figure>
</template>


<style scoped>
.traj {
  margin: 0;
}
figcaption {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 18px;
  align-items: baseline;
  margin-bottom: 8px;
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
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 10px;
}
.cell {
  background: var(--surface-2);
  border: 1px solid var(--line);
  border-radius: 6px;
  padding: 6px 8px 4px;
}
.cell.lit {
  border-color: var(--text-secondary);
}
.head {
  display: flex;
  justify-content: space-between;
  font-size: 11.5px;
  color: var(--text-secondary);
}
.value {
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}
.key svg {
  flex: none;
}
.cell svg {
  width: 100%;
  height: auto;
  display: block;
}
.kept {
  stroke: var(--series-best);
  stroke-width: 2;
  stroke-linejoin: round;
}
.kept.ghost {
  opacity: 0.22;
}
.tried {
  fill: none;
  stroke: var(--series-candidate);
  stroke-width: 5;
  stroke-linecap: round;
}
.mid {
  stroke: var(--grid);
}
.cursor {
  stroke: var(--text-muted);
}
</style>
