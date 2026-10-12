import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite-plus";

import { quranArtifacts } from "./vite-plugin-quran";

export default defineConfig({
  plugins: [quranArtifacts(), sveltekit()],
  resolve: {
    conditions: ["browser"],
  },
  test: {
    environment: "happy-dom",
    include: ["src/**/*.{test,spec}.ts"],
    // src/test-setup.ts drains bits-ui's 24ms body-scroll-lock cleanup at each
    // file's end so its timer never outlives the file's happy-dom environment.
    setupFiles: ["./src/test-setup.ts"],
  },
});
