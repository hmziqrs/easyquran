import { paraglideVitePlugin } from "@inlang/paraglide-js";
import adapter from "@sveltejs/adapter-bun";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig, lazyPlugins } from "vite-plus";

import { paraglideOptions } from "./paraglide.config.js";
import { previewSecurityHeaders } from "./vite-plugin-preview-headers";
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
  // adapter-bun externalizes package.json `dependencies` in the SSR build (they must
  // exist in node_modules at runtime). The web image deliberately ships NO node_modules:
  // everything the server bundle imports must be bundled in. gen-offline-pack.ts (postbuild)
  // fails the build if a bare import ever leaks through this list.
  ssr: {
    noExternal: [
      "clsx",
      "es-toolkit",
      "firebase",
      "simple-icons",
      "tailwind-merge",
      "tailwind-variants",
      "valibot",
    ],
  },
  optimizeDeps: {
    // `@sqlite.org/sqlite-wasm` is imported only by the OPFS worker, which is loaded through
    // `new Worker(new URL(...))` — a graph Vite's cold dependency scan never crawls. Left out,
    // the first offline read (API down, or a translation served from OPFS) discovers it
    // mid-session: Vite re-optimizes, the browser hash changes, and every module the live page
    // already holds under the old `?v=` hash goes stale. Anything rendering in that window can
    // throw inside the half-swapped Svelte runtime ("Cannot read properties of undefined
    // (reading 'call')") before the reload lands.
    include: ["@sqlite.org/sqlite-wasm"],
  },
  server: {
    // Dev mirror of the production edge: Traefik routes Host(DOMAIN) + PathPrefix(/api)
    // to api:8888 and strips /api (docker-compose.yml, strip-api middleware). With
    // PUBLIC_API_BASE_URL unset the client falls back to the same-origin "/api", so dev
    // needs the same strip — and same-origin keeps session cookies + CSRF behaving as
    // they do in production. The API must allow http://localhost:5173 as an origin:
    // set CONSUMER_PORT=5173 in .env (non-prod dev origin defaults are additive).
    proxy: {
      "/api": {
        target: "http://127.0.0.1:8888",
        rewrite: (path) => path.replace(/^\/api/, ""),
      },
    },
  },
  plugins: lazyPlugins(() => [
    previewSecurityHeaders(),
    quranArtifacts(),
    tailwindcss(),
    paraglideVitePlugin(paraglideOptions),
    sveltekit({
      version: {
        // Service-worker registration owns proactive update detection. SvelteKit still performs
        // its navigation-error version check, while no-SW browsers check from UpdateStore.
        pollInterval: 0,
      },
      compilerOptions: {
        runes: ({ filename }) =>
          filename.split(/[/\\]/).includes("node_modules") ? undefined : true,
      },
      serviceWorker: {
        // Kit 3 reduced this option to `register` (+ registration options) — there is
        // no `files` filter (verified against @sveltejs/kit 3.0.1 types) and the
        // `$app/manifest` asset walk is unfiltered. The kit 2 exclusions therefore
        // live in src/service-worker.ts's PRECACHE_FILES filter: dotfiles (the
        // server's isDotfile refuses them, so a precache fetch 404s and SW install
        // dies), `_headers`/`_redirects`/`robots.txt`/`og.png`, and the `quran-meta/`
        // prefix (quran-data.json is precached deliberately by the PRECACHE line).
        register: false,
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
