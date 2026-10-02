import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite-plus";

import { quranArtifacts } from "./vite-plugin-quran";

export default defineConfig({
  plugins: [quranArtifacts(), sveltekit()],
  resolve: {
    conditions: ["browser"],
  },
  test: {
    // Vitest v4 compatibility: preserve mock call history.
    // Remove after tests no longer rely on calls from setup or earlier tests.
    // https://viteplus.dev/guide/vitest-v5#remove-unneeded-compatibility-settings
    // https://vitest.dev/guide/migration/#clearmocks-is-enabled-by-default
    clearMocks: false,
    environment: "happy-dom",
    include: ["src/**/*.{test,spec}.ts"],
  },
});
