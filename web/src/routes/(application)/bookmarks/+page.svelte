<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { loadQuranData, peekQuranData } from "#lib/data/quran-data-client.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import {
    surahAyahPathFor,
    surahRouteContext,
    type SurahRouteContext,
  } from "#lib/data/quran.js";
  import { peekTranslationName } from "#lib/quran/catalogue.js";
  import { authModal } from "#lib/auth/auth-modal.svelte.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { online } from "#lib/offline/online.svelte.js";
  import { reader } from "#lib/stores/reader.svelte.js";
  import { bookmarks } from "#lib/bookmarks/store.svelte.js";
  import type { Bookmark, BookmarkFolder } from "#lib/bookmarks/schema.js";
  import { parseVerseKey, verseKeyOf } from "#lib/bookmarks/schema.js";
  import { appLocale } from "#lib/i18n/app-locale.js";
  import { getBookmarksCopy } from "#lib/i18n/bookmarks-copy.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import * as Select from "#lib/components/ui/select/index.js";
  import { Icon } from "#lib/components/icon/index.js";
  import { Skeleton } from "#lib/components/ui/skeleton/index.js";
  import BookmarkRow from "./_components/BookmarkRow.svelte";
  import FoldersPanel from "./_components/FoldersPanel.svelte";
  import SyncIndicator from "./_components/SyncIndicator.svelte";

  // Reader nav context for opening a bookmarked verse: a non-reader app page has
  // no translation segments to build, so it always resolves the Arabic reader.
  const ARABIC_CTX = surahRouteContext("uthmani");

  // Saved-translation reopen (Feature 3.5): still in the catalogue → that translation, else Arabic (EC-38).
  function ctxForSavedSource(sourceId: string | null | undefined): SurahRouteContext {
    if (sourceId !== null && sourceId !== undefined && peekTranslationName(sourceId) !== null) {
      return surahRouteContext(sourceId);
    }
    return ARABIC_CTX;
  }

  // This URL is never locale-prefixed, so getLocale() would fall back to the
  // base locale even right after a client-side nav from /ar/**; the layout's
  // published locale keeps the page's copy aligned with the nav/footer chrome.
  const copy = getBookmarksCopy(appLocale());

  let quranData = $state<QuranData | null>(peekQuranData() ?? null);

  const authed = $derived(bookmarks.authed);

  // Signed-in cold load (round-2): while the probe is in flight the session is
  // neither anon nor authed — render a neutral loading state, never the anon
  // "Sign in (free)" view.
  const authUnknown = $derived(authState.unknown);

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

  // List controls (FLOW-08): sort by date / folder / surah, filter by folder.
  type SortMode = "date" | "folder" | "surah";
  let sortMode = $state<SortMode>("folder");
  let folderFilter = $state<string>("all");

  // shadcn Select label persistence: closed triggers read their labels from
  // `items`; the filter list is derived because sortedFolders is.
  const sortItems = [
    { value: "date", label: copy.sortDate },
    { value: "folder", label: copy.sortFolder },
    { value: "surah", label: copy.sortSurah },
  ];
  const folderItems = $derived(
    sortedFolders.map((folder) => ({ value: folder.id, label: folder.name })),
  );
  const filterItems = $derived([
    { value: "all", label: copy.filterAll },
    { value: "unfiled", label: copy.unfiled },
    ...folderItems,
  ]);

  const filteredRows = $derived(
    bookmarks.bookmarks.filter((bookmark) => {
      if (folderFilter === "all") return true;
      if (folderFilter === "unfiled") return bookmark.folderId === null;
      return bookmark.folderId === folderFilter;
    }),
  );

  function datedRows(folderId: string | null): readonly Bookmark[] {
    return filteredRows
      .filter((bookmark) => bookmark.folderId === folderId)
      .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  }

  const flatRows = $derived.by<readonly Bookmark[]>(() => {
    if (sortMode === "folder") return [];
    return [...filteredRows].sort(
      sortMode === "date"
        ? (a, b) => b.updatedAt.localeCompare(a.updatedAt)
        : (a, b) => a.surah - b.surah || a.ayah - b.ayah,
    );
  });

  const groups = $derived.by<BookmarkGroup[]>(() => {
    if (folderFilter !== "all") {
      const id = folderFilter === "unfiled" ? null : folderFilter;
      const label = id === null ? copy.unfiled : (bookmarks.folderById(id)?.name ?? copy.unfiled);
      return [{ id, label, rows: datedRows(id) }];
    }
    const named = sortedFolders.map(
      (folder): BookmarkGroup => ({ id: folder.id, label: folder.name, rows: datedRows(folder.id) }),
    );
    const unfiled: BookmarkGroup = { id: null, label: copy.unfiled, rows: datedRows(null) };
    // Unfiled first only when it carries rows; named folders always render so
    // their empty state stays visible.
    if (unfiled.rows.length === 0) return named;
    return [unfiled, ...named];
  });

  const hasAnyBookmark = $derived(bookmarks.bookmarks.length > 0);

  // Fully empty authed list: show the page empty state — and (round-2) still
  // mount the folders panel so a fresh account can create its first folder.
  const emptyList = $derived(!hasAnyBookmark && sortedFolders.length === 0);

  // Offline cold load (offline-skeleton fix): with no network the first sync
  // can never complete this page lifetime, so the skeleton must not wait on
  // syncedAt alone — it also releases on the offline state or this deadline,
  // and the snapshot replaces the empty view once a later sync lands it.
  const FIRST_SYNC_SKELETON_MAX_MS = 10_000;
  let firstSyncWindowElapsed = $state(false);

  // Authed but no first snapshot yet: keep the skeleton up only while a
  // snapshot can still land (online, inside the bounded window) — an offline
  // first visit must not spin forever on "Loading your bookmarks", and any
  // local optimistic row or folder means there is real content to render.
  // Round-2: keyed on THIS domain's syncedAt (store truth), never the engine
  // global any-domain lastSyncAt.
  const waitingForFirstSync = $derived(
    authed &&
      bookmarks.syncedAt === null &&
      emptyList &&
      online.online &&
      !firstSyncWindowElapsed,
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
    readonly updatedAt: string;
    /** v5 save stamp (epoch ms); 0 = legacy row with no known time (EC-46). */
    readonly savedAt: number;
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
      const savedAt = reader.bookmarkSavedAt[key] ?? 0;
      out.push({
        key,
        surah: coords.surah,
        ayah: coords.ayah,
        name: entry.name,
        href: surahAyahPathFor(ARABIC_CTX, entry, coords.ayah),
        updatedAt: savedAt > 0 ? new Date(savedAt).toISOString() : "",
        savedAt,
      });
    }
    // Newest first; legacy stamp-less rows sort last (EC-46).
    out.sort((a, b) => b.savedAt - a.savedAt);
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
      href: surahAyahPathFor(ctxForSavedSource(bookmark.sourceId), entry, bookmark.ayah),
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

  // Remove flow (BM-04): optimistic remove + 5s undo; rapid removes coalesce — all restore.
  type UndoEntry = { bookmarks: readonly Bookmark[] } | { anonKey: string };
  let undoEntries = $state<UndoEntry[]>([]);
  let undoTimer: ReturnType<typeof setTimeout> | undefined;

  function armUndo(next: UndoEntry): void {
    clearTimeout(undoTimer);
    undoEntries = [...undoEntries, next];
    undoTimer = setTimeout(() => {
      undoEntries = [];
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
    for (const entry of undoEntries) {
      if ("bookmarks" in entry) {
        for (const bookmark of entry.bookmarks) bookmarks.restore(bookmark);
      } else {
        reader.toggleBookmark(entry.anonKey);
      }
    }
    clearTimeout(undoTimer);
    undoEntries = [];
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
    // First-sync deadline (offline-skeleton fix): even an online round that
    // never lands releases the skeleton once the bounded window has passed.
    const skeletonTimer = setTimeout(() => {
      firstSyncWindowElapsed = true;
    }, FIRST_SYNC_SKELETON_MAX_MS);
    return () => {
      clearTimeout(undoTimer);
      clearTimeout(skeletonTimer);
    };
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
      {#if authUnknown}
        <!-- Neutral: the auth probe is in flight; no sync truth and no anon note yet. -->
      {:else if authed}
        <SyncIndicator
          copy={copy}
          status={bookmarks.status}
          online={online.online}
          syncedAt={bookmarks.syncedAt}
          enqueueError={bookmarks.enqueueError}
        />
      {:else}
        <span class="text-caption text-muted">{copy.localOnly}</span>
      {/if}
    </div>

    {#if undoEntries.length > 0}
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

    {#if authUnknown}
      <!-- Round-2: signed-in cold load — neutral skeleton while the auth probe
           is in flight, never the anon sign-in view. -->
      <div class="mt-6 flex flex-col gap-3" role="status">
        {@render loadingRows()}
      </div>
    {:else if authed}
      {#if firstSyncPending}
        <div class="mt-6 flex flex-col gap-3" role="status">
          {@render loadingRows()}
        </div>
      {:else if quranData !== null}
        {#if !online.online && bookmarks.syncedAt === null}
          <!-- Offline cold load (offline-skeleton fix): the skeleton resolved
               to this view without a snapshot, so the blank list carries the
               reason. An online sync failure surfaces solely in the header
               SyncIndicator, never duplicated inline. -->
          <p class="mt-4 max-w-[70ch] text-caption leading-relaxed text-muted">{copy.offline}</p>
        {/if}
        {#if emptyList}
          {@render emptyState(copy.emptyPage)}
        {/if}
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
              <!-- P05 bulk move: an action menu, not a stateful field — the
                   controlled null value keeps the trigger on its prompt. -->
              <Select.Root
                type="single"
                value={undefined}
                onValueChange={(value) => moveSelected(value === "" ? null : value)}
                items={folderItems}
              >
                <Select.Trigger size="sm" aria-label={copy.moveSelectedTo}>
                  <Select.Value placeholder={copy.moveSelectedTo} />
                </Select.Trigger>
                <Select.Content>
                  {#each sortedFolders as folder (folder.id)}
                    <Select.Item value={folder.id} label={folder.name}>{folder.name}</Select.Item>
                  {/each}
                </Select.Content>
              </Select.Root>
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

        {#snippet rowList(rows: readonly Bookmark[])}
          <ul class="divide-y divide-border">
            {#each rows as bookmark (bookmark.id)}
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
        {/snippet}

        {#if !emptyList}
          <div class="mt-6 flex flex-wrap items-center gap-x-5 gap-y-2">
            <span class="flex items-center gap-1.5">
              <span class="text-caption text-muted">{copy.sortLabel}</span>
              <Select.Root
                type="single"
                value={sortMode}
                onValueChange={(value) => {
                  // SAFETY: the only values the menu can emit are sortItems' own.
                  sortMode = value as SortMode;
                }}
                items={sortItems}
              >
                <Select.Trigger size="sm" aria-label={copy.sortLabel}>
                  <Select.Value />
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="date" label={copy.sortDate}>{copy.sortDate}</Select.Item>
                  <Select.Item value="folder" label={copy.sortFolder}>{copy.sortFolder}</Select.Item>
                  <Select.Item value="surah" label={copy.sortSurah}>{copy.sortSurah}</Select.Item>
                </Select.Content>
              </Select.Root>
            </span>
            <span class="flex items-center gap-1.5">
              <span class="text-caption text-muted">{copy.filterLabel}</span>
              <Select.Root
                type="single"
                value={folderFilter}
                onValueChange={(value) => {
                  folderFilter = value;
                }}
                items={filterItems}
              >
                <Select.Trigger size="sm" aria-label={copy.filterLabel}>
                  <Select.Value />
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="all" label={copy.filterAll}>{copy.filterAll}</Select.Item>
                  <Select.Item value="unfiled" label={copy.unfiled}>{copy.unfiled}</Select.Item>
                  {#each sortedFolders as folder (folder.id)}
                    <Select.Item value={folder.id} label={folder.name}>{folder.name}</Select.Item>
                  {/each}
                </Select.Content>
              </Select.Root>
            </span>
          </div>
        {/if}

        {#if sortMode === "folder"}
          <div class="mt-6 flex flex-col gap-6">
            {#each groups as group (group.id ?? "unfiled")}
              <section
                aria-labelledby="bookmarks-group-{group.id ?? `unfiled`}"
                class="overflow-hidden rounded-md border border-border-strong bg-surface"
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
                  {@render rowList(group.rows)}
                {/if}
              </section>
            {/each}
          </div>
        {:else if flatRows.length > 0}
          <div class="mt-6 overflow-hidden rounded-md border border-border-strong bg-surface">
            {@render rowList(flatRows)}
          </div>
        {/if}
      {/if}
    {:else}
      {#if anonCatalogPending}
        <div class="mt-6 flex flex-col gap-3" role="status">
          {@render loadingRows()}
        </div>
      {:else if anonRows.length === 0}
        {@render emptyState(copy.emptyPage)}
      {:else}
        <ul class="mt-6 divide-y divide-border overflow-hidden rounded-md border border-border-strong bg-surface">
          {#each anonRows as row (row.key)}
            <BookmarkRow
              {copy}
              bookmark={{ id: row.key, folderId: null, surah: row.surah, ayah: row.ayah, updatedAt: row.updatedAt }}
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
