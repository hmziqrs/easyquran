import { browser } from "$app/environment";
import { isArabicSourceId, type QuranReaderSource } from "$lib/data/quran-types";
import { readRaw, removeRaw, writeRaw } from "$lib/storage";
import { quranWorker } from "./worker-client";

const VIEWS_KEY = "eq:reader-views";
const PREFETCH_PREFIX = "eq:tprefetch:";

export const VIEWS_BEFORE_PREFETCH = 2;

const settled = new Set<string>();
const deciding = new Set<string>();
const retried = new Set<string>();

function bumpViews(): number {
  const next = Number(readRaw("session", VIEWS_KEY) ?? 0) + 1;
  const views = Number.isSafeInteger(next) && next > 0 ? next : 1;
  writeRaw("session", VIEWS_KEY, String(views));
  return views;
}

interface NetworkInformation {
  readonly saveData?: boolean;
  readonly effectiveType?: string;
}

function connectionAllowsPrefetch(): boolean {
  const connection = (navigator as Navigator & { connection?: NetworkInformation }).connection;
  if (!connection) return true;
  if (connection.saveData) return false;
  return connection.effectiveType !== "2g" && connection.effectiveType !== "slow-2g";
}

function whenIdle(run: () => void): void {
  const idle = window.requestIdleCallback;
  if (typeof idle === "function") idle(() => run(), { timeout: 5_000 });
  else setTimeout(run, 500);
}

function markSettled(sourceId: string): void {
  settled.add(sourceId);
  writeRaw("session", `${PREFETCH_PREFIX}${sourceId}`, "1");
}

function allowRetry(sourceId: string): void {
  if (retried.has(sourceId)) return;
  retried.add(sourceId);
  settled.delete(sourceId);
  removeRaw("session", `${PREFETCH_PREFIX}${sourceId}`);
}

async function isCached(sourceId: QuranReaderSource): Promise<boolean> {
  await quranWorker.whenReady();
  return quranWorker.hasTranslation(sourceId);
}

if (browser) {
  quranWorker.onStatus?.((status, detail) => {
    if (status === "translation-fetch-failed" && detail) allowRetry(detail);
  });
}

export async function noteReaderView(
  sourceId: QuranReaderSource | null | undefined,
): Promise<void> {
  if (!browser) return;

  const views = bumpViews();

  if (!sourceId || isArabicSourceId(sourceId)) return;
  if (settled.has(sourceId) || deciding.has(sourceId)) return;
  if (readRaw("session", `${PREFETCH_PREFIX}${sourceId}`)) {
    settled.add(sourceId);
    return;
  }

  deciding.add(sourceId);
  try {
    if (await isCached(sourceId)) {
      markSettled(sourceId);
      return;
    }
    if (views < VIEWS_BEFORE_PREFETCH) return;
    if (!connectionAllowsPrefetch()) return;

    markSettled(sourceId);
    whenIdle(() => {
      void quranWorker.ensureTranslation(sourceId).catch(() => allowRetry(sourceId));
    });
  } catch {
  } finally {
    deciding.delete(sourceId);
  }
}

export async function noteTranslationChosen(sourceId: QuranReaderSource): Promise<void> {
  if (!browser) return;
  if (isArabicSourceId(sourceId)) return;
  if (settled.has(sourceId) || deciding.has(sourceId)) return;
  if (readRaw("session", `${PREFETCH_PREFIX}${sourceId}`)) {
    settled.add(sourceId);
    return;
  }
  if (!connectionAllowsPrefetch()) return;

  deciding.add(sourceId);
  try {
    if (await isCached(sourceId)) {
      markSettled(sourceId);
      return;
    }
    markSettled(sourceId);
    whenIdle(() => {
      void quranWorker.ensureTranslation(sourceId).catch(() => allowRetry(sourceId));
    });
  } catch {
  } finally {
    deciding.delete(sourceId);
  }
}

export function __resetEngagementState(): void {
  settled.clear();
  deciding.clear();
  retried.clear();
}
