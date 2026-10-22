<script setup lang="ts">
import { RATES, RATE_COLOR, RATE_DASH } from "../model";
</script>

<template>
  <div class="rate-legend" aria-label="Edge colour and dash by port rate">
    <span v-for="r in RATES" :key="r" class="key" :style="{ color: RATE_COLOR[r] }">
      <svg width="30" height="10">
        <path d="M3 5 H27" :stroke="RATE_COLOR[r]" :stroke-dasharray="RATE_DASH[r]" stroke-width="2.4" stroke-linecap="round" class="glow" />
        <path d="M3 5 H27" :stroke="RATE_COLOR[r]" :stroke-dasharray="RATE_DASH[r]" stroke-width="2.4" stroke-linecap="round" />
      </svg>
      <span class="label">{{ r }} rate</span>
    </span>
  </div>
</template>

<style scoped>
.rate-legend {
  display: flex;
  gap: 16px;
  font: 500 11px var(--font-mono);
  letter-spacing: 0.04em;
}
.key {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.label {
  color: var(--text-secondary);
}
.glow {
  filter: blur(2px);
  opacity: 0.6;
}
</style>
