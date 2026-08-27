<script lang="ts">
  import type { SyncStatus } from "$lib/sync";
  import { cn } from "$lib/utils";
  import type { BookmarksCopy } from "$lib/i18n/bookmarks-copy";

  let {
    copy,
    status,
    online: isOnline,
  }: {
    copy: Pick<BookmarksCopy, "synced" | "syncError" | "pending" | "offline">;
    status: SyncStatus;
    online: boolean;
  } = $props();

  function label(): string {
    if (!isOnline) return copy.offline;
    if (status.phase === "error") return copy.syncError;
    if (status.pending > 0) return copy.pending(status.pending);
    return copy.synced;
  }

  function dotClass(): string {
    if (isOnline && status.phase !== "error") return "bg-accent";
    return "bg-pop";
  }
</script>

<span role="status" class="inline-flex items-center gap-1.5 text-[12px] tabular-nums text-fg-3">
  <span class={cn("inline-block size-1.5 rounded-full", dotClass())} aria-hidden="true"></span>
  {label()}
</span>
