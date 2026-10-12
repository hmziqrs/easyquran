<script lang="ts">
  import { Icon } from "#lib/components/icon/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import type { BookmarksCopy } from "#lib/i18n/bookmarks-copy.js";
  import {
    FOLDER_KINDS,
    type BookmarkFolder,
    type BookmarkFolderKind,
  } from "#lib/bookmarks/schema.js";

  let {
    copy,
    folders,
    countIn,
    progressIn,
    onCreate,
    onUpdate,
    onDelete,
    onReorder,
  }: {
    copy: BookmarksCopy;
    /** Already ordered: position (NULL last) then name (Feature 2.3). */
    folders: readonly BookmarkFolder[];
    countIn: (folderId: string) => number;
    /** Daily-folder progress (Feature 2.3): done/total against the active session; null = nothing to show. */
    progressIn: (folderId: string) => { done: number; total: number } | null;
    onCreate: (name: string) => void;
    onUpdate: (id: string, patch: { name?: string; kind?: BookmarkFolderKind; position?: number | null }) => void;
    onDelete: (id: string) => void;
    onReorder: (id: string, direction: -1 | 1) => void;
  } = $props();

  let newName = $state("");
  let renamingId = $state<string | null>(null);
  let renameValue = $state("");
  let confirmingId = $state<string | null>(null);

  const actionButton =
    "rounded-lg border border-border-strong px-2.5 py-1.5 text-[12.5px] text-foreground-secondary transition-colors hover:border-border hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring";
  /* Scope-extension (run 3): the destructive control was text-pop/hover:border-pop
     (--accent = hue-2 emerald → green). Danger status tokens, matching the settings
     storage-action grammar (border-danger + text-danger). */
  const dangerButton =
    "rounded-lg border border-border-strong px-2.5 py-1.5 text-[12.5px] text-danger transition-colors hover:border-danger focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring";
  const kindSelect =
    "h-8 rounded-lg border border-border-strong bg-surface px-2 text-[12.5px] text-foreground-secondary transition-colors hover:border-border hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring";

  function submitCreate(): void {
    const name = newName;
    if (name.trim().length === 0) return;
    onCreate(name);
    newName = "";
  }

  function startRename(folder: BookmarkFolder): void {
    confirmingId = null;
    renamingId = folder.id;
    renameValue = folder.name;
  }

  function submitRename(id: string): void {
    const name = renameValue;
    if (name.trim().length === 0) return;
    onUpdate(id, { name: name.trim() });
    renamingId = null;
  }
</script>

<section class="mt-6" aria-labelledby="bookmarks-folders">
  <div class="flex items-baseline justify-between gap-3">
    <h2 id="bookmarks-folders" class="text-[17px] font-medium tracking-[-0.02em] text-foreground"
      >{copy.foldersHeading}</h2
    >
  </div>

  <!-- ACCT-04: the new-folder field spans the panel's full width, Add stays a pill. -->
  <form
    class="mt-3 flex items-center gap-2"
    onsubmit={(event) => {
      event.preventDefault();
      submitCreate();
    }}
  >
    <label class="sr-only" for="bookmarks-new-folder">{copy.newFolderLabel}</label>
    <Input
      id="bookmarks-new-folder"
      bind:value={newName}
      maxlength={100}
      placeholder={copy.newFolderPlaceholder}
      class="h-9 flex-1 rounded-lg border-border-strong bg-surface text-caption text-foreground"
    />
    <button
      type="submit"
      disabled={newName.trim().length === 0}
      class="inline-flex h-9 items-center gap-1.5 rounded-pill border border-border-strong px-3 text-[13px] font-medium text-foreground-secondary transition-colors hover:border-border hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring disabled:pointer-events-none disabled:opacity-40"
    >
      <Icon name="plus" size={14} />
      {copy.createFolder}
    </button>
  </form>

  {#if folders.length > 0}
    <ul class="mt-3 divide-y divide-border overflow-hidden rounded-md border border-border-strong bg-surface">
      {#each folders as folder, index (folder.id)}
        <li class="flex flex-wrap items-center gap-x-3 gap-y-2 px-4 py-3 sm:px-5">
          {#if renamingId === folder.id}
            <form
              class="flex min-w-0 flex-1 items-center gap-2"
              onsubmit={(event) => {
                event.preventDefault();
                submitRename(folder.id);
              }}
            >
              <label class="sr-only" for="bookmarks-rename-{folder.id}">{copy.renameLabel}</label>
              <Input
                id="bookmarks-rename-{folder.id}"
                bind:value={renameValue}
                maxlength={100}
                class="h-8 max-w-56 rounded-lg border-border-strong bg-surface text-caption text-foreground"
              />
              <button type="submit" class={actionButton}>{copy.save}</button>
              <button
                type="button"
                class={actionButton}
                onclick={() => (renamingId = null)}>{copy.cancel}</button
              >
            </form>
          {:else if confirmingId === folder.id}
            <span class="min-w-0 flex-1 text-caption text-foreground-secondary"
              >{copy.deleteFolderConfirm(folder.name)}</span
            >
            <button type="button" class={dangerButton} onclick={() => onDelete(folder.id)}
              >{copy.deleteFolder}</button
            >
            <button
              type="button"
              class={actionButton}
              onclick={() => (confirmingId = null)}>{copy.cancel}</button
            >
          {:else}
            <span class="flex items-center gap-1">
              <button
                type="button"
                class={actionButton}
                disabled={index === 0}
                aria-label={copy.moveUpLabel(folder.name)}
                onclick={() => onReorder(folder.id, -1)}
              >
                <Icon name="rows" size={13} class="rotate-180" />
              </button>
              <button
                type="button"
                class={actionButton}
                disabled={index === folders.length - 1}
                aria-label={copy.moveDownLabel(folder.name)}
                onclick={() => onReorder(folder.id, 1)}
              >
                <Icon name="rows" size={13} />
              </button>
            </span>
            <span class="inline-flex min-w-0 flex-1 items-center gap-2 text-[14px] text-foreground">
              <span class="truncate">{folder.name}</span>
              <span class="shrink-0 text-[12px] tabular-nums text-muted"
                >{countIn(folder.id)}</span
              >
              {#if folder.kind === "daily"}
                {@const progress = progressIn(folder.id)}
                {#if progress !== null}
                  <!-- Daily study-set progress (Feature 2.3): read share of the
                       folder's verses per the active reading session. -->
                  <span
                    class="flex min-w-[120px] flex-1 items-center gap-2"
                    title={copy.folderProgress(progress.done, progress.total)}
                  >
                    <span
                      class="h-1.5 flex-1 overflow-hidden rounded-pill bg-surface-hover"
                      role="progressbar"
                      aria-valuemin={0}
                      aria-valuemax={progress.total}
                      aria-valuenow={progress.done}
                      aria-label="{folder.name}: {copy.folderProgress(progress.done, progress.total)}"
                    >
                      <span
                        class="block h-full rounded-pill bg-primary"
                        style:width="{Math.round((progress.done / progress.total) * 100)}%"></span>
                    </span>
                    <span class="shrink-0 text-[11.5px] tabular-nums text-muted"
                      >{progress.done}/{progress.total}</span
                    >
                  </span>
                {/if}
              {/if}
            </span>
            <span class="flex items-center gap-1.5">
              <label class="sr-only" for="bookmarks-kind-{folder.id}"
                >{copy.folderKindLabel}: {folder.name}</label
              >
              <select
                id="bookmarks-kind-{folder.id}"
                value={folder.kind}
                onchange={(event) => {
                  const value = (event.currentTarget as HTMLSelectElement)
                    .value as BookmarkFolderKind;
                  onUpdate(folder.id, { kind: value });
                }}
                class={kindSelect}
              >
                {#each FOLDER_KINDS as kind (kind)}
                  <option value={kind}>{copy.folderKind(kind)}</option>
                {/each}
              </select>
              <button type="button" class={actionButton} onclick={() => startRename(folder)}
                >{copy.rename}</button
              >
              <button
                type="button"
                class={actionButton}
                onclick={() => (confirmingId = folder.id)}>{copy.deleteFolder}</button
              >
            </span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>
