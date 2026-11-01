<script setup lang="ts">
// The fit replay (DESIGN.md 16): a scrubber and play control over the frames of the loaded export.
// Play runs the whole trace in a chosen time, so a run of 400 evaluations plays as long as one of
// 20 (approval-record:wave-c1 Dc1-26); the frame table shows a window around the cursor.
import { computed, onBeforeUnmount, ref, watch } from "vue";
import LossChart from "../components/LossChart.vue";
import PatchGraph from "../components/PatchGraph.vue";
import RateLegend from "../components/RateLegend.vue";
import Trajectories from "../components/Trajectories.vue";
import { changedPath, retainedAt, samplesOf } from "../model";
import { loaded } from "../store";

const trace = computed(() => loaded.value.document.trace);
const frames = computed(() => trace.value.frames);
const last = computed(() => frames.value.length - 1);
const start = Number(new URLSearchParams(window.location.search).get("frame") ?? 0);
const index = ref(Number.isInteger(start) ? Math.max(0, Math.min(last.value, start)) : 0);
const playing = ref(false);
/** The time one play of the whole trace takes, in milliseconds. */
const duration = ref(20_000);
const showCandidate = ref(false);
let timer: number | undefined;

const MIN_TICK_MS = 16;
const tickMs = computed(() => Math.max(MIN_TICK_MS, duration.value / Math.max(1, frames.value.length)));
const stride = computed(() => Math.max(1, Math.round((frames.value.length * tickMs.value) / duration.value)));
const restarts = computed(() => (frames.value.at(-1)?.restart ?? 0) + 1);

const frame = computed(() => frames.value[index.value]!);
const kept = computed(() => retainedAt(frames.value, index.value));
const values = computed(() => samplesOf(showCandidate.value ? frame.value : (kept.value ?? frame.value)));
const changed = computed(() => changedPath(frames.value, index.value));
const graph = computed(() => loaded.value.document.graph);
const modules = computed(() => loaded.value.document.modules);

const WINDOW = 21;
const windowStart = computed(() =>
  Math.max(0, Math.min(frames.value.length - WINDOW, index.value - Math.floor(WINDOW / 2))),
);
const windowRows = computed(() =>
  frames.value.slice(windowStart.value, windowStart.value + WINDOW).map((f, k) => ({ f, i: windowStart.value + k })),
);

function tick() {
  if (index.value >= last.value) {
    playing.value = false;
    return;
  }
  index.value = Math.min(last.value, index.value + stride.value);
}

watch([playing, duration], ([on]) => {
  window.clearInterval(timer);
  if (on) {
    if (index.value >= last.value) index.value = 0;
    timer = window.setInterval(tick, tickMs.value);
  }
});
watch(loaded, () => {
  playing.value = false;
  index.value = 0;
});
onBeforeUnmount(() => window.clearInterval(timer));

function step(d: number) {
  if (last.value < 0) return;
  playing.value = false;
  index.value = Math.min(last.value, Math.max(0, index.value + d));
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
          <template v-if="frames.length">{{ frames.length }} evaluations over {{ restarts }} restart{{ restarts === 1 ? "" : "s" }}</template><template v-else>no evaluations</template>, source
          {{ trace.opened.source }}, ended {{ trace.closed.ended }}{{ trace.closed.stop_reason ? ` (${trace.closed.stop_reason})` : "" }}.
        </p>
      </div>
      <RateLegend />
    </header>
    <p class="fixture-note">From {{ loaded.label }}.</p>

    <template v-if="frames.length">
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
        play the run in
        <select v-model.number="duration">
          <option :value="60000">60 s</option>
          <option :value="20000">20 s</option>
          <option :value="6000">6 s</option>
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
        <span class="label">restart</span>
        <span class="value mono">{{ frame.restart }}</span>
      </div>
      <div class="stat">
        <span class="label">topology</span>
        <span class="value mono">{{ frame.topology_name }}</span>
      </div>
    </div>

    <div class="panel">
      <div class="panel-title">
        {{ showCandidate ? "Candidate patch" : "Retained patch" }} at evaluation {{ frame.evaluation }}
      </div>
      <PatchGraph :graph="graph" :modules="modules" :values="values" :highlight="changed" />
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
      <summary>
        Frame table: evaluations {{ windowStart + 1 }} to {{ windowStart + windowRows.length }} of {{ frames.length }}
      </summary>
      <table>
        <thead>
          <tr>
            <th>seq</th><th>eval</th><th>restart</th><th>outcome</th><th>candidate</th><th>best</th>
            <th>log_spectral</th><th>envelope</th><th>transient</th><th>band_energy</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="{ f, i } in windowRows"
            :key="f.frame_id"
            :class="{ current: i === index }"
            @click="playing = false; index = i"
          >
            <td class="num">{{ f.sequence }}</td>
            <td class="num">{{ f.evaluation }}</td>
            <td class="num">{{ f.restart }}</td>
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
    </template>
    <p v-else class="panel empty">The trace closed with no evaluations: there is no frame to replay.</p>
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
.stat:nth-child(5) { --c: var(--pb-purple); }
.stat:nth-child(6) { --c: var(--pb-cyan); }
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
