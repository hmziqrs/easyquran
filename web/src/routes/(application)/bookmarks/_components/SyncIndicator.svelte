<script lang="ts">
  import type { SyncStatus } from "#lib/sync/index.js";
  import { cn } from "#lib/utils.js";
  import type { BookmarksCopy } from "#lib/i18n/bookmarks-copy.js";

  let {
    copy,
    status,
    online: isOnline,
    syncedAt,
    enqueueError = null,
  }: {
    copy: Pick<BookmarksCopy, "synced" | "syncing" | "syncError" | "pending" | "offline" | "saveError">;
    status: SyncStatus;
    online: boolean;
    /** Per-domain sync truth (round-2): epoch ms of the bookmarks domain's last landed snapshot; null = never synced. */
    syncedAt: number | null;
    /** Last durable-enqueue failure (EC-27); the next successful enqueue clears it. */
    enqueueError?: string | null;
  } = $props();

  function label(): string {
    // Truth order (ACCT-03): offline/error/pending first, then the in-flight
    // and never-synced windows — 'synced' only once this domain's server state
    // has actually landed.
    if (!isOnline) return copy.offline;
    if (status.phase === "error") return copy.syncError;
    if (status.pending > 0) return copy.pending(status.pending);
    if (status.phase === "syncing" || syncedAt === null) return copy.syncing;
    return copy.synced;
  }

  /* Scope-extension (run 3): the not-ok dot was bg-pop (--accent = hue-2 emerald) — a green
     dot for offline/pending/error. Status grammar instead: error → danger, offline/pending →
     muted; ok stays primary. Round 2: the syncing/never-synced windows are not-ok too — no
     green dot before server truth has landed. */
  function dotClass(): string {
    if (status.phase === "error") return "bg-danger";
    if (!isOnline || status.pending > 0) return "bg-muted";
    if (status.phase === "syncing" || syncedAt === null) return "bg-muted";
    return "bg-primary";
  }
</script>

<div class="flex flex-col items-end gap-1">
  <!-- ACCT-05: caption size (13.5px) replaces the 12px grey — the sync line is
       real status copy, not a footnote. -->
  <span role="status" class="inline-flex items-center gap-1.5 text-caption tabular-nums text-muted">
    <span class={cn("inline-block size-1.5 rounded-pill", dotClass())} aria-hidden="true"></span>
    {label()}
  </span>
  {#if enqueueError !== null}
    <!-- EC-27: a failed durable write is never silent — the toggle the user just
         made was not saved. Catalog copy stays visible; the raw storage error
         rides along as the tooltip for diagnosis. -->
    <p role="alert" class="text-caption text-danger" title={enqueueError}>{copy.saveError}</p>
  {/if}
</div>
