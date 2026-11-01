<script setup lang="ts">
import { computed, ref } from "vue";
import PatchGraph from "../components/PatchGraph.vue";
import RateLegend from "../components/RateLegend.vue";
import { feedbackExample } from "../model";
import { loaded } from "../store";

const choices = computed(() => [
  {
    label: loaded.value.document.topology,
    note: `The retained patch of the fit run in ${loaded.value.label}.`,
    graph: loaded.value.document.graph,
    modules: loaded.value.document.modules,
  },
  { label: "delayed feedback", ...feedbackExample },
]);
const wanted = Number(new URLSearchParams(window.location.search).get("graph") ?? 0);
const selected = ref(wanted === 1 ? 1 : 0);
const current = computed(() => choices.value[selected.value] ?? choices.value[0]!);
const kindOf = (id: number) => current.value.graph.nodes.find((n) => n.node_id === id)?.kind ?? "?";
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
            v-for="(g, i) in choices"
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
    <p class="fixture-note">{{ current.note }}</p>
    <div class="panel">
      <PatchGraph :graph="current.graph" :modules="current.modules" />
    </div>
    <details class="panel table-view">
      <summary>Edge table</summary>
      <table>
        <thead>
          <tr><th>from</th><th>to</th><th>rate</th><th>delayed feedback</th></tr>
        </thead>
        <tbody>
          <tr v-for="(e, i) in current.graph.edges" :key="i">
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
