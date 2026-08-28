<script lang="ts">
  import { Icon } from "$lib/components/icon";
  import { Input } from "$lib/components/ui/input";
  import type { BookmarksCopy } from "$lib/i18n/bookmarks-copy";
  import type { BookmarkFolder } from "$lib/bookmarks/schema";

  let {
    copy,
    folders,
    countIn,
    onCreate,
    onRename,
    onDelete,
  }: {
    copy: BookmarksCopy;
    folders: readonly BookmarkFolder[];
    countIn: (folderId: string) => number;
    onCreate: (name: string) => void;
    onRename: (id: string, name: string) => void;
    onDelete: (id: string) => void;
  } = $props();

  let newName = $state("");
  let renamingId = $state<string | null>(null);
  let renameValue = $state("");
  let confirmingId = $state<string | null>(null);

  const actionButton =
    "rounded-lg border border-border-strong px-2.5 py-1.5 text-[12.5px] text-foreground-secondary transition-colors hover:border-border hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring";
  const dangerButton =
    "rounded-lg border border-border-strong px-2.5 py-1.5 text-[12.5px] text-pop transition-colors hover:border-pop focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring";

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
    onRename(id, name);
    renamingId = null;
  }
</script>

<section class="mt-6" aria-labelledby="bookmarks-folders">
  <div class="flex items-baseline justify-between gap-3">
    <h2 id="bookmarks-folders" class="text-[17px] font-semibold tracking-[-0.02em] text-foreground"
      >{copy.foldersHeading}</h2
    >
  </div>

  <form
    class="mt-3 flex max-w-md items-center gap-2"
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
      class="h-9 rounded-lg border-border-strong bg-surface text-caption text-foreground"
    />
    <button
      type="submit"
      disabled={newName.trim().length === 0}
      class="inline-flex h-9 items-center gap-1.5 rounded-lg border border-border-strong px-3 text-[13px] font-medium text-foreground-secondary transition-colors hover:border-border hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring disabled:pointer-events-none disabled:opacity-40"
    >
      <Icon name="plus" size={14} />
      {copy.createFolder}
    </button>
  </form>

  {#if folders.length > 0}
    <ul class="mt-3 divide-y divide-border overflow-hidden rounded-xl border border-border-strong bg-surface">
      {#each folders as folder (folder.id)}
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
            <span class="inline-flex min-w-0 flex-1 items-center gap-2 text-[14px] text-foreground">
              <Icon name="rows" size={14} class="shrink-0 text-muted" />
              <span class="truncate">{folder.name}</span>
              <span class="shrink-0 text-[12px] tabular-nums text-muted"
                >{countIn(folder.id)}</span
              >
            </span>
            <span class="flex items-center gap-1.5">
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
