import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  server: { host: "127.0.0.1", port: 3481, strictPort: true },
  preview: { host: "127.0.0.1", port: 3481, strictPort: true },
  test: { environment: "happy-dom", include: ["src/**/*.test.ts"] },
});
