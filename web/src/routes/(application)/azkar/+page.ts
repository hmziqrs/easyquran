import { groupBy } from "es-toolkit";

import {
  SNIPPETS,
  SNIPPET_KIND_ORDER,
  type Snippet,
  type SnippetKind,
} from "#lib/data/snippets.js";

// Arabic-only prerender (Feature 7.2): static editorial index — reference
// data only, no Quran text is copied into the markup (D06).
export const prerender = true;

const BY_KIND = groupBy(SNIPPETS, (snippet: Snippet) => snippet.kind);

/** Index sections in display order; empty kinds are skipped by the page. */
export function snippetGroups(): { kind: SnippetKind; snippets: Snippet[] }[] {
  return SNIPPET_KIND_ORDER.map((kind) => ({ kind, snippets: BY_KIND[kind] ?? [] }));
}

export const load = () => ({
  groups: snippetGroups(),
});
