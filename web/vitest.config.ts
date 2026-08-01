import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite-plus";
import { quranData } from "./vite-plugin-quran";

export default defineConfig({
  plugins: [quranData(), sveltekit()],
  resolve: {
    conditions: ["browser"],
  },
  test: {
    environment: "happy-dom",
    include: ["src/**/*.{test,spec}.ts"],
  },
});
