<script setup lang="ts">
import { computed } from "vue";
import PatchGraph from "../components/PatchGraph.vue";
import RateLegend from "../components/RateLegend.vue";
import SpectrumChart from "../components/SpectrumChart.vue";
import { contractByPath, formatPhysical, graphs, result, retainedAt, samplesOf, saw, trace } from "../model";

const frames = trace.frames;
const best = retainedAt(frames, frames.length - 1);
const recovered = best ? samplesOf(best) : {};
const reference = samplesOf({ parameters: result.reference_parameters });
const graph = graphs[0]!.fixture.event;
const fft = result.event.spectra.find((s) => s.source === "Render")?.fft_size ?? 8192;

const rows = computed(() =>
  trace.opened.parameter_paths.map((path) => {
    const c = contractByPath.get(path);
    const r = reference[path] ?? 0;
    const v = recovered[path] ?? 0;
    return {
      path,
      ref: c ? formatPhysical(c, r) : r.toFixed(3),
      rec: c ? formatPhysical(c, v) : v.toFixed(3),
      delta: v - r,
    };
  }),
);
const refSpectrum = saw(reference);
const renSpectrum = saw(recovered);
</script>

<template>
  <section>
    <header class="view-head">
      <div>
        <h2>End result</h2>
        <p class="sub">
          Retained patch <span class="mono">{{ result.event.best_patch_id?.slice(-4) ?? "none" }}</span>
          after {{ frames.length }} evaluations, best loss <span class="mono">{{ best?.best_loss ?? "none" }}</span>.
        </p>
      </div>
      <RateLegend />
    </header>
    <p class="fixture-note">{{ result.note }}</p>
    <div class="panel">
      <div class="panel-title">Recovered patch</div>
      <PatchGraph :graph="graph" :values="recovered" />
    </div>
    <div class="cols">
        <div class="panel">
          <SpectrumChart :reference="refSpectrum" :render="renSpectrum" :fft-size="fft" />
        </div>
        <div class="panel">
          <div class="panel-title">Parameters: reference against recovered</div>
          <table>
            <thead>
              <tr><th>path</th><th>reference</th><th>recovered</th><th>Δ normalized</th></tr>
            </thead>
            <tbody>
              <tr v-for="r in rows" :key="r.path">
                <td class="mono">{{ r.path }}</td>
                <td class="num">{{ r.ref }}</td>
                <td class="num">{{ r.rec }}</td>
                <td class="num delta">
                  <span class="bar-wrap">
                    <span
                      class="bar"
                      :style="{
                        width: Math.min(50, Math.abs(r.delta) * 100) + '%',
                        left: r.delta < 0 ? 50 - Math.min(50, Math.abs(r.delta) * 100) + '%' : '50%',
                      }"
                    />
                  </span>
                  {{ r.delta >= 0 ? "+" : "" }}{{ r.delta.toFixed(3) }}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
    </div>
  </section>
</template>

<style scoped>
.cols {
  display: grid;
  grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr);
  gap: 14px;
}
@media (max-width: 1100px) {
  .cols {
    grid-template-columns: 1fr;
  }
}
.stack {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.delta {
  display: flex;
  align-items: center;
  gap: 8px;
  justify-content: flex-end;
}
.bar-wrap {
  position: relative;
  width: 80px;
  height: 8px;
  background: var(--surface-2);
  border-radius: 4px;
}
.bar-wrap::after {
  content: "";
  position: absolute;
  left: 50%;
  top: -2px;
  bottom: -2px;
  width: 1px;
  background: var(--text-muted);
}
.bar {
  position: absolute;
  top: 0;
  bottom: 0;
  background: var(--pb-magenta);
  box-shadow: 0 0 8px rgb(255 79 216 / 60%);
  border-radius: 4px;
}
</style>
