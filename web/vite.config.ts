import tailwindcss from "@tailwindcss/vite";
import { paraglideVitePlugin } from "@inlang/paraglide-js";
import adapter from "@sveltejs/adapter-node";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig, lazyPlugins } from "vite-plus";
import { quranArtifacts } from "./vite-plugin-quran";
import { paraglideOptions } from "./paraglide.config.js";

export default defineConfig({
  fmt: {},
  lint: {
    jsPlugins: [{ name: "vite-plus", specifier: "vite-plus/oxlint-plugin" }],
    rules: {
      "vite-plus/prefer-vite-plus-imports": "error",
      // Readability: a `?:` inside another `?:` is always a named function with early returns,
      // a lookup table, or a first-match helper instead. See AGENTS.MD.
      "no-nested-ternary": "error",
      "unicorn/no-nested-ternary": "error",
      // Correctness: a shadowed binding silently rebinds the outer name on any later edit.
      "no-shadow": "error",
      // Correctness: IndexedDB's `tx.error` is nullable, so a raw reject drops the reason.
      "prefer-promise-reject-errors": "error",
      // Correctness: `arr.map(fn)` hands fn the index as a second argument.
      "unicorn/no-array-callback-reference": "error",
      "unicorn/prefer-number-properties": "error",
    },
    options: { typeAware: true, typeCheck: true },
  },
  plugins: lazyPlugins(() => [
    quranArtifacts(),
    tailwindcss(),
    paraglideVitePlugin(paraglideOptions),
    sveltekit({
      version: {
        pollInterval: 5 * 60_000,
      },
      compilerOptions: {
        runes: ({ filename }) =>
          filename.split(/[/\\]/).includes("node_modules") ? undefined : true,
      },
      serviceWorker: {
        register: false,
        files: (file) =>
          !/\.DS_Store/.test(file) &&
          !file.startsWith("quran-meta/") &&
          !/^(_headers|_redirects|robots\.txt|og\.png)$/.test(file),
      },
      adapter: adapter({
        precompress: true,
      }),
      prerender: {
        handleMissingId: "fail",
      },
    }),
  ]),
});
