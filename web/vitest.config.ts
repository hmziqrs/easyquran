import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vitest/config";

import { quranArtifacts } from "./vite-plugin-quran";

export default defineConfig({
  plugins: [quranArtifacts(), sveltekit()],
  resolve: {
    conditions: ["browser"],
  },
  test: {
    environment: "happy-dom",
    include: ["src/**/*.{test,spec}.ts"],
  },
});
