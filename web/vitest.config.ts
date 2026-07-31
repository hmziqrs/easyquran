import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite-plus";
import { quranData } from "./vite-plugin-quran";

/**
 * Standalone test config. Vitest loads this in preference to vite.config.ts,
 * so it must be self-contained: quranData() resolves the `quran-meta:data` /
 * `quran-virtual` modules that $lib/data/* imports, and sveltekit() provides
 * the $app/* and $lib aliases and compiles .svelte / .svelte.ts (runes).
 *
 * resolve.conditions: ["browser"] makes svelte resolve its CLIENT build, so
 * `svelte/reactivity` (SvelteMap/SvelteSet) and the effect scheduler behave as
 * they do in the browser rather than the SSR shims (where SvelteMap === Map).
 *
 * happy-dom drives Svelte's effect scheduler (effects don't fire in bare node).
 */
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
