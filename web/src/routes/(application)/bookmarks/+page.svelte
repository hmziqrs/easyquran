<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { loadQuranData, peekQuranData } from "#lib/data/quran-data-client.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import { surahAyahPathFor, surahRouteContext } from "#lib/data/quran.js";
  import { authModal } from "#lib/auth/auth-modal.svelte.js";
  import { online } from "#lib/offline/online.svelte.js";
  import { reader } from "#lib/stores/reader.svelte.js";
  import { bookmarks } from "#lib/bookmarks/store.svelte.js";
  import type { Bookmark, BookmarkFolder } from "#lib/bookmarks/schema.js";
  import { parseVerseKey, verseKeyOf } from "#lib/bookmarks/schema.js";
  import { appLocale } from "#lib/i18n/app-locale.js";
  import { getBookmarksCopy } from "#lib/i18n/bookmarks-copy.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Icon } from "#lib/components/icon/index.js";
  import { Skeleton } from "#lib/components/ui/skeleton/index.js";
  import BookmarkRow from "./_components/BookmarkRow.svelte";
  import FoldersPanel from "./_components/FoldersPanel.svelte";
  import SyncIndicator from "./_components/SyncIndicator.svelte";

  // Reader nav context for opening a bookmarked verse: a non-reader app page has
  // no translation segments to build, so it always resolves the Arabic reader.
  const ARABIC_CTX = surahRouteContext("uthmani");

  // This URL is never locale-prefixed, so getLocale() would fall back to the
  // base locale even right after a client-side nav from /ar/**; the layout's
  // published locale keeps the page's copy aligned with the nav/footer chrome.
  const copy = getBookmarksCopy(appLocale());

  let quranData = $state<QuranData | null>(peekQuranData() ?? null);

  const authed = $derived(bookmarks.authed);

  // Manual order first (NULL position = unpositioned, sorts last), then name
  // (Feature 2.3). FoldersPanel renders this order and reorders relative to it.
  const sortedFolders = $derived(
    [...bookmarks.folders].sort((a, b) => {
      const positions = (a.position ?? Number.MAX_SAFE_INTEGER) - (b.position ?? Number.MAX_SAFE_INTEGER);
      return positions !== 0 ? positions : a.name.localeCompare(b.name);
    }),
  );

  interface BookmarkGroup {
    readonly id: string | null;
    readonly label: string;
    readonly rows: readonly Bookmark[];
  }

  // Store-side grouping: orphaned folderIds land under null, never hidden.
  const byFolder = $derived(bookmarks.bookmarksByFolder());

  function rowsFor(folderId: string | null): readonly Bookmark[] {
    return [...(byFolder.get(folderId) ?? [])].sort((a, b) =>
      b.updatedAt.localeCompare(a.updatedAt),
    );
  }

  const groups = $derived.by<BookmarkGroup[]>(() => {
    const unfiled: BookmarkGroup = { id: null, label: copy.unfiled, rows: rowsFor(null) };
    const named = sortedFolders.map(
      (folder): BookmarkGroup => ({ id: folder.id, label: folder.name, rows: rowsFor(folder.id) }),
    );
    // Unfiled first only when it carries rows; named folders always render so
    // their empty state stays visible.
    if (unfiled.rows.length === 0) return named;
    return [unfiled, ...named];
  });

  const hasAnyBookmark = $derived(bookmarks.bookmarks.length > 0);

  // Authed but no first snapshot yet: keep the skeleton up — even when the
  // round failed (offline first visit must not flash "No bookmarks"), and any
  // local optimistic row or folder means there is real content to render.
  const waitingForFirstSync = $derived(
    authed &&
      bookmarks.status.lastSyncAt === null &&
      !hasAnyBookmark &&
      sortedFolders.length === 0,
  );
  // Same skeleton for the blank window after sync while the quran catalog is
  // still loading (rows/folders exist but cannot render yet).
  const firstSyncPending = $derived(waitingForFirstSync || (authed && quranData === null));

  // Anon catalog wait: anonRows stays empty until the quran catalog lands, so
  // the empty state would flash — hold the loading skeleton instead.
  const anonCatalogPending = $derived(!authed && quranData === null);

  interface AnonRow {
    readonly key: string;
    readonly surah: number;
    readonly ayah: number;
    readonly name: string;
    readonly href: string;
  }

  const anonRows = $derived.by<AnonRow[]>(() => {
    const data = quranData;
    if (data === null) return [];
    const out: AnonRow[] = [];
    for (const key of reader.bookmarkedKeys) {
      const coords = parseVerseKey(key);
      if (coords === null) continue;
      const entry = data.surahByNum(coords.surah);
      if (!entry) continue;
      out.push({
        key,
        surah: coords.surah,
        ayah: coords.ayah,
        name: entry.name,
        href: surahAyahPathFor(ARABIC_CTX, entry, coords.ayah),
      });
    }
    return out;
  });

  /** Display data for an authed bookmark row; null while the catalog is not loaded yet. */
  function rowMeta(bookmark: Bookmark): { name: string; href: string | null } | null {
    const data = quranData;
    if (data === null) return null;
    const entry = data.surahByNum(bookmark.surah);
    if (!entry) return null;
    return {
      name: entry.name,
      href: surahAyahPathFor(ARABIC_CTX, entry, bookmark.ayah),
    };
  }

  function folderOf(bookmark: Bookmark): BookmarkFolder | null {
    if (bookmark.folderId === null) return null;
    return bookmarks.folderById(bookmark.folderId) ?? null;
  }

  // Local-only note lookup (BM-02): notes never leave the device, so the icon
  // is resolved at render time against the reader's per-owner note store.
  function hasNote(bookmark: Bookmark): boolean {
    return reader.getNote(verseKeyOf(bookmark.surah, bookmark.ayah)).trim().length > 0;
  }

  function countIn(folderId: string): number {
    return byFolder.get(folderId)?.length ?? 0;
  }

  // Daily-folder progress (Feature 2.3): share of the folder's bookmarked
  // verses at or below the ACTIVE session's furthestAyah for their surah.
  // reader.progressFor mirrors the active session (Feature 4) as a fraction of
  // the surah; the integer ayah is recovered exactly — fraction·total stays
  // within 1 ulp of the stored integer, so round() cannot drift.
  const folderProgress = $derived.by<Map<string, { done: number; total: number }>>(() => {
    const out = new Map<string, { done: number; total: number }>();
    for (const folder of bookmarks.folders) {
      if (folder.kind !== "daily") continue;
      const rows = byFolder.get(folder.id) ?? [];
      if (rows.length === 0) continue;
      let done = 0;
      for (const row of rows) {
        if (row.ayah <= furthestAyahRead(row.surah)) done += 1;
      }
      out.set(folder.id, { done, total: rows.length });
    }
    return out;
  });

  function furthestAyahRead(surah: number): number {
    const total = quranData?.surahByNum(surah)?.ayahCount ?? 0;
    const fraction = reader.progressFor(surah);
    if (fraction === null || total === 0) return 0;
    if (fraction >= 1) return total;
    return Math.round(fraction * total);
  }

  // Bulk selection (P05): move/remove many rows at once. SvelteSet: the set
  // mutates in place, so row checkboxes and the toolbar track it directly.
  let selected = new SvelteSet<string>();

  function toggleSelect(bookmarkId: string, next: boolean): void {
    if (next) selected.add(bookmarkId);
    else selected.delete(bookmarkId);
  }

  function moveSelected(folderId: string | null): void {
    for (const id of selected) bookmarks.moveToFolder(id, folderId);
    selected.clear();
  }

  function removeSelected(): void {
    const rows = bookmarks.bookmarks.filter((bookmark) => selected.has(bookmark.id));
    if (rows.length === 0) {
      selected.clear();
      return;
    }
    armUndo({ bookmarks: rows });
    for (const row of rows) bookmarks.remove(row.id);
    selected.clear();
  }

  /** Manual order = normalized indices: apply the move, then one position write per folder whose slot changed. */
  function reorderFolder(id: string, direction: -1 | 1): void {
    const index = sortedFolders.findIndex((folder) => folder.id === id);
    if (index === -1) return;
    const target = index + direction;
    if (target < 0 || target >= sortedFolders.length) return;
    const next = [...sortedFolders];
    const [moved] = next.splice(index, 1);
    if (moved === undefined) return;
    next.splice(target, 0, moved);
    for (const [slot, folder] of next.entries()) {
      if (folder.position !== slot) bookmarks.updateFolder(folder.id, { position: slot });
    }
  }

  // Remove flow (BM-04): optimistic remove + 5-second undo, signed-in and
  // offline alike; bulk removals undo as one batch.
  let undoState = $state<{ bookmarks: readonly Bookmark[] } | { anonKey: string } | null>(null);
  let undoTimer: ReturnType<typeof setTimeout> | undefined;

  function armUndo(next: { bookmarks: readonly Bookmark[] } | { anonKey: string }): void {
    clearTimeout(undoTimer);
    undoState = next;
    undoTimer = setTimeout(() => {
      undoState = null;
    }, 5000);
  }

  function removeAuthed(bookmark: Bookmark): void {
    armUndo({ bookmarks: [bookmark] });
    bookmarks.remove(bookmark.id);
  }

  function removeAnon(row: AnonRow): void {
    armUndo({ anonKey: row.key });
    reader.toggleBookmark(row.key);
  }

  function undoRemove(): void {
    if (undoState === null) return;
    if ("bookmarks" in undoState) {
      for (const bookmark of undoState.bookmarks) bookmarks.restore(bookmark);
    } else {
      reader.toggleBookmark(undoState.anonKey);
    }
    clearTimeout(undoTimer);
    undoState = null;
  }

  function openSignIn(): void {
    authModal.show("login");
  }

  onMount(() => {
    if (peekQuranData() === undefined) {
      void loadQuranData().then(
        (data) => {
          quranData = data;
        },
        () => undefined,
      );
    }
    return () => clearTimeout(undoTimer);
  });
</script>

<svelte:head>
  <title>{copy.title} · EasyQuran</title>
</svelte:head>

<div lang={copy.locale} dir={copy.direction}>
  {#snippet loadingRows()}
    <span class="sr-only">{copy.loading}</span>
    <Skeleton class="h-16 w-full" />
    <Skeleton class="h-16 w-full" />
    <Skeleton class="h-16 w-full" />
  {/snippet}
  {#snippet emptyState(text: string)}
    <!-- Hue-2 soft/legible icon holder (§6) — the empty state matches the search
         page's, instead of bare grey text. -->
    <div class="mt-6 flex items-center gap-3.5">
      <span
        class="flex size-11 flex-none items-center justify-center rounded-sm text-[var(--hue-2-legible)]"
        style:background="var(--hue-2-soft)"
        aria-hidden="true"
      >
        <Icon name="bookmark" size={20} />
      </span>
      <p class="max-w-[70ch] text-[14.5px] leading-relaxed text-foreground-secondary">{text}</p>
    </div>
  {/snippet}

  <div class="mx-auto max-w-[1180px] px-6 pt-5 pb-10 sm:px-7 sm:pt-6 sm:pb-12">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <h1 class="text-[28px] font-medium leading-tight tracking-[-0.02em] text-foreground"
        >{copy.title}</h1
      >
      {#if authed}
        <SyncIndicator copy={copy} status={bookmarks.status} online={online.online} />
      {:else}
        <span class="text-caption text-muted">{copy.localOnly}</span>
      {/if}
    </div>

    {#if undoState !== null}
      <div
        role="status"
        class="mt-4 flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border-strong bg-background-subtle px-4 py-2.5"
      >
        <p class="text-caption text-foreground-secondary">{copy.removed}</p>
        <button
          type="button"
          class="inline-flex h-8 items-center rounded-pill border border-border-strong px-3 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:border-border hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          onclick={undoRemove}>{copy.undo}</button
        >
      </div>
    {/if}

    {#if authed}
      {#if firstSyncPending}
        <div class="mt-6 flex flex-col gap-3" role="status">
          {@render loadingRows()}
          <!-- Offline note only; an online sync failure surfaces solely in the
               header SyncIndicator, never duplicated inline. -->
          {#if waitingForFirstSync && !online.online}
            <p class="max-w-[70ch] text-caption leading-relaxed text-muted">{copy.offline}</p>
          {/if}
        </div>
      {:else if quranData !== null && !hasAnyBookmark && sortedFolders.length === 0}
        {@render emptyState(copy.empty)}
      {:else if quranData !== null}
        <FoldersPanel
          {copy}
          folders={sortedFolders}
          {countIn}
          progressIn={(id) => folderProgress.get(id) ?? null}
          onCreate={(name) => bookmarks.createFolder(name)}
          onUpdate={(id, patch) => bookmarks.updateFolder(id, patch)}
          onDelete={(id) => bookmarks.deleteFolder(id)}
          onReorder={(id, direction) => reorderFolder(id, direction)}
        />

        {#if selected.size > 0}
          <!-- Bulk toolbar (P05): acts on the selected rows only. -->
          <div
            class="mt-6 flex flex-wrap items-center gap-2 rounded-xl border border-border-strong bg-background-subtle px-4 py-2.5 sm:px-5"
          >
            <span class="text-caption tabular-nums text-foreground-secondary"
              >{copy.selected(selected.size)}</span
            >
            <span class="flex items-center gap-1.5">
              <label class="sr-only" for="bookmarks-bulk-move">{copy.moveSelectedTo}</label>
              <select
                id="bookmarks-bulk-move"
                onchange={(event) => {
                  const value = (event.currentTarget as HTMLSelectElement).value;
                  moveSelected(value === "" ? null : value);
                }}
                class="h-8 rounded-lg border border-border-strong bg-surface px-2 text-[12.5px] text-foreground-secondary transition-colors hover:border-border hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
              >
                <option value="">{copy.moveSelectedTo}</option>
                {#each sortedFolders as folder (folder.id)}
                  <option value={folder.id}>{folder.name}</option>
                {/each}
              </select>
            </span>
            <button
              type="button"
              class="inline-flex h-8 items-center rounded-lg border border-danger px-2.5 text-[12.5px] text-danger transition-colors hover:border-danger focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
              onclick={removeSelected}>{copy.removeSelected}</button
            >
            <button
              type="button"
              class="inline-flex h-8 items-center rounded-lg border border-border-strong px-2.5 text-[12.5px] text-foreground-secondary transition-colors hover:border-border hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
              onclick={() => selected.clear()}>{copy.clearSelection}</button
            >
          </div>
        {/if}

        {#if hasAnyBookmark}
          <div class="mt-6 flex flex-col gap-6">
            {#each groups as group (group.id ?? "unfiled")}
              <section
                aria-labelledby="bookmarks-group-{group.id ?? `unfiled`}"
                class="overflow-hidden rounded-xl border border-border-strong bg-surface"
              >
                <h2
                  id="bookmarks-group-{group.id ?? `unfiled`}"
                  class="eyebrow mb-0 border-b border-border px-4 py-3 sm:px-5"
                  >{group.label}
                  <span class="ms-1 text-[12px] text-muted">{group.rows.length}</span>
                </h2>
                {#if group.rows.length === 0}
                  <p class="px-4 py-3.5 text-caption text-muted sm:px-5">{copy.folderEmpty}</p>
                {:else}
                  <ul class="divide-y divide-border">
                    {#each group.rows as bookmark (bookmark.id)}
                      {@const meta = rowMeta(bookmark)}
                      {#if meta !== null}
                        <BookmarkRow
                          {copy}
                          {bookmark}
                          surahName={meta.name}
                          href={meta.href}
                          folder={folderOf(bookmark)}
                          folders={sortedFolders}
                          onMove={(id, folderId) => bookmarks.moveToFolder(id, folderId)}
                          onRemove={() => removeAuthed(bookmark)}
                          onSelect={toggleSelect}
                          selected={selected.has(bookmark.id)}
                          noteIcon={hasNote(bookmark)}
                        />
                      {/if}
                    {/each}
                  </ul>
                {/if}
              </section>
            {/each}
          </div>
        {/if}
      {/if}
    {:else}
      {#if anonCatalogPending}
        <div class="mt-6 flex flex-col gap-3" role="status">
          {@render loadingRows()}
        </div>
      {:else if anonRows.length === 0}
        {@render emptyState(copy.empty)}
      {:else}
        <ul class="mt-6 divide-y divide-border overflow-hidden rounded-xl border border-border-strong bg-surface">
          {#each anonRows as row (row.key)}
            <BookmarkRow
              {copy}
              bookmark={{ id: row.key, folderId: null, surah: row.surah, ayah: row.ayah, updatedAt: "" }}
              surahName={row.name}
              href={row.href}
              onRemove={() => removeAnon(row)}
            />
          {/each}
        </ul>
      {/if}

      <div
        class="mt-6 flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border-strong bg-background-subtle px-4 py-3.5 sm:px-5"
      >
        <p class="max-w-[60ch] text-caption leading-relaxed text-foreground-secondary">{copy.signInNote}</p>
        <Button variant="primary" size="sm" onclick={openSignIn}>{copy.signIn}</Button>
      </div>
    {/if}
  </div>
</div>
