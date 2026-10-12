import { defineConfig } from "vite-plus";

export default defineConfig({
  staged: {
    "*": "vp check --fix",
  },
  fmt: {
    // quran.com mirror payloads must stay byte-verbatim (id + byte-size identity;
    // documentation/qurancom-mirror.md). documentation/ is human-owned and untouchable —
    // the formatter must never rewrite it (AGENTS.md Hard rules). The staged hook formats
    // through THIS config — never remove these ignores or bytes drift on commit.
    ignorePatterns: ["web/static/qurancom/**", "documentation/**"],
  },
  lint: {
    jsPlugins: [{ name: "vite-plus", specifier: "vite-plus/oxlint-plugin" }],
    rules: { "vite-plus/prefer-vite-plus-imports": "error" },
    options: { typeAware: true, typeCheck: true },
  },
});
