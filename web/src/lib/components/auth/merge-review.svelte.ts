import { bookmarksDomainFor } from "#lib/bookmarks/domain.js";
import {
  newBookmarkEntityId,
  parseVerseKey,
  type Bookmark,
  type BookmarksMutation,
} from "#lib/bookmarks/schema.js";
import { bookmarks, legacyMigratedKey, type SyncEngineLike } from "#lib/bookmarks/store.svelte.js";
import type { VerseKey } from "#lib/data/quran.js";
import { sessionsDomainFor } from "#lib/sessions/domain.svelte.js";
import {
  LOCAL_DEFAULT_SESSION_ID,
  LOCAL_DEFAULT_SESSION_NAME,
  accountDefaultSessionId,
  decodeSessionsEnvelope,
  defaultSessionFor,
  type ReadingSession,
  type SessionMutation,
  type SessionReaderState,
} from "#lib/sessions/schema.svelte.js";
import { readJSON, readRaw, writeRaw } from "#lib/storage/index.js";
import { decodeReader } from "#lib/stores/reader-persistence.svelte.js";
import { syncEngine } from "#lib/sync/index.js";

/**
 * Merge on login (§9.4) — the non-silent replacement for the old `#migrateLegacy`
 * upload: legacy blob bookmarks, the `local` owner's reading sessions, and
 * retained unowned outbox entries (EC-48) are never enqueued before the user's
 * explicit choice, and the per-account markers land only after every enqueue
 * resolves (EC-11).
 */

/** Mirror of reader-persistence's module-private key — the review reads the durable blob. */
const READER_BLOB_KEY = "easyquran.reader";
/** The `local` owner's sessions key (Feature 4.2) — the merge's other input. */
const SESSIONS_LOCAL_KEY = "easyquran.sessions.local";
/** No `bookmarkSavedAt` ⇒ epoch: an account copy of the same verse wins LWW, new verses still add (EC-10). */
const EPOCH_ISO = "1970-01-01T00:00:00.000Z";

function stampOf(ms: number | undefined): string {
  return ms !== undefined && Number.isFinite(ms) && ms > 0 ? new Date(ms).toISOString() : EPOCH_ISO;
}

/** The `local` envelope shape the watermark reads — what `#localEnvelope` snapshots. */
type LocalEnvelope = {
  sessions: readonly ReadingSession[];
  state: Readonly<Record<string, SessionReaderState>>;
};

/** Max mtime of the envelope's mergeable content: session rows' `updatedAt` plus each state's newest progress stamp. */
function envelopeWatermark(envelope: LocalEnvelope): number {
  let max = 0;
  for (const session of envelope.sessions) {
    const ms = Date.parse(session.updatedAt);
    if (Number.isFinite(ms) && ms > max) max = ms;
  }
  for (const state of Object.values(envelope.state)) {
    for (const entry of Object.values(state.progress)) {
      if (entry.ts > max) max = entry.ts;
    }
  }
  return max;
}

export function mergeReviewedKey(userId: number): string {
  return `eq.merge.reviewed.${userId}`;
}

export function mergeDeferredKey(userId: number): string {
  return `eq.merge.deferred.${userId}`;
}

/**
 * Per-account watermark of the `local` sessions envelope at the last completed
 * merge (§9.4 detection (b)): the max of every session row's `updatedAt` and
 * each state's newest progress stamp. Unchanged local sessions stay suppressed;
 * NEW local data re-arms detection instead of stranding behind a one-way flag.
 */
export function sessionsMigratedKey(userId: number): string {
  return `eq.merge.sessions-migrated.${userId}`;
}

export interface MergeBookmarkRow {
  readonly key: VerseKey;
  readonly surah: number;
  readonly ayah: number;
  /** RFC3339 stamp the upsert carries: `bookmarkSavedAt` when present, else the epoch. */
  readonly savedAt: string;
}

export type MergeOutcome = "new" | "device-wins" | "account-wins";

/** One reviewable reading-session item (§9.4 step 2). */
export interface MergeSessionRow {
  /** Local session id — the re-key target is resolved at merge time for the default row. */
  readonly id: string;
  readonly name: string;
  /** The synthetic row for the device-local default session: position + progress only (EC-22). */
  readonly isDefault: boolean;
}

/** Bookmarks-store surface the review drives; the real store satisfies this structurally. */
export interface MergeReviewDeps {
  readonly bookmarks: {
    unownedPending(): number;
    applyLegacyUnowned(ownerId: number): Promise<number>;
    discardLegacyUnowned(): Promise<void>;
    readonly bookmarks: readonly Bookmark[];
  };
  readonly engine: Pick<SyncEngineLike, "enqueue" | "flush">;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- err is a caught throw of unknown provenance; this formatter is the boundary that narrows it to a displayable message string.
function errorMessage(err: unknown): string {
  return err instanceof Error && err.message ? err.message : String(err);
}

export class MergeReview {
  readonly #deps: MergeReviewDeps;
  #open = $state(false);
  #rows = $state.raw<readonly MergeBookmarkRow[]>([]);
  #sessionRows = $state.raw<readonly MergeSessionRow[]>([]);
  #unowned = $state(0);
  #result = $state<{ merged: number; skipped: number } | null>(null);
  #merging = $state(false);
  #error = $state<string | null>(null);
  #userId: number | null = null;
  #resolved = false;
  readonly #enqueuedKeys = new Set<string>();
  readonly #enqueuedSessions = new Set<string>();
  /** The `local` envelope read at detection — merge uploads from this snapshot. */
  #localEnvelope: LocalEnvelope | null = null;

  constructor(deps: MergeReviewDeps) {
    this.#deps = deps;
  }

  get open(): boolean {
    return this.#open;
  }
  get rows(): readonly MergeBookmarkRow[] {
    return this.#rows;
  }
  get sessionRows(): readonly MergeSessionRow[] {
    return this.#sessionRows;
  }
  get unowned(): number {
    return this.#unowned;
  }
  get result(): { merged: number; skipped: number } | null {
    return this.#result;
  }
  get merging(): boolean {
    return this.#merging;
  }
  get error(): string | null {
    return this.#error;
  }

  /** LWW preview against the current account view — row labels and result counts both use it (EC-13). */
  outcomeFor(row: MergeBookmarkRow): MergeOutcome {
    const rival = this.#deps.bookmarks.bookmarks.find(
      (bookmark) => bookmark.surah === row.surah && bookmark.ayah === row.ayah,
    );
    if (rival === undefined) return "new";
    return row.savedAt > rival.updatedAt ? "device-wins" : "account-wins";
  }

  /**
   * Detection after auth → authenticated: mergeable local data exists if legacy
   * blob bookmarks are unmarked for this account, the `local` owner's sessions
   * (Feature 4.2) changed since the last completed merge, or retained unowned
   * entries are queued. This account's own pending mutations are never a
   * trigger. Only the user's explicit markers gate detection (deferred /
   * reviewed); a completed merge re-arms for NEW local data.
   */
  evaluate(userId: number): void {
    if (this.#open && this.#userId === userId) return;
    if (readRaw("local", mergeReviewedKey(userId)) === "1") return;
    if (readRaw("session", mergeDeferredKey(userId)) === "1") return;
    const rows = this.#legacyRows(userId);
    const sessionRows = this.#localSessionRows(userId);
    const unowned = this.#deps.bookmarks.unownedPending();
    if (rows.length === 0 && sessionRows.length === 0 && unowned === 0) return;
    this.#userId = userId;
    this.#rows = rows;
    this.#sessionRows = sessionRows;
    this.#unowned = unowned;
    this.#result = null;
    this.#error = null;
    this.#resolved = false;
    this.#enqueuedKeys.clear();
    this.#enqueuedSessions.clear();
    this.#open = true;
  }

  /**
   * Enqueue the chosen rows as account upserts carrying their existing
   * timestamps (LWW decides). Markers land only after every enqueue resolved;
   * an interrupted merge leaves them unwritten so the retry presents the same
   * rows (EC-11). The `local` default session is re-keyed onto the account
   * default session id before upload; other sessions keep their own UUIDs
   * (§9.4, EC-22).
   */
  async merge(selected: readonly VerseKey[], selectedSessions?: readonly string[]): Promise<void> {
    const userId = this.#userId;
    if (userId === null || this.#merging) return;
    const domain = bookmarksDomainFor(userId);
    const chosen = this.#rows.filter((row) => selected.includes(row.key));
    const remaining = chosen.filter((row) => !this.#enqueuedKeys.has(row.key));
    const wantedSessions = selectedSessions ?? this.#sessionRows.map((row) => row.id);
    const remainingSessions = this.#sessionRows.filter(
      (row) => wantedSessions.includes(row.id) && !this.#enqueuedSessions.has(row.id),
    );
    this.#merging = true;
    this.#error = null;
    for (const row of remaining) {
      try {
        await this.#deps.engine.enqueue(domain, {
          kind: "bookmark.upsert",
          id: newBookmarkEntityId(),
          folderId: null,
          surah: row.surah,
          ayah: row.ayah,
          updatedAt: row.savedAt,
        } satisfies BookmarksMutation);
      } catch (err) {
        // EC-11: no marker on an interrupted merge — the same rows stay queued
        // for the next attempt (already-enqueued keys are never re-sent).
        this.#error = errorMessage(err);
        this.#merging = false;
        return;
      }
      this.#enqueuedKeys.add(row.key);
    }
    let sessionsUploaded = 0;
    for (const row of remainingSessions) {
      try {
        await this.#enqueueSessionRow(userId, row);
      } catch (err) {
        // Same EC-11 rule for every item kind: no marker, same rows on retry.
        this.#error = errorMessage(err);
        this.#merging = false;
        return;
      }
      this.#enqueuedSessions.add(row.id);
      sessionsUploaded += 1;
    }
    if (this.#rows.length > 0) writeRaw("local", legacyMigratedKey(String(userId)), "1");
    const snapshot = this.#localEnvelope;
    if (this.#sessionRows.length > 0 && snapshot !== null) {
      // Watermark of what this review offered: unchanged local sessions stay
      // suppressed, NEW local data re-arms detection (§9.4 (b), no stranding).
      writeRaw("local", sessionsMigratedKey(userId), String(envelopeWatermark(snapshot)));
    }
    let merged = 0;
    let skipped = 0;
    for (const row of chosen) {
      if (this.outcomeFor(row) === "account-wins") skipped += 1;
      else merged += 1;
    }
    merged += sessionsUploaded;
    this.#result = { merged, skipped };
    // No auto `reviewed` marker on a completed merge (§9.4): suppression beyond
    // this review is the user's explicit choice alone ("Don't ask again"), so
    // detection re-arms for NEW local data; unresolved unowned entries stay
    // eligible for the next session's prompt (EC-48).
    this.#merging = false;
    void this.#deps.engine.flush(domain).catch(() => undefined);
    if (sessionsUploaded > 0) {
      void this.#deps.engine.flush(sessionsDomainFor(userId)).catch(() => undefined);
    }
  }

  /** "Not now" / dialog close: nothing enqueued; the prompt returns next browser session (EC-09). */
  close(): void {
    const userId = this.#userId;
    this.#open = false;
    if (userId !== null && !this.#resolved && this.#result === null) {
      writeRaw("session", mergeDeferredKey(userId), "1");
    }
  }

  /** "Don't ask again": permanent per-account suppression (EC-09). */
  neverAsk(): void {
    const userId = this.#userId;
    if (userId !== null) writeRaw("local", mergeReviewedKey(userId), "1");
    this.#resolved = true;
    this.#open = false;
  }

  /** Merge-review action: attribute retained unowned entries to the signed-in account (EC-48). */
  async applyUnowned(): Promise<void> {
    const userId = this.#userId;
    if (userId === null) return;
    try {
      await this.#deps.bookmarks.applyLegacyUnowned(userId);
      this.#unowned = this.#deps.bookmarks.unownedPending();
    } catch (err) {
      this.#error = errorMessage(err);
    }
  }

  /** Merge-review action: discard retained unowned entries (explicit choice, EC-48). */
  async discardUnowned(): Promise<void> {
    try {
      await this.#deps.bookmarks.discardLegacyUnowned();
      this.#unowned = this.#deps.bookmarks.unownedPending();
    } catch (err) {
      this.#error = errorMessage(err);
    }
  }

  #legacyRows(userId: number): MergeBookmarkRow[] {
    if (readRaw("local", legacyMigratedKey(String(userId))) === "1") return [];
    const blob = decodeReader(readJSON(READER_BLOB_KEY));
    const savedAt = blob.bookmarkSavedAt ?? {};
    const rows: MergeBookmarkRow[] = [];
    for (const key of Object.keys(blob.bookmarks ?? {})) {
      const coords = parseVerseKey(key);
      if (coords === null) continue;
      const ms = savedAt[key];
      // SAFETY: parseVerseKey(key) returned non-null above, so key matches the VerseKey `s:a` shape.
      rows.push({
        key: key as VerseKey,
        surah: coords.surah,
        ayah: coords.ayah,
        savedAt: ms !== undefined ? new Date(ms).toISOString() : EPOCH_ISO,
      });
    }
    return rows;
  }

  /**
   * Mergeable `local` sessions (§9.4 detection (b)): every user-created session,
   * plus one synthetic row for the default session when its state holds a
   * position or progress worth uploading. Recents never sync and never trigger.
   */
  #localSessionRows(userId: number): MergeSessionRow[] {
    const envelope = decodeSessionsEnvelope(readJSON(SESSIONS_LOCAL_KEY));
    if (envelope === undefined) return [];
    this.#localEnvelope = { sessions: envelope.sessions, state: envelope.state };
    const rows: MergeSessionRow[] = [];
    for (const session of envelope.sessions) {
      if (session.id === LOCAL_DEFAULT_SESSION_ID) continue;
      rows.push({ id: session.id, name: session.name, isDefault: false });
    }
    const defaultState = envelope.state[LOCAL_DEFAULT_SESSION_ID];
    if (
      defaultState !== undefined &&
      (defaultState.lastRead !== null || Object.keys(defaultState.progress).length > 0)
    ) {
      rows.push({
        id: LOCAL_DEFAULT_SESSION_ID,
        name: LOCAL_DEFAULT_SESSION_NAME,
        isDefault: true,
      });
    }
    if (rows.length === 0) return rows;
    // §9.4 detection (b) re-arm: the marker is a watermark of the envelope at
    // the last completed merge, not a one-way off switch — only unchanged local
    // sessions stay suppressed.
    const migrated = readRaw("local", sessionsMigratedKey(userId));
    if (migrated !== null && Number(migrated) === envelopeWatermark(envelope)) return [];
    return rows;
  }

  /**
   * Upload one chosen session row: the whole session row plus its state's
   * position and per-surah progress, each stamped with its existing timestamp
   * (progress entries carry their own `ts`; a position rides the same surah's
   * progress stamp — `markRead` writes both together). The default row is
   * re-keyed onto the account default session id first (EC-22).
   */
  async #enqueueSessionRow(userId: number, row: MergeSessionRow): Promise<void> {
    const domain = sessionsDomainFor(userId);
    const targetId = row.isDefault ? accountDefaultSessionId(userId) : row.id;
    const stored = this.#localEnvelope?.sessions.find((session) => session.id === row.id);
    let session: ReadingSession;
    if (row.isDefault) {
      session = { ...defaultSessionFor(targetId), ...pickOverrides(stored) };
    } else if (stored !== undefined) {
      session = { ...stored, id: targetId };
    } else {
      return;
    }
    await this.#deps.engine.enqueue(domain, {
      kind: "session.upsert",
      id: targetId,
      name: session.name,
      sessionKind: session.kind,
      translationId: session.translationId,
      archivedAt: session.archivedAt,
      updatedAt: session.updatedAt,
    } satisfies SessionMutation);
    const state = this.#localEnvelope?.state[row.id];
    if (state === undefined) return;
    const last = state.lastRead;
    if (last !== null) {
      await this.#deps.engine.enqueue(domain, {
        kind: "position.upsert",
        sessionId: targetId,
        surah: last.num,
        ayah: last.n,
        localPage: state.lastReadAnchor?.localPage ?? null,
        ratio: state.lastReadAnchor?.ratio ?? null,
        sourceId: last.sourceId ?? null,
        updatedAt: stampOf(state.progress[last.num]?.ts),
      } satisfies SessionMutation);
    }
    for (const [surah, entry] of Object.entries(state.progress)) {
      await this.#deps.engine.enqueue(domain, {
        kind: "progress.upsert",
        sessionId: targetId,
        surah: Number(surah),
        furthestAyah: entry.furthestAyah,
        updatedAt: stampOf(entry.ts),
      } satisfies SessionMutation);
    }
  }
}

/**
 * The seeded local default row keeps its constant id out of the account, but its
 * own fields survive the re-key (a renamed default carries its real timestamp,
 * so LWW sees the device's actual edit; the untouched seed stays at epoch).
 */
function pickOverrides(stored: ReadingSession | undefined): Partial<ReadingSession> {
  if (stored === undefined) return {};
  return {
    name: stored.name,
    kind: stored.kind,
    translationId: stored.translationId,
    archivedAt: stored.archivedAt,
    updatedAt: stored.updatedAt,
  };
}

export const mergeReview = new MergeReview({
  bookmarks: {
    unownedPending: () => bookmarks.unownedPending(),
    applyLegacyUnowned: (ownerId) => bookmarks.applyLegacyUnowned(ownerId),
    discardLegacyUnowned: () => bookmarks.discardLegacyUnowned(),
    get bookmarks() {
      return bookmarks.bookmarks;
    },
  },
  engine: syncEngine,
});
