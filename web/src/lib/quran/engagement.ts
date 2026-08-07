import { browser } from "$app/environment";
import { isArabicSourceId, type QuranReaderSource } from "$lib/data/quran-types";
import { quranWorker } from "./worker-client";

const VIEWS_KEY = "eq:reader-views";
const PREFETCH_PREFIX = "eq:tprefetch:";

export const VIEWS_BEFORE_PREFETCH = 2;

/** Sources this tab is done with: already cached, or prefetch already fired. */
const settled = new Set<string>();
/** Sources with an in-progress decision, so overlapping views cannot stack requests. */
const deciding = new Set<string>();
/** Sources already given their one retry, so a failing download cannot loop. */
const retried = new Set<string>();

function readStorage(key: string): string | null {
  try {
    return sessionStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeStorage(key: string, value: string): void {
  try {
    sessionStorage.setItem(key, value);
  } catch {}
}

function clearStorage(key: string): void {
  try {
    sessionStorage.removeItem(key);
  } catch {}
}

function bumpViews(): number {
  const next = Number(readStorage(VIEWS_KEY) ?? 0) + 1;
  const views = Number.isSafeInteger(next) && next > 0 ? next : 1;
  writeStorage(VIEWS_KEY, String(views));
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
  writeStorage(`${PREFETCH_PREFIX}${sourceId}`, "1");
}

/** Allow exactly one more attempt after a failed download, then stay settled. */
function allowRetry(sourceId: string): void {
  if (retried.has(sourceId)) return;
  retried.add(sourceId);
  settled.delete(sourceId);
  clearStorage(`${PREFETCH_PREFIX}${sourceId}`);
}

async function isCached(sourceId: QuranReaderSource): Promise<boolean> {
  await quranWorker.whenReady();
  return quranWorker.hasTranslation(sourceId);
}

/**
 * Records one reader view and, once the visitor looks engaged, kicks off a
 * background download of the translation database.
 *
 * Safe to call on every mount and every navigation: a source is evaluated at
 * most once per tab, and not at all once it is known to be cached.
 */
export async function noteReaderView(
  sourceId: QuranReaderSource | null | undefined,
): Promise<void> {
  if (!browser) return;

  const views = bumpViews();

  if (!sourceId || isArabicSourceId(sourceId)) return;
  if (settled.has(sourceId) || deciding.has(sourceId)) return;
  if (readStorage(`${PREFETCH_PREFIX}${sourceId}`)) {
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
      // The worker acknowledges this before the download finishes, so a
      // resolved promise means "accepted", not "cached". Only an outright
      // rejection is worth a retry.
      void quranWorker.ensureTranslation(sourceId).catch(() => allowRetry(sourceId));
    });
  } catch {
    // Worker not up yet or request failed — leave unsettled, retry on next view.
  } finally {
    deciding.delete(sourceId);
  }
}

/**
 * Picking a translation by hand is an explicit signal; skip the view counter.
 */
export async function noteTranslationChosen(sourceId: QuranReaderSource): Promise<void> {
  if (!browser) return;
  if (isArabicSourceId(sourceId)) return;
  if (settled.has(sourceId) || deciding.has(sourceId)) return;
  if (readStorage(`${PREFETCH_PREFIX}${sourceId}`)) {
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

/** Test-only reset of per-tab state. */
export function __resetEngagementState(): void {
  settled.clear();
  deciding.clear();
  retried.clear();
}
