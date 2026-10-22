<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import LossChart from "../components/LossChart.vue";
import PatchGraph from "../components/PatchGraph.vue";
import RateLegend from "../components/RateLegend.vue";
import Trajectories from "../components/Trajectories.vue";
import { changedPath, graphs, retainedAt, samplesOf, trace } from "../model";

const frames = trace.frames;
const last = frames.length - 1;
const start = Number(new URLSearchParams(window.location.search).get("frame") ?? 0);
const index = ref(Number.isInteger(start) ? Math.min(last, Math.max(0, start)) : 0);
const playing = ref(false);
const speed = ref(700);
const showCandidate = ref(false);
let timer: number | undefined;

const frame = computed(() => frames[index.value]!);
const kept = computed(() => retainedAt(frames, index.value));
const values = computed(() => samplesOf(showCandidate.value ? frame.value : (kept.value ?? frame.value)));
const changed = computed(() => changedPath(frames, index.value));
const graph = graphs[0]!.fixture.event;

function tick() {
  if (index.value >= last) {
    playing.value = false;
    return;
  }
  index.value += 1;
}

watch([playing, speed], ([on]) => {
  window.clearInterval(timer);
  if (on) {
    if (index.value >= last) index.value = 0;
    timer = window.setInterval(tick, speed.value);
  }
});
onBeforeUnmount(() => window.clearInterval(timer));

function step(d: number) {
  playing.value = false;
  index.value = Math.min(last, Math.max(0, index.value + d));
}

function onKey(ev: KeyboardEvent) {
  if (ev.key === "ArrowRight") step(1);
  else if (ev.key === "ArrowLeft") step(-1);
  else if (ev.key === " ") {
    ev.preventDefault();
    playing.value = !playing.value;
  } else return;
}

const OUTCOME_LABEL: Record<string, string> = {
  FirstRetained: "first retained",
  Improved: "improved, retained",
  NotImproved: "not improved",
  Rejected: "rejected, no loss",
};
</script>

<template>
  <section tabindex="0" class="evo" @keydown="onKey">
    <header class="view-head">
      <div>
        <h2>Fit evolution</h2>
        <p class="sub">
          Replay of fit run <span class="mono">{{ trace.opened.fit_run_id.slice(-4) }}</span>,
          {{ frames.length }} evaluations, source {{ trace.opened.source }}, ended {{ trace.closed.ended }}.
        </p>
      </div>
      <RateLegend />
    </header>
    <p class="fixture-note">{{ trace.note }}</p>

    <div class="transport panel">
      <button class="btn primary" :aria-label="playing ? 'Pause' : 'Play'" @click="playing = !playing">
        {{ playing ? "❚❚ Pause" : "▶ Play" }}
      </button>
      <button class="btn ghost" aria-label="Previous evaluation" @click="step(-1)">◀</button>
      <button class="btn ghost" aria-label="Next evaluation" @click="step(1)">▶</button>
      <input
        v-model.number="index"
        type="range"
        min="0"
        :max="last"
        step="1"
        class="scrubber"
        aria-label="Evaluation"
        @input="playing = false"
      />
      <span class="pos mono">{{ frame.evaluation }} / {{ frames[last]!.evaluation }}</span>
      <label class="small">
        speed
        <select v-model.number="speed">
          <option :value="1200">slow</option>
          <option :value="700">normal</option>
          <option :value="300">fast</option>
        </select>
      </label>
      <label class="small check">
        <input v-model="showCandidate" type="checkbox" /> show the candidate, not the retained patch
      </label>
    </div>

    <div class="stats">
      <div class="stat">
        <span class="label">outcome</span>
        <span class="value" :class="'o-' + frame.outcome">{{ OUTCOME_LABEL[frame.outcome] }}</span>
      </div>
      <div class="stat">
        <span class="label">candidate loss</span>
        <span class="value mono">{{ frame.total_loss ?? "none" }}</span>
      </div>
      <div class="stat">
        <span class="label">best retained loss</span>
        <span class="value mono">{{ frame.best_loss ?? "none" }}</span>
      </div>
      <div class="stat">
        <span class="label">changed parameter</span>
        <span class="value mono">{{ changed ?? "none" }}</span>
      </div>
      <div class="stat">
        <span class="label">topology</span>
        <span class="value mono">{{ frame.topology_id.slice(-4) }}</span>
      </div>
    </div>

    <div class="panel">
      <div class="panel-title">
        {{ showCandidate ? "Candidate patch" : "Retained patch" }} at evaluation {{ frame.evaluation }}
      </div>
      <PatchGraph :graph="graph" :values="values" :highlight="changed" />
    </div>

    <div class="cols">
      <div class="panel">
        <LossChart :frames="frames" :index="index" @seek="(i) => { playing = false; index = i; }" />
      </div>
      <div class="panel">
        <Trajectories :frames="frames" :paths="trace.opened.parameter_paths" :index="index" :highlight="changed" />
      </div>
    </div>

    <details class="panel table-view">
      <summary>Frame table</summary>
      <table>
        <thead>
          <tr>
            <th>seq</th><th>eval</th><th>outcome</th><th>candidate</th><th>best</th>
            <th>log_spectral</th><th>envelope</th><th>transient</th><th>band_energy</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="(f, i) in frames"
            :key="f.frame_id"
            :class="{ current: i === index }"
            @click="playing = false; index = i"
          >
            <td class="num">{{ f.sequence }}</td>
            <td class="num">{{ f.evaluation }}</td>
            <td>{{ OUTCOME_LABEL[f.outcome] }}</td>
            <td class="num">{{ f.total_loss ?? "–" }}</td>
            <td class="num">{{ f.best_loss ?? "–" }}</td>
            <td class="num">{{ f.components?.log_spectral ?? "–" }}</td>
            <td class="num">{{ f.components?.envelope ?? "–" }}</td>
            <td class="num">{{ f.components?.transient ?? "–" }}</td>
            <td class="num">{{ f.components?.band_energy ?? "–" }}</td>
          </tr>
        </tbody>
      </table>
    </details>
  </section>
</template>

<style scoped>
.evo:focus {
  outline: none;
}
.transport {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  margin-bottom: 14px;
}
.scrubber {
  flex: 1 1 260px;
  accent-color: var(--pb-magenta);
}
.pos {
  min-width: 56px;
  color: var(--pb-amber);
}
.check {
  display: inline-flex;
  gap: 6px;
  align-items: center;
}
.stats {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 10px;
  margin-bottom: 14px;
}
.stat {
  --c: var(--pb-cyan);
  position: relative;
  overflow: hidden;
  background:
    radial-gradient(14rem 5rem at 0% 0%, color-mix(in srgb, var(--c) 9%, transparent), transparent 70%),
    rgb(17 13 46 / 82%);
  border: 1px solid var(--line);
  border-radius: 14px;
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.stat::before {
  position: absolute;
  inset: 0 0 auto;
  height: 2px;
  background: linear-gradient(90deg, var(--c), transparent);
  content: "";
}
.stat:nth-child(1) { --c: var(--pb-lime); }
.stat:nth-child(2) { --c: var(--series-candidate); }
.stat:nth-child(3) { --c: var(--series-best); }
.stat:nth-child(4) { --c: var(--pb-amber); }
.stat:nth-child(5) { --c: var(--pb-cyan); }
.stat .label {
  font: 600 10px var(--font-mono);
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: rgb(220 215 255 / 55%);
}
.stat .value {
  font-size: 17px;
  font-weight: 700;
}
.stat:nth-child(n + 2) .value {
  color: var(--c);
  text-shadow: 0 0 14px color-mix(in srgb, var(--c) 45%, transparent);
}
.o-Rejected::before {
  content: "✕ ";
  color: var(--text-muted);
}
.o-Improved::before,
.o-FirstRetained::before {
  content: "● ";
  color: var(--series-best);
}
.o-NotImproved::before {
  content: "○ ";
  color: var(--series-candidate);
}
.cols {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 14px;
  margin-bottom: 14px;
}
@media (max-width: 1100px) {
  .cols {
    grid-template-columns: 1fr;
  }
}
tr.current td {
  background: rgb(255 79 216 / 12%);
}
tbody tr {
  cursor: pointer;
}
</style>
