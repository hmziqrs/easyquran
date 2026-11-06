<script lang="ts">
  import ArrowDownIcon from "@lucide/svelte/icons/arrow-down";
  import ArrowUpIcon from "@lucide/svelte/icons/arrow-up";
  import { bookmarks } from "#lib/bookmarks/store.svelte.js";
  import { parseVerseKey } from "#lib/bookmarks/schema.js";
  import { TRANSLATION_CATALOGUE_BY_ID } from "#lib/quran/catalogue.js";
  import { ARABIC_SELECTION, compareCell, compareSelections, pins } from "#lib/stores/pins.svelte.js";
  import { Icon } from "#lib/components/icon/index.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import {
    Sheet,
    SheetContent,
    SheetDescription,
    SheetFooter,
    SheetHeader,
    SheetTitle,
  } from "#lib/components/ui/sheet/index.js";
  import { appLocale } from "#lib/i18n/app-locale.js";
  import {
    reader_pins_arabic_option,
    reader_pins_compare_with,
    reader_pins_description,
    reader_pins_empty,
    reader_pins_folder_name,
    reader_pins_move_down,
    reader_pins_move_up,
    reader_pins_no_arabic,
    reader_pins_no_text,
    reader_pins_remove,
    reader_pins_save_folder,
    reader_pins_saved,
    reader_pins_title,
    reader_pins_trigger,
    reader_pins_trigger_count,
  } from "#lib/i18n/m/reader.js";

  const locale = { locale: appLocale() };
  let open = $state(false);
  let selection = $state(ARABIC_SELECTION);
  let savedCount = $state<number | null>(null);

  const selections = $derived(compareSelections(pins.list));
  // A selection whose translation was unpinned falls back to Arabic instead of going blank.
  const activeSelection = $derived(
    selections.includes(selection) ? selection : ARABIC_SELECTION,
  );

  function onSelectionChange(event: Event): void {
    // SAFETY: onchange is bound only to the compare <select>, so currentTarget is that element.
    selection = (event.currentTarget as HTMLSelectElement).value;
  }

  function selectionLabel(id: string): string {
    if (id === ARABIC_SELECTION) return reader_pins_arabic_option(undefined, locale);
    const catalogue = TRANSLATION_CATALOGUE_BY_ID.get(id);
    if (catalogue !== undefined) return catalogue.name;
    for (const pin of pins.list) {
      const lane = pin.lanes.find((l) => l.sourceId === id);
      if (lane !== undefined && lane.name !== "") return lane.name;
    }
    return id;
  }

  function directionFor(id: string): "ltr" | "rtl" {
    return TRANSLATION_CATALOGUE_BY_ID.get(id)?.direction === "rtl" ? "rtl" : "ltr";
  }

  function languageFor(id: string): string {
    return TRANSLATION_CATALOGUE_BY_ID.get(id)?.languageCode ?? "en";
  }

  // Bookmark folder from the pinned set: one folder, each verse saved with the translation
  // it was pinned in. Pins themselves stay where they are.
  function saveAsFolder(): void {
    const folder = bookmarks.createFolder(reader_pins_folder_name(undefined, locale));
    if (folder === null) return;
    let saved = 0;
    for (const pin of pins.list) {
      const coords = parseVerseKey(pin.key);
      if (coords === null) continue;
      bookmarks.setInFolder(coords.surah, coords.ayah, folder.id, true, pin.sourceId);
      saved += 1;
    }
    savedCount = saved;
  }
</script>

{#if pins.count > 0}
  <Button
    variant="ghost"
    size="sm"
    class="flex-none gap-1.5"
    title={reader_pins_trigger(undefined, locale)}
    aria-label={reader_pins_trigger_count({ count: pins.count }, locale)}
    onclick={() => (open = true)}
    data-pins-trigger
  >
    <Icon name="pin" size={15} />
    <span class="tabular-nums">{pins.count}</span>
  </Button>
{/if}

<Sheet bind:open>
  <SheetContent side="bottom" class="max-h-[88dvh] overflow-y-auto" data-pins-sheet>
    <SheetHeader>
      <SheetTitle>{reader_pins_title(undefined, locale)}</SheetTitle>
      <SheetDescription>{reader_pins_description(undefined, locale)}</SheetDescription>
    </SheetHeader>

    {#if pins.count === 0}
      <p class="px-4 pb-4 text-sm text-foreground-secondary">{reader_pins_empty(undefined, locale)}</p>
    {:else}
      <ol class="flex flex-col gap-1 px-4" data-pins-list>
        {#each pins.list as pin, index (pin.key)}
          <li class="flex items-center gap-2 rounded-md border border-border px-3 py-2">
            <span class="min-w-0 flex-1 truncate text-sm font-medium tabular-nums">{pin.key}</span>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={reader_pins_move_up(undefined, locale)}
              disabled={index === 0}
              onclick={() => pins.move(pin.key, -1)}
            >
              <ArrowUpIcon size={12} />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={reader_pins_move_down(undefined, locale)}
              disabled={index === pins.count - 1}
              onclick={() => pins.move(pin.key, 1)}
            >
              <ArrowDownIcon size={12} />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={reader_pins_remove(undefined, locale)}
              onclick={() => pins.remove(pin.key)}
            >
              <Icon name="x" size={13} />
            </Button>
          </li>
        {/each}
      </ol>

      <div class="flex flex-col gap-3 px-4 pt-4">
        <label class="flex items-center justify-between gap-3 text-sm">
          <span class="font-medium">{reader_pins_compare_with(undefined, locale)}</span>
          <select
            value={activeSelection}
            onchange={onSelectionChange}
            aria-label={reader_pins_compare_with(undefined, locale)}
            class="h-9 max-w-[60%] rounded-md border border-border bg-background px-2 text-sm"
            data-pins-selection
          >
            {#each selections as id (id)}
              <option value={id}>{selectionLabel(id)}</option>
            {/each}
          </select>
        </label>

        <!-- Phones stack the verses; wider screens lay them out in columns and scroll
             sideways only when the pins do not fit. -->
        <div class="pins-compare" data-pins-compare>
          {#each pins.list as pin (pin.key)}
            {@const cell = compareCell(pin, activeSelection)}
            <article class="flex min-w-0 flex-col gap-2 rounded-md border border-border p-3" data-pin-cell={pin.key}>
              <span class="text-xs font-medium tabular-nums text-foreground-secondary">{pin.key}</span>
              {#if cell.arabic !== null}
                <p dir="rtl" lang="ar" class="font-arabic text-lg leading-[2.1] text-quran-foreground">
                  {cell.arabic}
                </p>
              {:else if activeSelection === ARABIC_SELECTION}
                <p class="text-sm text-muted-foreground">{reader_pins_no_arabic(undefined, locale)}</p>
              {/if}
              {#if activeSelection !== ARABIC_SELECTION}
                {#if cell.translation !== null}
                  <p
                    dir={directionFor(activeSelection)}
                    lang={languageFor(activeSelection)}
                    class="text-[15px] leading-relaxed text-foreground"
                  >
                    {cell.translation}
                  </p>
                {:else}
                  <p class="text-sm text-muted-foreground">{reader_pins_no_text(undefined, locale)}</p>
                {/if}
              {/if}
            </article>
          {/each}
        </div>
      </div>
    {/if}

    <SheetFooter>
      {#if savedCount !== null}
        <p role="status" class="text-sm text-foreground-secondary">
          {reader_pins_saved({ count: savedCount }, locale)}
        </p>
      {/if}
      {#if pins.count > 0}
        <Button variant="secondary" onclick={saveAsFolder} data-pins-save>
          {reader_pins_save_folder(undefined, locale)}
        </Button>
      {/if}
    </SheetFooter>
  </SheetContent>
</Sheet>

<style>
  .pins-compare {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  @media (min-width: 768px) {
    .pins-compare {
      display: grid;
      grid-auto-columns: minmax(16rem, 1fr);
      grid-auto-flow: column;
      overflow-x: auto;
    }
  }
</style>
