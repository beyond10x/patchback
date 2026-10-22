<script setup lang="ts">
import type { ModuleSpec } from "../generated/types";
import RateLegend from "../components/RateLegend.vue";
import { MODULE_COLOR, RATE_COLOR, RATE_DASH, catalogue, formatPhysical } from "../model";

const modules = catalogue.event.modules;
const inputs = (m: ModuleSpec) => m.ports.filter((p) => p.direction === "Input");
const outputs = (m: ModuleSpec) => m.ports.filter((p) => p.direction === "Output");
const rows = (m: ModuleSpec) => Math.max(inputs(m).length, outputs(m).length, 1);
</script>

<template>
  <section>
    <header class="view-head">
      <div>
        <h2>Module catalogue</h2>
        <p class="sub">Every atomic module kind with its ports, rates and parameter contracts.</p>
      </div>
      <RateLegend />
    </header>
    <p class="fixture-note">{{ catalogue.note }}</p>
    <div class="cards">
      <article v-for="m in modules" :key="m.kind" class="card" :style="{ '--c': MODULE_COLOR[m.kind] }">
        <svg :viewBox="`0 0 240 ${44 + rows(m) * 20}`" class="glyph" role="img" :aria-label="`${m.kind} ports`">
          <rect x="40" y="4" width="160" :height="36 + rows(m) * 20" rx="12" class="box" />
          <rect x="41" y="5" width="158" height="3" rx="1.5" class="strip" />
          <text x="52" y="26" class="kind">{{ m.kind }}</text>
          <circle cx="187" cy="22" r="3.5" class="led" />
          <g v-for="(p, i) in inputs(m)" :key="'i' + p.name">
            <path :d="`M 4 ${48 + i * 20} H 40`" :stroke="RATE_COLOR[p.rate]" :stroke-dasharray="RATE_DASH[p.rate] || undefined" class="lead" />
            <circle cx="40" :cy="48 + i * 20" r="6" class="jack" />
            <circle cx="40" :cy="48 + i * 20" r="2.8" class="jack-hole" :stroke="RATE_COLOR[p.rate]" />
            <text x="52" :y="52 + i * 20" class="port-label">{{ p.name }}</text>
          </g>
          <g v-for="(p, i) in outputs(m)" :key="'o' + p.name">
            <path :d="`M 200 ${48 + i * 20} H 236`" :stroke="RATE_COLOR[p.rate]" :stroke-dasharray="RATE_DASH[p.rate] || undefined" class="lead" />
            <circle cx="200" :cy="48 + i * 20" r="6" class="jack" />
            <circle cx="200" :cy="48 + i * 20" r="2.8" class="jack-hole" :stroke="RATE_COLOR[p.rate]" />
            <text x="188" :y="52 + i * 20" text-anchor="end" class="port-label">{{ p.name }}</text>
          </g>
        </svg>
        <div class="meta">
          <span v-if="m.family" class="chip">family {{ m.family }}</span>
          <span v-else class="chip">MVP voice</span>
        </div>
        <table class="ports">
          <tbody>
            <tr v-for="p in m.ports" :key="p.name">
              <td>{{ p.direction === "Input" ? "in" : "out" }}</td>
              <td class="mono">{{ p.name }}</td>
              <td :style="{ color: RATE_COLOR[p.rate] }">{{ p.rate }}</td>
              <td class="muted">{{ p.meaning }}</td>
            </tr>
          </tbody>
        </table>
        <table v-if="m.parameters.length" class="params">
          <thead>
            <tr><th>#</th><th>path</th><th>mapping</th><th>range</th><th>default</th></tr>
          </thead>
          <tbody>
            <tr v-for="c in m.parameters" :key="c.path">
              <td class="num">{{ c.index }}</td>
              <td class="mono path">{{ c.path }}</td>
              <td>{{ c.mapping }}</td>
              <td class="num">{{ formatPhysical(c, 0) }} – {{ formatPhysical(c, 1) }}</td>
              <td class="num">{{ formatPhysical(c, Number(c.default_normalized)) }}</td>
            </tr>
          </tbody>
        </table>
        <p v-else class="muted small">No learned parameters.</p>
      </article>
    </div>
  </section>
</template>

<style scoped>
.cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(100%, 420px), 1fr));
  gap: 16px;
}
.card {
  position: relative;
  overflow-x: auto;
  background:
    radial-gradient(22rem 9rem at 50% 0%, color-mix(in srgb, var(--c) 9%, transparent), transparent 70%),
    rgb(17 13 46 / 82%);
  border: 1px solid color-mix(in srgb, var(--c) 22%, var(--line));
  border-radius: 16px;
  padding: 14px;
  transition: border-color 0.2s ease, box-shadow 0.2s ease;
}
.card:hover {
  border-color: color-mix(in srgb, var(--c) 60%, transparent);
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--c) 25%, transparent), 0 16px 40px -20px color-mix(in srgb, var(--c) 45%, transparent);
}
.glyph {
  width: 100%;
  max-width: 280px;
  display: block;
  margin: 0 auto 10px;
}
.box {
  fill: rgb(17 13 46 / 96%);
  stroke: color-mix(in srgb, var(--c) 45%, transparent);
  stroke-width: 1.2;
}
.strip {
  fill: var(--c);
  opacity: 0.85;
}
.led {
  fill: rgb(255 255 255 / 14%);
}
.kind {
  fill: var(--c);
  font: 700 11px var(--font-mono);
  letter-spacing: 0.06em;
}
.lead {
  fill: none;
  stroke-width: 2.4;
  stroke-linecap: round;
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
.meta {
  margin-bottom: 8px;
}
table {
  width: 100%;
  border-collapse: collapse;
  font-size: 11.5px;
  margin-top: 6px;
}
th {
  text-align: left;
  padding: 3px 4px;
}
td {
  padding: 3px 4px;
  vertical-align: top;
}
.path {
  color: var(--c);
  font-size: 10.5px;
}
.small {
  font-size: 12px;
}
.params .num {
  white-space: nowrap;
}
</style>
