<script setup lang="ts">
import { computed, ref } from "vue";
import PatchGraph from "../components/PatchGraph.vue";
import RateLegend from "../components/RateLegend.vue";
import { graphs, moduleByKind } from "../model";

const wanted = Number(new URLSearchParams(window.location.search).get("graph") ?? 0);
const selected = ref(graphs[wanted] ? wanted : 0);
const current = computed(() => graphs[selected.value] ?? graphs[0]!);
const kindOf = (id: number) => current.value.fixture.event.nodes.find((n) => n.node_id === id)?.kind ?? "?";
const defaults = computed(() =>
  Object.fromEntries(
    current.value.fixture.event.nodes.flatMap((n) =>
      (moduleByKind.get(n.kind)?.parameters ?? []).map((p) => [p.path, Number(p.default_normalized)] as const),
    ),
  ),
);
</script>

<template>
  <section>
    <header class="view-head">
      <div>
        <h2>Patch graph</h2>
        <p class="sub">Nodes by rank, edges coloured and dashed by port rate, delayed feedback routed under the graph.</p>
      </div>
      <div class="controls">
        <div class="segmented" role="tablist">
          <button
            v-for="(g, i) in graphs"
            :key="g.label"
            :class="{ on: i === selected }"
            role="tab"
            :aria-selected="i === selected"
            @click="selected = i"
          >{{ g.label }}</button>
        </div>
        <RateLegend />
      </div>
    </header>
    <p class="fixture-note">{{ current.fixture.note }}</p>
    <div class="panel">
      <PatchGraph :graph="current.fixture.event" :values="defaults" />
    </div>
    <details class="panel table-view">
      <summary>Edge table</summary>
      <table>
        <thead>
          <tr><th>from</th><th>to</th><th>rate</th><th>delayed feedback</th></tr>
        </thead>
        <tbody>
          <tr v-for="(e, i) in current.fixture.event.edges" :key="i">
            <td class="mono">{{ kindOf(e.from_node) }}#{{ e.from_node }}.{{ e.from_port }}</td>
            <td class="mono">{{ kindOf(e.to_node) }}#{{ e.to_node }}.{{ e.to_port }}</td>
            <td>{{ e.rate }}</td>
            <td>{{ e.delayed_feedback ? "yes" : "no" }}</td>
          </tr>
        </tbody>
      </table>
    </details>
  </section>
</template>
