import tailwindcss from "@tailwindcss/vite";
import adapter from "@sveltejs/adapter-static";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig, lazyPlugins } from "vite-plus";
import { quranArtifacts } from "./vite-plugin-quran";

export default defineConfig({
  fmt: {},
  lint: {
    jsPlugins: [{ name: "vite-plus", specifier: "vite-plus/oxlint-plugin" }],
    rules: { "vite-plus/prefer-vite-plus-imports": "error" },
    options: { typeAware: true, typeCheck: true },
  },
  plugins: lazyPlugins(() => [
    quranArtifacts(),
    tailwindcss(),
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
        pages: "build",
        assets: "build",
        fallback: "404.html",
        precompress: true,
        strict: true,
      }),
    }),
  ]),
});
