import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import type { Bookmark, BookmarksMutation } from "#lib/bookmarks/schema.js";
import { legacyMigratedKey } from "#lib/bookmarks/store.svelte.js";
import {
  LOCAL_DEFAULT_SESSION_ID,
  LOCAL_DEFAULT_SESSION_NAME,
  accountDefaultSessionId,
  type ReadingSession,
  type SessionMutation,
  type SessionReaderState,
} from "#lib/sessions/schema.svelte.js";

import {
  MergeReview,
  mergeDeferredKey,
  mergeReviewedKey,
  sessionsMigratedKey,
} from "../merge-review.svelte";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

const EPOCH = "1970-01-01T00:00:00.000Z";
const READ_TS = Date.UTC(2026, 8, 20, 10, 30, 0);
const READ_TS_ISO = "2026-09-20T10:30:00.000Z";

/** Seed the durable reader blob: key → savedAt ms (null = legacy, no timestamp). */
function seedBlob(entries: Record<string, number | null>): void {
  const bookmarks: Record<string, boolean> = {};
  const savedAt: Record<string, number> = {};
  for (const [key, ms] of Object.entries(entries)) {
    bookmarks[key] = true;
    if (ms !== null) savedAt[key] = ms;
  }
  localStorage.setItem(
    "easyquran.reader",
    JSON.stringify({ v: 5, bookmarks, bookmarkSavedAt: savedAt }),
  );
}

interface Rig {
  readonly review: MergeReview;
  readonly enqueued: Array<{ domain: string; payload: SessionMutation | BookmarksMutation }>;
  setUnowned(count: number): void;
  /** The enqueue after `successes` durable ones rejects (simulated storage failure). */
  failAfter(successes: number): void;
}

function makeRig(accountBookmarks: readonly Bookmark[] = []): Rig {
  const enqueued: Array<{ domain: string; payload: SessionMutation | BookmarksMutation }> = [];
  let unowned = 0;
  let failAfterCount = -1;
  const engine = {
    async enqueue<P>(domain: string, payload: P) {
      if (failAfterCount >= 0 && enqueued.length === failAfterCount) {
        failAfterCount = -1;
        throw new Error("idb full");
      }
      // SAFETY: test rig — the review only enqueues payloads that satisfy SessionMutation or
      // BookmarksMutation (one per call site), so the generic P is exactly the recorded union.
      enqueued.push({ domain, payload: payload as SessionMutation | BookmarksMutation });
      // SAFETY: test double — full SyncMutation shape; the review never reads it back.
      return {
        id: `m${enqueued.length}`,
        seq: enqueued.length,
        domain,
        payload,
        queuedAt: READ_TS,
      };
    },
    async flush() {},
  };
  const review = new MergeReview({
    engine,
    bookmarks: {
      unownedPending: () => unowned,
      applyLegacyUnowned: async () => {
        unowned = 0;
        return 1;
      },
      discardLegacyUnowned: async () => {
        unowned = 0;
      },
      bookmarks: accountBookmarks,
    },
  });
  return {
    review,
    enqueued,
    setUnowned(count: number) {
      unowned = count;
    },
    failAfter(successes: number) {
      failAfterCount = successes;
    },
  };
}

function accountRow(surah: number, ayah: number, updatedAt: string): Bookmark {
  return { id: `a-${surah}-${ayah}`, folderId: null, surah, ayah, createdAt: updatedAt, updatedAt };
}

const DAILY_ID = "11111111-1111-4111-8111-111111111111";

function sessionRow(
  id: string,
  name: string,
  updatedAt: string,
  kind: ReadingSession["kind"] = "daily",
): ReadingSession {
  return { id, name, kind, translationId: null, createdAt: updatedAt, updatedAt, archivedAt: null };
}

function readState(surah: number, ayah: number, ts: number): SessionReaderState {
  return {
    lastRead: { num: surah, n: ayah },
    lastReadAnchor: { verseKey: `${surah}:${ayah}`, localPage: 3, ratio: 0.5 },
    recents: [],
    progress: { [surah]: { furthestAyah: ayah, ts } },
  };
}

/** Seed the `local` owner's durable sessions key (Feature 4.2 shape, v1). */
function seedSessions(
  sessions: readonly ReadingSession[],
  state: Readonly<Record<string, SessionReaderState>>,
): void {
  localStorage.setItem(
    "easyquran.sessions.local",
    JSON.stringify({ v: 1, activeId: LOCAL_DEFAULT_SESSION_ID, sessions, state }),
  );
}

describe("MergeReview — merge on login (§9.4)", () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
  });

  it("surfaces the review but enqueues nothing before the explicit choice (EC-08, EC-10)", async () => {
    const rig = makeRig();
    seedBlob({ "2:255": null, "112:1": Date.UTC(2026, 0, 5) });

    rig.review.evaluate(7);
    expect(rig.review.open).toBe(true);
    expect(rig.review.rows).toHaveLength(2);
    expect(rig.enqueued).toEqual([]); // nothing before Merge is clicked

    await rig.review.merge(["2:255", "112:1"]);
    // No savedAt ⇒ epoch, so an account copy of the verse wins LWW while new
    // verses are still added; a real savedAt rides along (EC-10).
    expect(rig.enqueued).toHaveLength(2);
    expect(rig.enqueued[0]).toMatchObject({
      domain: "bookmarks.user:7",
      payload: { kind: "bookmark.upsert", surah: 2, ayah: 255, updatedAt: EPOCH },
    });
    expect(rig.enqueued[1]).toMatchObject({
      payload: { surah: 112, ayah: 1, updatedAt: "2026-01-05T00:00:00.000Z" },
    });
    // Markers land only after the enqueues resolved (EC-11); a completed merge
    // is not "Don't ask again" — no auto reviewed marker beyond the user's
    // explicit ones (§9.4).
    expect(localStorage.getItem(legacyMigratedKey("7"))).toBe("1");
    expect(localStorage.getItem(mergeReviewedKey(7))).toBeNull();
    expect(rig.review.result).toEqual({ merged: 2, skipped: 0 });
    rig.review.close();
    rig.review.evaluate(7);
    expect(rig.review.open).toBe(false); // the (a) marker holds: nothing new to offer
  });

  it("labels and counts both conflict directions against the account view (EC-13)", async () => {
    const rig = makeRig([
      accountRow(2, 255, "2026-02-01T00:00:00Z"), // newer than the epoch row: account wins
      accountRow(3, 9, "2025-01-01T00:00:00Z"), // older than the device row: device wins
    ]);
    seedBlob({ "2:255": null, "3:9": Date.UTC(2026, 0, 5) });
    rig.review.evaluate(7);
    const rows = rig.review.rows;
    expect(rig.review.outcomeFor(rows[0]!)).toBe("account-wins");
    expect(rig.review.outcomeFor(rows[1]!)).toBe("device-wins");

    await rig.review.merge(rows.map((row) => row.key));
    expect(rig.review.result).toEqual({ merged: 1, skipped: 1 });
  });

  it("an interrupted merge writes no marker; the retry presents the same rows (EC-11)", async () => {
    const rig = makeRig();
    seedBlob({ "2:255": null, "112:1": null });
    rig.review.evaluate(7);
    rig.failAfter(1);
    await rig.review.merge(["2:255", "112:1"]);

    expect(rig.enqueued).toHaveLength(1); // first key durable, second rejected
    expect(localStorage.getItem(legacyMigratedKey("7"))).toBeNull();
    expect(localStorage.getItem(mergeReviewedKey(7))).toBeNull();
    expect(rig.review.result).toBeNull();
    expect(rig.review.rows).toHaveLength(2); // same items on retry

    await rig.review.merge(["2:255", "112:1"]); // already-sent key is not re-enqueued
    expect(rig.enqueued).toHaveLength(2);
    expect(localStorage.getItem(legacyMigratedKey("7"))).toBe("1");
  });

  it("Not now defers to the next browser session; Don't ask again is permanent (EC-09)", () => {
    const rig = makeRig();
    seedBlob({ "1:1": null });

    rig.review.evaluate(7);
    expect(rig.review.open).toBe(true);
    rig.review.close();
    expect(sessionStorage.getItem(mergeDeferredKey(7))).toBe("1");
    expect(localStorage.getItem(mergeReviewedKey(7))).toBeNull();
    rig.review.evaluate(7);
    expect(rig.review.open).toBe(false); // deferred within this browser session

    sessionStorage.clear();
    rig.review.evaluate(7);
    expect(rig.review.open).toBe(true);
    rig.review.neverAsk();
    expect(localStorage.getItem(mergeReviewedKey(7))).toBe("1");

    // Markers are per-account: another account still gets its review.
    rig.review.evaluate(9);
    expect(rig.review.open).toBe(true);
  });

  it("retained unowned entries surface only through an explicit choice (EC-48)", async () => {
    const rig = makeRig();
    rig.setUnowned(2);
    rig.review.evaluate(7);
    expect(rig.review.open).toBe(true);
    expect(rig.review.rows).toHaveLength(0);
    expect(rig.review.unowned).toBe(2);

    await rig.review.applyUnowned();
    expect(rig.review.unowned).toBe(0);
    // The review is not complete yet (no local rows, nothing merged): closing
    // defers, and unresolved entries can resurface next session.
    rig.review.close();
    expect(sessionStorage.getItem(mergeDeferredKey(7))).toBe("1");
  });

  it("local sessions trigger the review and merge under the account partition (§9.4 detection (b))", () => {
    const rig = makeRig();
    seedSessions([sessionRow(LOCAL_DEFAULT_SESSION_ID, LOCAL_DEFAULT_SESSION_NAME, EPOCH)], {
      [LOCAL_DEFAULT_SESSION_ID]: readState(2, 255, READ_TS),
    });

    rig.review.evaluate(7);
    expect(rig.review.open).toBe(true);
    expect(rig.review.rows).toHaveLength(0); // bookmarks did not trigger this
    expect(rig.review.sessionRows).toHaveLength(1);
    expect(rig.review.sessionRows[0]).toMatchObject({
      id: LOCAL_DEFAULT_SESSION_ID,
      isDefault: true,
    });
    expect(rig.enqueued).toEqual([]); // nothing before the explicit choice
  });

  it("an empty default session never triggers the review", () => {
    const rig = makeRig();
    seedSessions([sessionRow(LOCAL_DEFAULT_SESSION_ID, LOCAL_DEFAULT_SESSION_NAME, EPOCH)], {
      [LOCAL_DEFAULT_SESSION_ID]: {
        lastRead: null,
        lastReadAnchor: null,
        recents: [],
        progress: {},
      },
    });
    rig.review.evaluate(7);
    expect(rig.review.open).toBe(false);
  });

  it("the local default session re-keys onto the account default id; others keep their UUIDs (EC-22)", async () => {
    const rig = makeRig();
    seedSessions(
      [
        sessionRow(LOCAL_DEFAULT_SESSION_ID, LOCAL_DEFAULT_SESSION_NAME, EPOCH, "free"),
        sessionRow(DAILY_ID, "Daily", "2026-09-01T00:00:00.000Z"),
      ],
      {
        [LOCAL_DEFAULT_SESSION_ID]: readState(2, 255, READ_TS),
        [DAILY_ID]: readState(112, 3, READ_TS),
      },
    );
    rig.review.evaluate(7);
    const target = accountDefaultSessionId(7);

    await rig.review.merge(
      [],
      rig.review.sessionRows.map((row) => row.id),
    );
    const sessionEnqueues = rig.enqueued.filter((entry) => entry.domain === "sessions.user:7");
    expect(sessionEnqueues).toHaveLength(6); // row + position + progress per session
    expect(sessionEnqueues[0]?.payload).toMatchObject({
      kind: "session.upsert",
      id: DAILY_ID, // own UUID, own timestamp
      name: "Daily",
      updatedAt: "2026-09-01T00:00:00.000Z",
    });
    expect(sessionEnqueues[1]?.payload).toMatchObject({
      kind: "position.upsert",
      sessionId: DAILY_ID,
      surah: 112,
      ayah: 3,
      localPage: 3,
      ratio: 0.5,
    });
    expect(sessionEnqueues[2]?.payload).toMatchObject({
      kind: "progress.upsert",
      sessionId: DAILY_ID,
      surah: 112,
      furthestAyah: 3,
      updatedAt: READ_TS_ISO,
    });
    expect(sessionEnqueues[3]?.payload).toMatchObject({
      kind: "session.upsert",
      id: target, // re-keyed: the constant local id never uploads (EC-22)
      name: LOCAL_DEFAULT_SESSION_NAME,
      sessionKind: "free",
      updatedAt: EPOCH,
    });
    expect(sessionEnqueues[4]?.payload).toMatchObject({
      kind: "position.upsert",
      sessionId: target,
      surah: 2,
      ayah: 255,
      updatedAt: READ_TS_ISO, // existing progress stamp, not nowIso()
    });
    expect(sessionEnqueues[5]?.payload).toMatchObject({
      kind: "progress.upsert",
      sessionId: target,
      surah: 2,
      furthestAyah: 255,
      updatedAt: READ_TS_ISO,
    });

    // The sessions marker is a watermark of the envelope at merge time (max of
    // the rows' updatedAt and the states' progress stamps — here READ_TS), and
    // no reviewed marker is auto-written.
    expect(localStorage.getItem(sessionsMigratedKey(7))).toBe(String(READ_TS));
    expect(localStorage.getItem(mergeReviewedKey(7))).toBeNull();
    expect(rig.review.result).toEqual({ merged: 2, skipped: 0 });
  });

  it("an interrupted session merge writes no marker; the retry finishes it (EC-11, every kind)", async () => {
    const rig = makeRig();
    seedSessions([sessionRow(LOCAL_DEFAULT_SESSION_ID, LOCAL_DEFAULT_SESSION_NAME, EPOCH)], {
      [LOCAL_DEFAULT_SESSION_ID]: readState(1, 1, READ_TS),
    });
    rig.review.evaluate(7);
    rig.failAfter(1); // the first session.upsert lands, the position rejects
    await rig.review.merge([], undefined);
    expect(rig.enqueued).toHaveLength(1);
    expect(localStorage.getItem(sessionsMigratedKey(7))).toBeNull();
    expect(localStorage.getItem(mergeReviewedKey(7))).toBeNull();

    await rig.review.merge([], undefined); // retry: same row, not re-sent wholesale
    expect(localStorage.getItem(sessionsMigratedKey(7))).toBe(String(READ_TS));
    expect(rig.review.result).toEqual({ merged: 1, skipped: 0 });
  });

  it("a completed session merge re-arms detection for NEW local data, not for the same rows", async () => {
    const rig = makeRig();
    const seed = (): void => {
      seedSessions([sessionRow(LOCAL_DEFAULT_SESSION_ID, LOCAL_DEFAULT_SESSION_NAME, EPOCH)], {
        [LOCAL_DEFAULT_SESSION_ID]: readState(2, 255, READ_TS),
      });
    };
    seed();
    rig.review.evaluate(7);
    await rig.review.merge(
      [],
      rig.review.sessionRows.map((row) => row.id),
    );
    expect(localStorage.getItem(sessionsMigratedKey(7))).toBe(String(READ_TS));

    // Unchanged envelope: the watermark matches, nothing new to offer.
    rig.review.close();
    rig.review.evaluate(7);
    expect(rig.review.open).toBe(false);

    // NEW local session later (signed-out use): the watermark moves, the
    // prompt returns instead of stranding the data behind a one-way marker.
    seedSessions(
      [
        sessionRow(LOCAL_DEFAULT_SESSION_ID, LOCAL_DEFAULT_SESSION_NAME, EPOCH),
        sessionRow(DAILY_ID, "Daily", "2026-09-25T00:00:00.000Z"),
      ],
      { [LOCAL_DEFAULT_SESSION_ID]: readState(2, 255, READ_TS) },
    );
    rig.review.evaluate(7);
    expect(rig.review.open).toBe(true);
    expect(rig.review.sessionRows).toHaveLength(2);
  });

  it("the sessions watermark suppresses an unchanged envelope for that account", () => {
    const rig = makeRig();
    seedSessions([sessionRow(LOCAL_DEFAULT_SESSION_ID, LOCAL_DEFAULT_SESSION_NAME, EPOCH)], {
      [LOCAL_DEFAULT_SESSION_ID]: readState(2, 255, READ_TS),
    });
    localStorage.setItem(sessionsMigratedKey(7), String(READ_TS));
    rig.review.evaluate(7);
    expect(rig.review.open).toBe(false);
  });
});
