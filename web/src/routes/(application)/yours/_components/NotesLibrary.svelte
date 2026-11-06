<script lang="ts">
  import { toast } from "svelte-sonner";
  import type { QuranData } from "#lib/data/quran-data.js";
  import type { VerseKey } from "#lib/data/quran.js";
  import { surahRouteContext } from "#lib/data/quran.js";
  import { appLocale } from "#lib/i18n/app-locale.js";
  import { peekTranslationName } from "#lib/quran/catalogue.js";
  import { openVerseAt } from "#lib/reader/resume.js";
  import { reader } from "#lib/stores/reader.svelte.js";
  import type { NoteEntry, NoteSnapshot } from "#lib/stores/annotations.svelte.js";
  import { filterNotes, NOTE_SORTS, sortNotes, type NoteSort } from "#lib/home/notes-library.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import { Textarea } from "#lib/components/ui/textarea/index.js";
  import * as Select from "#lib/components/ui/select/index.js";
  import {
    reader_bookmarks_ayah,
    reader_bookmarks_undo,
    reader_note_placeholder,
    reader_notes_cancel,
    reader_notes_delete,
    reader_notes_device,
    reader_notes_edit,
    reader_notes_edited,
    reader_notes_empty_search,
    reader_notes_removed,
    reader_notes_save,
    reader_notes_search_label,
    reader_notes_search_placeholder,
    reader_notes_source_arabic,
    reader_notes_sort_label,
    reader_notes_sort_newest,
    reader_notes_sort_oldest,
    reader_notes_sort_verse,
    reader_widget_notes_empty,
    reader_your_note,
  } from "#lib/i18n/m/reader.js";

  let { quranData }: { quranData: QuranData | undefined } = $props();

  // Personal app routes carry no translation segments, so the Arabic reader is the
  // fallback context; a note opens in its own translation when that still exists.
  const ARABIC_CTX = surahRouteContext("uthmani");
  const locale = appLocale();
  const msgOptions = { locale };
  const dateFormat = new Intl.DateTimeFormat(locale, { dateStyle: "medium" });

  let query = $state("");
  let sort = $state<NoteSort>("verse");
  let editingKey = $state<VerseKey | null>(null);
  let draft = $state("");

  function surahName(surah: number): string {
    return quranData?.surahByNum(surah)?.name ?? `Surah ${surah}`;
  }

  function sortLabel(mode: NoteSort): string {
    if (mode === "newest") return reader_notes_sort_newest(undefined, msgOptions);
    if (mode === "oldest") return reader_notes_sort_oldest(undefined, msgOptions);
    return reader_notes_sort_verse(undefined, msgOptions);
  }

  const sortItems = $derived(NOTE_SORTS.map((mode) => ({ value: mode, label: sortLabel(mode) })));

  // Reactive: re-derives on every note write, owner switch or cross-tab change.
  const allNotes = $derived(reader.noteEntries);
  const visible = $derived(sortNotes(filterNotes(allNotes, query, surahName), sort));

  function sourceLabel(note: NoteEntry): string {
    return peekTranslationName(note.sourceId ?? undefined) ?? reader_notes_source_arabic(undefined, msgOptions);
  }

  function editedLabel(note: NoteEntry): string | null {
    if (note.updatedAt === null) return null;
    return reader_notes_edited({ date: dateFormat.format(new Date(note.updatedAt)) }, msgOptions);
  }

  function openNote(note: NoteEntry): void {
    void openVerseAt(note.surah, note.ayah, note.sourceId ?? undefined, ARABIC_CTX);
  }

  function startEdit(note: NoteEntry): void {
    editingKey = note.key;
    draft = note.text;
  }

  function cancelEdit(): void {
    editingKey = null;
  }

  // Empty text is a delete: it goes through the same undoable path as the button.
  function saveEdit(note: NoteEntry): void {
    if (draft.trim() === "") {
      removeNote(note);
    } else {
      reader.setNote(note.key, draft, note.sourceId);
    }
    editingKey = null;
  }

  // Optimistic removal with one Undo toast; Undo restores the text and original timestamp.
  function removeNote(note: NoteEntry): void {
    const snapshot: NoteSnapshot | null = reader.removeNote(note.key);
    if (snapshot === null) return;
    if (editingKey === note.key) editingKey = null;
    toast(reader_notes_removed(undefined, msgOptions), {
      id: "notes-feedback",
      action: {
        label: reader_bookmarks_undo(undefined, msgOptions),
        onClick: () => reader.restoreNote(note.key, snapshot),
      },
    });
  }
</script>

<section aria-label={reader_your_note(undefined, msgOptions)} class="mt-5">
  <div class="flex flex-wrap items-center gap-2">
    <Input
      type="search"
      bind:value={query}
      placeholder={reader_notes_search_placeholder(undefined, msgOptions)}
      aria-label={reader_notes_search_label(undefined, msgOptions)}
      class="h-9 min-w-0 flex-1"
    />
    <Select.Root
      type="single"
      value={sort}
      onValueChange={(value) => {
        sort = NOTE_SORTS.find((mode) => mode === value) ?? "verse";
      }}
      items={sortItems}
    >
      <Select.Trigger size="sm" aria-label={reader_notes_sort_label(undefined, msgOptions)}>
        <Select.Value />
      </Select.Trigger>
      <Select.Content>
        {#each sortItems as item (item.value)}
          <Select.Item value={item.value} label={item.label}>{item.label}</Select.Item>
        {/each}
      </Select.Content>
    </Select.Root>
  </div>

  {#if allNotes.length === 0}
    <p class="mt-3 text-[13px] text-muted">{reader_widget_notes_empty(undefined, msgOptions)}</p>
  {:else if visible.length === 0}
    <p class="mt-3 text-[13px] text-muted">{reader_notes_empty_search(undefined, msgOptions)}</p>
  {:else}
    <ul class="mt-3 divide-y divide-border overflow-hidden rounded-md border border-border bg-surface">
      {#each visible as note (note.key)}
        <li class="px-4 py-3">
          <div class="flex items-start gap-3">
            <div class="min-w-0 flex-1">
              <button
                type="button"
                class="block w-full text-start"
                onclick={() => openNote(note)}
              >
                <span class="block truncate text-sm font-medium text-foreground">
                  {surahName(note.surah)}
                  <span class="font-normal text-muted"
                    >· {reader_bookmarks_ayah({ ayah: note.ayah }, msgOptions)}</span
                  >
                </span>
                <span class="mt-0.5 block text-[12px] text-muted">
                  {sourceLabel(note)}{#if editedLabel(note) !== null}
                    · {editedLabel(note)}{/if}
                </span>
              </button>

              {#if editingKey === note.key}
                <div class="mt-2 flex flex-col gap-2">
                  <Textarea
                    bind:value={draft}
                    rows={3}
                    aria-label={reader_your_note(undefined, msgOptions)}
                    placeholder={reader_note_placeholder(undefined, msgOptions)}
                    class="resize-y rounded-md border-border bg-background px-3.5 py-3 text-[14.5px] leading-[1.6] text-foreground"
                  />
                  <div class="flex items-center gap-2">
                    <Button size="sm" onclick={() => saveEdit(note)}>
                      {reader_notes_save(undefined, msgOptions)}
                    </Button>
                    <Button variant="ghost" size="sm" onclick={cancelEdit}>
                      {reader_notes_cancel(undefined, msgOptions)}
                    </Button>
                  </div>
                </div>
              {:else}
                <p class="mt-1.5 line-clamp-3 whitespace-pre-line text-[14px] leading-relaxed text-foreground-secondary">
                  {note.text}
                </p>
              {/if}
            </div>

            {#if editingKey !== note.key}
              <div class="flex flex-none items-center gap-1">
                <Button variant="ghost" size="sm" onclick={() => startEdit(note)}>
                  {reader_notes_edit(undefined, msgOptions)}
                </Button>
                <Button variant="ghost" size="sm" class="text-danger" onclick={() => removeNote(note)}>
                  {reader_notes_delete(undefined, msgOptions)}
                </Button>
              </div>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  <p class="mt-3 text-[11px] text-muted">{reader_notes_device(undefined, msgOptions)}</p>
</section>
