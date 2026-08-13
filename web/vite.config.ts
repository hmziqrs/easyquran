import { paraglideVitePlugin } from "@inlang/paraglide-js";
import adapter from "@sveltejs/adapter-node";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig, lazyPlugins } from "vite-plus";

import { paraglideOptions } from "./paraglide.config.js";
import { quranArtifacts } from "./vite-plugin-quran";

export default defineConfig({
  fmt: {
    // Width: Oxfmt default — 100, not Prettier's 80 (TS is longer under annotations).
    printWidth: 100,
    tabWidth: 2,
    semi: true,
    singleQuote: false,
    trailingComma: "all",
    // Determinism: stable import + Tailwind class order across edits.
    sortImports: {},
    sortTailwindcss: {},
  },
  lint: {
    jsPlugins: [
      { name: "vite-plus", specifier: "vite-plus/oxlint-plugin" },
      { name: "anti-slop", specifier: "./.cache/anti-slop/index.ts" },
    ],
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
      // Correctness: an unawaited promise loses its rejection and races whatever runs next.
      "no-floating-promises": "error",
      "no-misused-promises": "error",
      "anti-slop/no-chained-type-assertions": "error",
      "anti-slop/no-conditional-empty-object-spread": "error",
      "anti-slop/no-known-value-widening": "error",
      "anti-slop/no-module-mocking": "error",
      "anti-slop/no-object-parameters": "error",
      "anti-slop/no-reflect-apply": "error",
      "anti-slop/no-reflect-get": "error",
      "anti-slop/no-runtime-typeof": "error",
      "anti-slop/no-shape-in-symbol-names": "error",
      "anti-slop/no-unknown-parameters": "error",
      "anti-slop/no-unknown-returns": "error",
      "anti-slop/no-unknown-type-aliases": "error",
      "anti-slop/no-unsafe-dictionary-type": "error",
      "anti-slop/no-widen-then-assert": "error",
      "anti-slop/require-safety-comment-for-type-assertion": "error",
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
