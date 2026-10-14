<script lang="ts">
  import { bookmarks } from "#lib/bookmarks/store.svelte.js";
  import { saveToPicker } from "#lib/bookmarks/feedback.svelte.js";
  import { peekQuranData } from "#lib/data/quran-data-client.js";
  import { getBookmarksCopy } from "#lib/i18n/bookmarks-copy.js";
  import { Badge } from "#lib/components/ui/badge/index.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Checkbox } from "#lib/components/ui/checkbox/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import { Label } from "#lib/components/ui/label/index.js";
  import * as Sheet from "#lib/components/ui/sheet/index.js";

  // "Save to…" picker: one verse, every folder as a checkbox (a verse may sit
  // in many folders), plus an inline "New folder" that also files the verse.
  const copy = getBookmarksCopy();
  let newName = $state("");

  const target = $derived(saveToPicker.target);
  const inFolders = $derived(
    target === null ? new Set<string>() : bookmarks.foldersOf(target.surah, target.ayah),
  );
  const reference = $derived.by(() => {
    if (target === null) return "";
    const name = peekQuranData()?.surahByNum(target.surah)?.name ?? `${target.surah}`;
    return `${name} · ${copy.ayah(target.ayah)}`;
  });

  function setIn(folderId: string, on: boolean): void {
    if (target === null) return;
    bookmarks.setInFolder(target.surah, target.ayah, folderId, on, target.sourceId);
  }

  function addFolder(event: SubmitEvent): void {
    event.preventDefault();
    if (target === null) return;
    const folder = bookmarks.createFolder(newName);
    if (folder === null) return;
    setIn(folder.id, true);
    newName = "";
  }
</script>

<Sheet.Root
  open={target !== null}
  onOpenChange={(open) => {
    if (!open) saveToPicker.close();
  }}
>
  <Sheet.Content side="bottom" class="mx-auto w-full max-w-lg rounded-t-xl px-5 pb-6">
    <Sheet.Header class="px-0">
      <Sheet.Title>{copy.saveTo}</Sheet.Title>
      <Sheet.Description>{reference}</Sheet.Description>
    </Sheet.Header>
    <ul class="flex max-h-[45vh] flex-col gap-1 overflow-y-auto">
      {#each bookmarks.folders as folder (folder.id)}
        <li>
          <Label
            for="save-to-{folder.id}"
            class="flex min-h-11 cursor-pointer items-center gap-3 rounded-md px-2 hover:bg-surface-hover"
          >
            <Checkbox
              id="save-to-{folder.id}"
              checked={inFolders.has(folder.id)}
              onCheckedChange={(checked) => setIn(folder.id, checked)}
            />
            <span class="min-w-0 flex-1 truncate text-sm text-foreground">{folder.name}</span>
            {#if folder.isDefault}
              <Badge variant="secondary">{copy.defaultBadge}</Badge>
            {/if}
            <span class="text-xs text-muted-foreground">{bookmarks.countIn(folder.id)}</span>
          </Label>
        </li>
      {/each}
    </ul>
    <form class="flex items-center gap-2" onsubmit={addFolder}>
      <Label for="save-to-new" class="sr-only">{copy.newFolderLabel}</Label>
      <Input
        id="save-to-new"
        bind:value={newName}
        maxlength={100}
        placeholder={copy.newFolder}
        class="flex-1"
      />
      <Button type="submit" variant="secondary" disabled={newName.trim().length === 0}>
        {copy.createFolder}
      </Button>
    </form>
  </Sheet.Content>
</Sheet.Root>
