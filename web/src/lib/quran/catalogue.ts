import { QURAN } from "$lib/config/site";
import {
  type SourceCatalogueEntry,
  type TranslationCatalogueEntry,
} from "$lib/data/quran-types";
import { decodeSourcesPayload } from "./wire";

const SOURCE_CATALOGUE_TTL_MS = 300_000;
let catalogueCache: { entries: SourceCatalogueEntry[]; expiresAt: number } | null = null;
let pendingCatalogue: Promise<SourceCatalogueEntry[]> | null = null;

async function fetchSourceCatalogue(): Promise<SourceCatalogueEntry[]> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), 3000);
  try {
    const res = await fetch(`${QURAN.apiBase}/sources`, {
      signal: ctrl.signal,
      headers: { accept: "application/json" },
    });
    if (!res.ok) return [];
    const entries = decodeSourcesPayload(await res.json());
    if (!entries) return [];
    return entries;
  } catch {
    return [];
  } finally {
    clearTimeout(timer);
  }
}

export async function resolveSourceCatalogue(
  signal?: AbortSignal,
): Promise<SourceCatalogueEntry[]> {
  if (!QURAN.apiBase) return [];
  const cached = catalogueCache;
  if (cached && cached.expiresAt > Date.now()) {
    return cached.entries;
  }
  if (signal?.aborted) return [];
  if (!pendingCatalogue) {
    pendingCatalogue = fetchSourceCatalogue()
      .then((entries) => {
        if (entries.length > 0) {
          catalogueCache = { entries, expiresAt: Date.now() + SOURCE_CATALOGUE_TTL_MS };
        }
        return entries;
      })
      .finally(() => {
        pendingCatalogue = null;
      });
  }
  return pendingCatalogue;
}

export function translationCatalogue(
  entries: readonly SourceCatalogueEntry[],
): TranslationCatalogueEntry[] {
  const out: TranslationCatalogueEntry[] = [];
  for (const entry of entries) {
    if (entry.kind === "translation") out.push(entry.entry);
  }
  return out;
}

export function findCatalogueEntry(
  entries: readonly SourceCatalogueEntry[],
  sourceId: string,
): SourceCatalogueEntry | undefined {
  return entries.find((entry) =>
    entry.kind === "translation" ? entry.entry.id === sourceId : entry.spec.id === sourceId,
  );
}
