<script setup lang="ts">
import { onMounted, ref, type Component } from "vue";
import mark from "./assets/mark.svg";
import CatalogueView from "./views/CatalogueView.vue";
import EvolutionView from "./views/EvolutionView.vue";
import GraphView from "./views/GraphView.vue";
import ResultView from "./views/ResultView.vue";

const tabs: { id: string; label: string; view: Component }[] = [
  { id: "catalogue", label: "Modules", view: CatalogueView },
  { id: "graph", label: "Patch graph", view: GraphView },
  { id: "evolution", label: "Fit evolution", view: EvolutionView },
  { id: "result", label: "End result", view: ResultView },
];

const active = ref("evolution");
const fromHash = () => {
  const id = window.location.hash.slice(1);
  if (tabs.some((t) => t.id === id)) active.value = id;
};
onMounted(() => {
  fromHash();
  window.addEventListener("hashchange", fromHash);
});
function go(id: string) {
  active.value = id;
  window.location.hash = id;
}
</script>

<template>
  <div class="shell">
    <header class="top">
      <div class="brand">
        <img :src="mark" alt="" width="34" height="34" class="mark" />
        <span class="wordmark">Patchback</span>
        <span class="product">/ studio</span>
        <span class="badge"><span class="dot" />prototype · replay of fixture data · no server</span>
      </div>
      <nav class="tabs" role="tablist">
        <button
          v-for="t in tabs"
          :key="t.id"
          role="tab"
          :aria-selected="active === t.id"
          :class="{ on: active === t.id }"
          @click="go(t.id)"
        >{{ t.label }}</button>
      </nav>
    </header>
    <main>
      <component :is="tabs.find((t) => t.id === active)?.view" />
    </main>
  </div>
</template>

<style scoped>
.shell {
  max-width: 1440px;
  margin: 0 auto;
  padding: 0 24px 40px;
}
.top {
  position: relative;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 16px 0;
  margin-bottom: 22px;
}
.top::after {
  position: absolute;
  inset: auto 0 0;
  height: 1px;
  background: linear-gradient(90deg, transparent, var(--pb-cyan) 18%, var(--pb-magenta) 55%, var(--pb-amber) 82%, transparent);
  opacity: 0.55;
  content: "";
}
.brand {
  display: flex;
  align-items: center;
  gap: 10px;
}
.mark {
  border-radius: 9px;
  box-shadow: 0 0 0.9rem rgb(255 79 216 / 28%);
}
.wordmark {
  color: #f6f3ff;
  font-weight: 800;
  font-size: 21px;
  letter-spacing: -0.04em;
}
.product {
  color: rgb(220 215 255 / 60%);
  font: 400 13px var(--font-mono);
  margin-left: -2px;
}
.badge {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  margin-left: 8px;
  padding: 3px 11px;
  border: 1px solid rgb(255 210 63 / 40%);
  border-radius: 999px;
  background: rgb(255 210 63 / 8%);
  color: var(--pb-amber);
  font: 500 11px/1.3 var(--font-mono);
  letter-spacing: 0.02em;
}
.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--pb-amber);
  animation: ping 2.2s ease-out infinite;
}
@keyframes ping {
  0% { box-shadow: 0 0 0 0 rgb(255 210 63 / 55%); }
  80%, 100% { box-shadow: 0 0 0 8px rgb(255 210 63 / 0%); }
}
.tabs {
  display: flex;
  gap: 4px;
  background: rgb(7 5 26 / 60%);
  border: 1px solid var(--line);
  border-radius: 999px;
  padding: 4px;
}
.tabs button {
  position: relative;
  background: none;
  border: 0;
  color: var(--text-secondary);
  padding: 7px 16px;
  border-radius: 999px;
  font: inherit;
  font-weight: 550;
  cursor: pointer;
}
.tabs button:hover {
  color: #fff;
}
.tabs button.on {
  background: rgb(255 255 255 / 6%);
  color: #fff;
  box-shadow: 0 0 0 1px rgb(255 255 255 / 10%) inset;
}
.tabs button.on::after {
  position: absolute;
  inset: auto 16px 3px;
  height: 2px;
  border-radius: 2px;
  background: var(--pb-signal);
  content: "";
}
</style>
