import { groupBy } from "es-toolkit";

import type { PaletteEntry, PaletteGroup, PaletteQuery, PaletteSource } from "./types";

export const DEFAULT_SOURCE_LIMIT = 7;

const sources = new Map<string, PaletteSource>();
const groups = new Map<string, PaletteGroup>();

/**
 * Adds a domain to the palette. Registering the same id again replaces it, so a
 * source can be swapped in tests or hot-reloaded without duplicate rows.
 */
export function registerPaletteSource(source: PaletteSource): void {
  for (const group of source.groups) {
    const existing = groups.get(group.id);
    if (existing && existing.order !== group.order) {
      throw new Error(
        `[palette] group "${group.id}" registered with conflicting order ${existing.order} vs ${group.order}`,
      );
    }
    groups.set(group.id, group);
  }
  sources.set(source.id, source);
}

export function unregisterPaletteSource(id: string): void {
  sources.delete(id);
}

export function paletteSources(): PaletteSource[] {
  return [...sources.values()];
}

export function paletteGroups(): PaletteGroup[] {
  return [...groups.values()].sort((a, b) => a.order - b.order || a.id.localeCompare(b.id));
}

const queryFor = (query: PaletteQuery, source: PaletteSource): PaletteQuery => ({
  ...query,
  limit: source.limit ?? DEFAULT_SOURCE_LIMIT,
});

const isEnabled = (source: PaletteSource, query: PaletteQuery): boolean =>
  source.enabled?.(queryFor(query, source)) ?? true;

/** Sources with in-memory `entries`, eligible for this query. */
export function syncSources(query: PaletteQuery): PaletteSource[] {
  return paletteSources().filter((source) => source.entries && isEnabled(source, query));
}

/** Sources with async `search`, eligible for this query. */
export function asyncSources(query: PaletteQuery): PaletteSource[] {
  return paletteSources().filter((source) => source.search && isEnabled(source, query));
}

/** Runs every eligible in-memory source, capping each at its own limit. */
export function collectSyncEntries(query: PaletteQuery): PaletteEntry[] {
  const out: PaletteEntry[] = [];
  for (const source of syncSources(query)) {
    const scoped = queryFor(query, source);
    try {
      out.push(...source.entries!(scoped).slice(0, scoped.limit));
    } catch (error) {
      console.warn(`[palette] source "${source.id}" failed:`, error);
    }
  }
  return out;
}

/**
 * Runs every eligible async source concurrently. A source that rejects is
 * reported and skipped rather than sinking the whole result set.
 */
export async function collectAsyncEntries(
  query: PaletteQuery,
  signal: AbortSignal,
): Promise<{ entries: PaletteEntry[]; failed: string[] }> {
  const eligible = asyncSources(query);
  const settled = await Promise.allSettled(
    eligible.map((source) => {
      const scoped = queryFor(query, source);
      return source.search!(scoped, signal).then((entries) => entries.slice(0, scoped.limit));
    }),
  );

  const entries: PaletteEntry[] = [];
  const failed: string[] = [];
  settled.forEach((result, i) => {
    const source = eligible[i]!;
    if (result.status === "fulfilled") entries.push(...result.value);
    else {
      failed.push(source.id);
      console.warn(`[palette] source "${source.id}" search failed:`, result.reason);
    }
  });
  return { entries, failed };
}

/**
 * Drops later entries pointing where an earlier one already goes — a typed `2`
 * yields both a jump-to hit and a catalogue row for Al-Baqarah. Entries must
 * arrive in display order so the earlier, more specific one survives.
 */
export function dedupeEntries(entries: readonly PaletteEntry[]): PaletteEntry[] {
  const seenTargets = new Set<string>();
  const seenIds = new Set<string>();
  const out: PaletteEntry[] = [];
  for (const entry of entries) {
    // Ids are also enforced: two entries sharing one would render as two rows
    // with the same `Command` value, and keyboard selection would pick either.
    if (seenIds.has(entry.id)) continue;
    const target = entry.dedupeKey ?? entry.id;
    if (seenTargets.has(target)) continue;
    seenIds.add(entry.id);
    seenTargets.add(target);
    out.push(entry);
  }
  return out;
}

export interface PaletteSection {
  group: PaletteGroup;
  entries: PaletteEntry[];
}

const topScore = (entries: readonly PaletteEntry[]): number =>
  entries.reduce((best, entry) => Math.max(best, entry.score), 0);

/**
 * Buckets entries into registered groups, dropping empties. Sections are
 * ordered by their strongest match first and fall back to `group.order` — so a
 * group whose best hit is exact rises above one matched only fuzzily, while an
 * empty query (every score 0) lays the groups out in their designed order.
 */
export function sectionsFor(entries: readonly PaletteEntry[]): PaletteSection[] {
  const byGroup = groupBy(entries, (entry) => entry.groupId);
  return paletteGroups()
    .flatMap((group) => {
      const grouped = byGroup[group.id];
      return grouped ? [{ group, entries: grouped }] : [];
    })
    .sort((a, b) => topScore(b.entries) - topScore(a.entries) || a.group.order - b.group.order);
}
