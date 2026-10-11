import { defineConfig } from "vite-plus";

export default defineConfig({
  staged: {
    "*": "vp check --fix",
  },
  fmt: {
    // quran.com mirror payloads must stay byte-verbatim (id + byte-size identity;
    // docs/qurancom-mirror.md). The staged hook formats through THIS config —
    // never remove this ignore or mirror bytes drift on commit.
    ignorePatterns: ["web/static/qurancom/**"],
  },
  lint: {
    jsPlugins: [{ name: "vite-plus", specifier: "vite-plus/oxlint-plugin" }],
    rules: { "vite-plus/prefer-vite-plus-imports": "error" },
    options: { typeAware: true, typeCheck: true },
  },
});
