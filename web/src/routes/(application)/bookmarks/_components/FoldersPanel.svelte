<script lang="ts">
  import { Icon } from "#lib/components/icon/index.js";
  import * as AlertDialog from "#lib/components/ui/alert-dialog/index.js";
  import { Badge } from "#lib/components/ui/badge/index.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import * as DropdownMenu from "#lib/components/ui/dropdown-menu/index.js";
  import { iconButtonVariants } from "#lib/components/ui/icon-button/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import { Label } from "#lib/components/ui/label/index.js";
  import type { BookmarksCopy } from "#lib/i18n/bookmarks-copy.js";
  import type { BookmarkFolder } from "#lib/bookmarks/schema.js";

  // Folders: default first (never deletable), then manual order. Row actions
  // sit in one menu; deleting asks once, with the count, and the page offers
  // Undo right after.
  let {
    copy,
    folders,
    countIn,
    onCreate,
    onRename,
    onDelete,
    onReorder,
  }: {
    copy: BookmarksCopy;
    folders: readonly BookmarkFolder[];
    countIn: (folderId: string) => number;
    onCreate: (name: string) => void;
    onRename: (id: string, name: string) => void;
    onDelete: (id: string) => void;
    onReorder: (id: string, direction: -1 | 1) => void;
  } = $props();

  let newName = $state("");
  let renamingId = $state<string | null>(null);
  let renameValue = $state("");
  let deleting = $state<BookmarkFolder | null>(null);

  // The default folder pins the head of the list; manual order covers the rest.
  const movable = $derived(folders.filter((folder) => !folder.isDefault));

  function submitCreate(event: SubmitEvent): void {
    event.preventDefault();
    if (newName.trim().length === 0) return;
    onCreate(newName);
    newName = "";
  }

  function startRename(folder: BookmarkFolder): void {
    renamingId = folder.id;
    renameValue = folder.name;
  }

  function submitRename(event: SubmitEvent, id: string): void {
    event.preventDefault();
    if (renameValue.trim().length === 0) return;
    onRename(id, renameValue);
    renamingId = null;
  }

  function confirmDelete(): void {
    if (deleting !== null) onDelete(deleting.id);
    deleting = null;
  }
</script>

<section class="mt-6" aria-labelledby="bookmarks-folders">
  <h2 id="bookmarks-folders" class="text-[17px] font-medium tracking-[-0.02em] text-foreground">
    {copy.foldersHeading}
  </h2>

  <form class="mt-3 flex items-center gap-2" onsubmit={submitCreate}>
    <Label class="sr-only" for="bookmarks-new-folder">{copy.newFolderLabel}</Label>
    <Input
      id="bookmarks-new-folder"
      bind:value={newName}
      maxlength={100}
      placeholder={copy.newFolderPlaceholder}
      class="flex-1"
    />
    <Button type="submit" variant="secondary" size="sm" disabled={newName.trim().length === 0}>
      <Icon name="plus" size={14} />
      {copy.createFolder}
    </Button>
  </form>

  <ul class="mt-3 divide-y divide-border overflow-hidden rounded-md border border-border-strong bg-surface">
    {#each folders as folder (folder.id)}
      {@const slot = movable.findIndex((entry) => entry.id === folder.id)}
      <li class="flex min-h-14 items-center gap-3 px-4 py-2 sm:px-5">
        {#if renamingId === folder.id}
          <form class="flex min-w-0 flex-1 items-center gap-2" onsubmit={(e) => submitRename(e, folder.id)}>
            <Label class="sr-only" for="bookmarks-rename-{folder.id}">{copy.renameLabel}</Label>
            <Input id="bookmarks-rename-{folder.id}" bind:value={renameValue} maxlength={100} class="flex-1" />
            <Button type="submit" size="sm">{copy.save}</Button>
            <Button type="button" variant="ghost" size="sm" onclick={() => (renamingId = null)}>
              {copy.cancel}
            </Button>
          </form>
        {:else}
          <Icon name="bookmark" size={15} class="shrink-0 text-muted" />
          <span class="min-w-0 flex-1 truncate text-[14px] text-foreground">{folder.name}</span>
          {#if folder.isDefault}
            <Badge variant="secondary">{copy.defaultBadge}</Badge>
          {/if}
          <span class="shrink-0 text-[12px] tabular-nums text-muted">{countIn(folder.id)}</span>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger
              aria-label={folder.name}
              class={iconButtonVariants({ variant: "ghost", size: "sm" })}
            >
              <Icon name="dots" size={18} />
            </DropdownMenu.Trigger>
            <DropdownMenu.Content align="end" sideOffset={4}>
              <DropdownMenu.Group>
                <DropdownMenu.Item onSelect={() => startRename(folder)}>{copy.rename}</DropdownMenu.Item>
                {#if !folder.isDefault}
                  <DropdownMenu.Item disabled={slot <= 0} onSelect={() => onReorder(folder.id, -1)}>
                    {copy.moveUpLabel(folder.name)}
                  </DropdownMenu.Item>
                  <DropdownMenu.Item
                    disabled={slot === -1 || slot === movable.length - 1}
                    onSelect={() => onReorder(folder.id, 1)}
                  >
                    {copy.moveDownLabel(folder.name)}
                  </DropdownMenu.Item>
                {/if}
              </DropdownMenu.Group>
              {#if !folder.isDefault}
                <DropdownMenu.Separator />
                <DropdownMenu.Group>
                  <DropdownMenu.Item variant="destructive" onSelect={() => (deleting = folder)}>
                    {copy.deleteFolder}
                  </DropdownMenu.Item>
                </DropdownMenu.Group>
              {/if}
            </DropdownMenu.Content>
          </DropdownMenu.Root>
        {/if}
      </li>
    {/each}
  </ul>
</section>

<AlertDialog.Root
  open={deleting !== null}
  onOpenChange={(open) => {
    if (!open) deleting = null;
  }}
>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>
        {deleting === null ? "" : copy.deleteFolderConfirm(deleting.name, countIn(deleting.id))}
      </AlertDialog.Title>
      <AlertDialog.Description>{copy.deleteFolderBody}</AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>{copy.cancel}</AlertDialog.Cancel>
      <AlertDialog.Action onclick={confirmDelete}>{copy.deleteFolder}</AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
