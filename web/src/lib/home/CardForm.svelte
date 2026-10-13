<script lang="ts">
  import { appLocale } from "#lib/i18n/app-locale.js";
  import {
    reader_cards_add_title,
    reader_cards_ayah_number,
    reader_cards_cancel,
    reader_cards_collection_add,
    reader_cards_collection_empty,
    reader_cards_collection_full,
    reader_cards_collection_items,
    reader_cards_collection_ref,
    reader_cards_collection_snippet,
    reader_cards_edit_title,
    reader_cards_full,
    reader_cards_invalid_ayah,
    reader_cards_invalid_range,
    reader_cards_item_down,
    reader_cards_item_kind,
    reader_cards_item_remove,
    reader_cards_item_up,
    reader_cards_kind_locked,
    reader_cards_needs_surah,
    reader_cards_pick_snippet,
    reader_cards_pick_surah,
    reader_cards_range_end,
    reader_cards_range_start,
    reader_cards_save,
    reader_cards_schedule_invalid,
    reader_cards_target_label,
    reader_cards_translation,
    reader_cards_translation_arabic,
    reader_cards_zikr_text,
  } from "#lib/i18n/m/reader.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import {
    CARD_KINDS,
    MAX_COLLECTION_ITEMS,
    homeCards,
    newCardId,
    nowStamp,
    type CardKind,
    type CollectionItem,
    type HomeCard,
  } from "#lib/home/cards.svelte.js";
  import { cardKindLabel } from "#lib/home/card-copy.js";
  import ScheduleEditor from "#lib/home/ScheduleEditor.svelte";
  import { parseTimeHHMM, type ScheduleSlot } from "#lib/home/schedule.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import { suggestSurahs } from "#lib/search/page/surah-suggest.js";
  import type { SurahSuggestion } from "#lib/search/page/types.js";
  import { SNIPPETS } from "#lib/data/snippets.js";
  import { TRANSLATION_CATALOGUE } from "#lib/quran/catalogue.js";

  /**
   * The one add/edit card form (extracted from /yours, extended with the
   * collection multi-item builder and the schedule editor). Add mode builds a
   * fresh card via homeCards.add; edit mode prefiles from `initial` and saves
   * through homeCards.replace — the id-keyed upsert keeps count and the event
   * ring untouched. The kind never changes after creation (count/events/
   * schedule belong to the card identity), so edit-mode kind chips are inert:
   * aria-disabled but focusable, with an on-screen hint explaining why.
   * Validation errors are per-field strings resolved at submit time.
   */
  interface Props {
    quranData: QuranData | undefined;
    /** Edit mode: the card being edited; absent = add mode. */
    initial?: HomeCard;
    /** Cancel and successful Save both close the form. */
    onClose: () => void;
  }

  let { quranData, initial, onClose }: Props = $props();

  const locale = appLocale();
  const msgOptions = { locale };

  // The /yours section keys this form per card, so props never change
  // mid-mount; snapshot the edit target and the quran data once for the
  // prefill below (closure reads, not component-init references).
  interface FormSeed {
    card: HomeCard | undefined;
    data: QuranData | undefined;
  }
  function readSeed(): FormSeed {
    return {
      card: initial === undefined ? undefined : $state.snapshot(initial),
      data: quranData,
    };
  }
  const seed = readSeed();
  const editing = seed.card !== undefined;
  const initialCard = seed.card;

  const h = {
    addTitle: reader_cards_add_title(undefined, msgOptions),
    editTitle: reader_cards_edit_title(undefined, msgOptions),
    save: reader_cards_save(undefined, msgOptions),
    cancel: reader_cards_cancel(undefined, msgOptions),
    kindLocked: reader_cards_kind_locked(undefined, msgOptions),
    pickSurah: reader_cards_pick_surah(undefined, msgOptions),
    ayahNumber: reader_cards_ayah_number(undefined, msgOptions),
    rangeStart: reader_cards_range_start(undefined, msgOptions),
    rangeEnd: reader_cards_range_end(undefined, msgOptions),
    zikrText: reader_cards_zikr_text(undefined, msgOptions),
    pickSnippet: reader_cards_pick_snippet(undefined, msgOptions),
    translation: reader_cards_translation(undefined, msgOptions),
    translationArabic: reader_cards_translation_arabic(undefined, msgOptions),
    invalidAyah: reader_cards_invalid_ayah(undefined, msgOptions),
    invalidRange: reader_cards_invalid_range(undefined, msgOptions),
    needsSurah: reader_cards_needs_surah(undefined, msgOptions),
    full: reader_cards_full(undefined, msgOptions),
    collectionItems: reader_cards_collection_items(undefined, msgOptions),
    collectionAdd: reader_cards_collection_add(undefined, msgOptions),
    collectionEmpty: reader_cards_collection_empty(undefined, msgOptions),
    collectionFull: reader_cards_collection_full(undefined, msgOptions),
    collectionRef: reader_cards_collection_ref(undefined, msgOptions),
    collectionSnippet: reader_cards_collection_snippet(undefined, msgOptions),
    itemKind: reader_cards_item_kind(undefined, msgOptions),
    itemUp: reader_cards_item_up(undefined, msgOptions),
    itemDown: reader_cards_item_down(undefined, msgOptions),
    itemRemove: reader_cards_item_remove(undefined, msgOptions),
    scheduleInvalid: reader_cards_schedule_invalid(undefined, msgOptions),
    target: reader_cards_target_label(undefined, msgOptions),
  };

  const ZIKR_PRESETS = [
    { ar: "سُبْحَانَ اللَّهِ", label: "Subhanallah" },
    { ar: "الْحَمْدُ لِلَّهِ", label: "Alhamdulillah" },
    { ar: "اللَّهُ أَكْبَرُ", label: "Allahu Akbar" },
    { ar: "لَا إِلَٰهَ إِلَّا اللَّهُ", label: "La ilaha illa Allah" },
    { ar: "أَسْتَغْفِرُ اللَّهَ", label: "Astaghfirullah" },
  ] as const;

  /** Same span cap the range API enforces (EC-39) — matches the store decoder. */
  const RANGE_SPAN_MAX = 50;
  /** Flat ayah bound when the picked surah's count cannot resolve yet. */
  const AYAH_MAX = 286;

  interface TranslationOption {
    readonly id: string;
    readonly label: string;
    readonly language: string;
  }

  const translationOptions: readonly TranslationOption[] = TRANSLATION_CATALOGUE.map((entry) => ({
    id: entry.id,
    label: entry.translator === null ? entry.name : `${entry.name} — ${entry.translator}`,
    language: entry.language,
  })).sort((a, b) => a.language.localeCompare(b.language) || a.label.localeCompare(b.label));

  interface BuilderItem {
    /** Per-row item type: reader reference or azkar set — mutually exclusive. */
    mode: "ref" | "snippet";
    query: string;
    surahNum: number | null;
    surahName: string;
    ayahStart: string;
    ayahEnd: string;
    slug: string;
  }

  function blankRef(): BuilderItem {
    return { mode: "ref", query: "", surahNum: null, surahName: "", ayahStart: "", ayahEnd: "", slug: "" };
  }

  function blankSnippet(): BuilderItem {
    return { mode: "snippet", query: "", surahNum: null, surahName: "", ayahStart: "", ayahEnd: "", slug: "" };
  }

  function initialSurahNum(card: HomeCard | undefined): number | null {
    if (card !== undefined && "surah" in card) return card.surah;
    return null;
  }

  function initialSurahName(card: HomeCard | undefined, data: QuranData | undefined): string {
    if (card === undefined || !("surah" in card)) return "";
    return data?.surahByNum(card.surah)?.name ?? `Surah ${card.surah}`;
  }

  function initialSourceId(card: HomeCard | undefined): string {
    if (card === undefined) return "";
    if (card.kind === "zikr") return "";
    const sourceId =
      card.kind === "collection" ? card.items[0]?.sourceId : card.sourceId;
    // SAFETY: sourceId is string | null on every card variant; null = Arabic = "" in the select.
    return sourceId === null || sourceId === undefined ? "" : sourceId;
  }

  function initialBuilderItems(card: HomeCard | undefined, data: QuranData | undefined): BuilderItem[] {
    if (card === undefined || card.kind !== "collection") return [];
    return card.items.map((item) => {
      if ("slug" in item) return { ...blankSnippet(), slug: item.slug };
      return {
        ...blankRef(),
        surahNum: item.surah,
        surahName: data?.surahByNum(item.surah)?.name ?? `Surah ${item.surah}`,
        ayahStart: item.ayahStart !== undefined ? String(item.ayahStart) : "",
        ayahEnd: item.ayahEnd !== undefined ? String(item.ayahEnd) : "",
      };
    });
  }

  let kind = $state<CardKind>(initialCard?.kind ?? "surah");
  let surahQuery = $state("");
  let pickedSurahNum = $state<number | null>(initialSurahNum(initialCard));
  let pickedSurahName = $state(initialSurahName(initialCard, seed.data));
  let ayahInput = $state(initialCard !== undefined && initialCard.kind === "ayah" ? String(initialCard.ayah) : "");
  let rangeStartInput = $state(
    initialCard !== undefined && initialCard.kind === "range" ? String(initialCard.ayahStart) : "",
  );
  let rangeEndInput = $state(
    initialCard !== undefined && initialCard.kind === "range" ? String(initialCard.ayahEnd) : "",
  );
  let zikrText = $state(initialCard !== undefined && initialCard.kind === "zikr" ? initialCard.text : "");
  let snippetSlug = $state(initialCard !== undefined && initialCard.kind === "snippet" ? initialCard.slug : (SNIPPETS[0]?.slug ?? ""));
  let builderItems = $state<BuilderItem[]>(initialBuilderItems(initialCard, seed.data));
  let scheduleSlots = $state<ScheduleSlot[]>(
    initialCard?.schedule !== undefined ? initialCard.schedule.map((slot) => ({ ...slot })) : [],
  );
  let targetInput = $state(initialCard?.target !== undefined ? String(initialCard.target) : "");
  let sourceIdInput = $state(initialSourceId(initialCard));
  let error = $state("");

  let panelEl = $state<HTMLFormElement>();
  let headingEl = $state<HTMLHeadingElement>();

  // Opening scrolls the panel into view and moves focus to the heading; Tab
  // then reaches the first field naturally.
  $effect(() => {
    panelEl?.scrollIntoView({ block: "nearest" });
    headingEl?.focus();
  });

  /** Parses a bounded positive integer field; undefined = out of range/empty. */
  function boundedInt(raw: string, min: number, max: number): number | undefined {
    if (/^\d+$/.test(raw.trim()) === false) return undefined;
    const value = Number(raw.trim());
    if (value < min || value > max) return undefined;
    return value;
  }

  function snippetTitle(text: { en: string; ar: string }): string {
    if (locale === "ar") return text.ar;
    return text.en;
  }

  function pickedSurah() {
    if (pickedSurahNum === null || quranData === undefined) return undefined;
    return quranData.surahByNum(pickedSurahNum);
  }

  function pickSurah(suggestion: SurahSuggestion): void {
    pickedSurahNum = suggestion.num;
    pickedSurahName = suggestion.name;
    surahQuery = "";
  }

  function clearPickedSurah(): void {
    pickedSurahNum = null;
    pickedSurahName = "";
  }

  function chooseKind(next: CardKind): void {
    if (editing || kind === next) return;
    kind = next;
    ayahInput = "";
    rangeStartInput = "";
    rangeEndInput = "";
    zikrText = "";
    builderItems = [];
  }

  function setItemMode(index: number, value: string): void {
    if (value === "snippet") {
      builderItems[index] = blankSnippet();
      return;
    }
    builderItems[index] = blankRef();
  }

  function pickItemSurah(index: number, suggestion: SurahSuggestion): void {
    const row = builderItems[index];
    if (row === undefined) return;
    builderItems[index] = { ...row, query: "", surahNum: suggestion.num, surahName: suggestion.name };
  }

  function clearItemSurah(index: number): void {
    const row = builderItems[index];
    if (row === undefined) return;
    builderItems[index] = { ...row, surahNum: null, surahName: "" };
  }

  function moveItem(index: number, direction: -1 | 1): void {
    const target = index + direction;
    if (target < 0 || target >= builderItems.length) return;
    const next = [...builderItems];
    const [moved] = next.splice(index, 1);
    if (moved === undefined) return;
    next.splice(target, 0, moved);
    builderItems = next;
  }

  function removeItem(index: number): void {
    builderItems = builderItems.filter((_, i) => i !== index);
  }

  function addItem(): void {
    if (builderItems.length >= MAX_COLLECTION_ITEMS) return;
    builderItems = [...builderItems, blankRef()];
  }

  const surahSuggestions = $derived<SurahSuggestion[]>(
    quranData === undefined ? [] : suggestSurahs(quranData.surahs, surahQuery),
  );

  const rowSuggestions = $derived.by<SurahSuggestion[][]>(() => {
    const data = quranData;
    if (data === undefined) return builderItems.map(() => []);
    return builderItems.map((row) =>
      row.query.trim().length === 0 ? [] : suggestSurahs(data.surahs, row.query),
    );
  });

  /** HH:MM-validated slots; empty labels fall back to the slot's own time. */
  function cleanedSchedule(): ScheduleSlot[] | undefined {
    const slots: ScheduleSlot[] = [];
    for (const slot of scheduleSlots) {
      const time = parseTimeHHMM(slot.time.trim());
      if (time === undefined) return undefined;
      const trimmedLabel = slot.label.trim();
      slots.push({ id: slot.id, label: trimmedLabel.length > 0 ? trimmedLabel : time, time });
    }
    return slots.length === 0 ? undefined : slots;
  }

  function buildCollectionItems(sourceId: string | null): CollectionItem[] | undefined {
    const built: CollectionItem[] = [];
    for (const row of builderItems) {
      if (row.mode === "snippet") {
        if (row.slug.length === 0) continue;
        built.push({ slug: row.slug, sourceId });
        continue;
      }
      if (row.surahNum === null) continue;
      const ayahMax = quranData?.surahByNum(row.surahNum)?.ayahCount ?? AYAH_MAX;
      const startRaw = row.ayahStart.trim();
      const endRaw = row.ayahEnd.trim();
      if (startRaw.length === 0 && endRaw.length === 0) {
        built.push({ surah: row.surahNum, sourceId });
        continue;
      }
      const start = boundedInt(startRaw, 1, ayahMax);
      const end = boundedInt(endRaw, 1, ayahMax);
      if (start === undefined || end === undefined || start > end || end - start + 1 > RANGE_SPAN_MAX) {
        error = h.invalidRange;
        return undefined;
      }
      built.push({ surah: row.surahNum, ayahStart: start, ayahEnd: end, sourceId });
    }
    if (built.length === 0) {
      error = h.collectionEmpty;
      return undefined;
    }
    return built;
  }

  function buildCard(): HomeCard | undefined {
    error = "";
    const targetRaw = targetInput.trim();
    const target = targetRaw.length === 0 ? undefined : boundedInt(targetRaw, 1, 10_000);
    if (targetRaw.length > 0 && target === undefined) {
      error = h.invalidRange;
      return undefined;
    }
    const sourceId = sourceIdInput.length === 0 ? null : sourceIdInput;
    const schedule = cleanedSchedule();
    if (schedule === undefined) {
      error = h.scheduleInvalid;
      return undefined;
    }
    const id = initialCard?.id ?? newCardId();
    const count = initialCard?.count ?? 0;
    const createdAt = initialCard?.createdAt ?? nowStamp();
    const events = initialCard?.events;

    if (kind === "zikr") {
      const text = zikrText.trim();
      if (text.length === 0) {
        error = h.zikrText;
        return undefined;
      }
      return { id, kind: "zikr", text, count, target, createdAt, events, schedule };
    }
    if (kind === "snippet") {
      return { id, kind: "snippet", slug: snippetSlug, sourceId, count, target, createdAt, events, schedule };
    }
    if (kind === "collection") {
      const items = buildCollectionItems(sourceId);
      if (items === undefined) return undefined;
      return { id, kind: "collection", items, count, target, createdAt, events, schedule };
    }

    const surah = pickedSurah();
    if (surah === undefined) {
      error = h.needsSurah;
      return undefined;
    }
    if (kind === "ayah") {
      const ayah = boundedInt(ayahInput, 1, surah.ayahCount);
      if (ayah === undefined) {
        error = h.invalidAyah;
        return undefined;
      }
      return { id, kind: "ayah", surah: surah.num, ayah, sourceId, count, target, createdAt, events, schedule };
    }
    const ayahStart = boundedInt(rangeStartInput, 1, surah.ayahCount);
    const ayahEnd = boundedInt(rangeEndInput, 1, surah.ayahCount);
    if (
      ayahStart === undefined ||
      ayahEnd === undefined ||
      ayahStart > ayahEnd ||
      ayahEnd - ayahStart + 1 > RANGE_SPAN_MAX
    ) {
      error = h.invalidRange;
      return undefined;
    }
    return {
      id,
      kind: "range",
      surah: surah.num,
      ayahStart,
      ayahEnd,
      sourceId,
      count,
      target,
      createdAt,
      events,
      schedule,
    };
  }

  function submit(event: SubmitEvent): void {
    event.preventDefault();
    const next = buildCard();
    if (next === undefined) return;
    if (initialCard !== undefined) {
      homeCards.replace(next);
      onClose();
      return;
    }
    if (homeCards.add(next)) onClose();
    else error = h.full;
  }
</script>

<form
  bind:this={panelEl}
  class="flex flex-col gap-3 rounded-md border border-border bg-surface px-4 py-4"
  onsubmit={submit}
>
  <h3 bind:this={headingEl} tabindex="-1" class="text-sm font-medium text-foreground">
    {editing ? h.editTitle : h.addTitle}
  </h3>

  <div class="flex flex-wrap gap-1.5" role="group" aria-label={editing ? h.editTitle : h.addTitle}>
    {#each CARD_KINDS as kindOption (kindOption)}
      <button
        type="button"
        aria-pressed={kind === kindOption}
        aria-disabled={editing && kind !== kindOption}
        class="flex h-8 items-center rounded-pill border px-3 text-[12.5px] font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        class:border-border-strong={kind === kindOption}
        class:text-foreground={kind === kindOption}
        class:border-border={kind !== kindOption}
        class:text-foreground-secondary={kind !== kindOption}
        class:opacity-40={editing && kind !== kindOption}
        onclick={() => chooseKind(kindOption)}
      >
        {cardKindLabel(kindOption, locale)}
      </button>
    {/each}
  </div>
  {#if editing}
    <p class="text-[12.5px] text-muted">{h.kindLocked}</p>
  {/if}

  {#if kind === "zikr"}
    <div class="flex flex-wrap gap-1.5">
      {#each ZIKR_PRESETS as preset (preset.label)}
        <button
          type="button"
          class="flex h-8 items-center rounded-pill border border-border px-3 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          onclick={() => (zikrText = preset.ar)}
        >
          {preset.label}
        </button>
      {/each}
    </div>
    <label class="flex flex-col gap-1">
      <span class="eyebrow">{h.zikrText}</span>
      <input
        type="text"
        bind:value={zikrText}
        maxlength="120"
        lang="ar"
        dir="rtl"
        placeholder={h.zikrText}
        class="h-9 w-full rounded-pill border border-border bg-transparent px-3 text-sm text-foreground placeholder:text-muted focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
      />
    </label>
  {:else if kind === "snippet"}
    <label class="flex flex-col gap-1">
      <span class="eyebrow">{h.pickSnippet}</span>
      <select
        bind:value={snippetSlug}
        class="h-9 rounded-pill border border-border bg-transparent px-3 text-sm text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
      >
        {#each SNIPPETS as snippet (snippet.slug)}
          <option value={snippet.slug}>{snippetTitle(snippet.title)}</option>
        {/each}
      </select>
    </label>
  {:else if kind === "collection"}
    <span class="eyebrow">{h.collectionItems}</span>
    {#if builderItems.length === 0}
      <p class="text-[13px] text-muted">{h.collectionEmpty}</p>
    {/if}
    {#if builderItems.length >= MAX_COLLECTION_ITEMS}
      <p class="text-[13px] text-muted">{h.collectionFull}</p>
    {/if}
    <div class="flex flex-col gap-2">
      {#each builderItems as row, index (index)}
        <div class="flex flex-col gap-2 rounded-md border border-border px-3 py-3">
          <div class="flex items-center gap-2">
            <select
              aria-label={h.itemKind}
              value={row.mode}
              onchange={(event) => setItemMode(index, event.currentTarget.value)}
              class="h-7 rounded-pill border border-border bg-transparent px-1.5 text-[12.5px] text-foreground-secondary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
            >
              <option value="ref">{h.collectionRef}</option>
              <option value="snippet">{h.collectionSnippet}</option>
            </select>
            <span class="flex-1"></span>
            <button
              type="button"
              aria-label={h.itemUp}
              aria-disabled={index === 0}
              class="flex h-7 w-7 items-center justify-center rounded-pill border border-border text-[12.5px] text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
              class:opacity-40={index === 0}
              onclick={() => moveItem(index, -1)}
            >
              ↑
            </button>
            <button
              type="button"
              aria-label={h.itemDown}
              aria-disabled={index === builderItems.length - 1}
              class="flex h-7 w-7 items-center justify-center rounded-pill border border-border text-[12.5px] text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
              class:opacity-40={index === builderItems.length - 1}
              onclick={() => moveItem(index, 1)}
            >
              ↓
            </button>
            <button
              type="button"
              aria-label={h.itemRemove}
              class="flex h-7 w-7 items-center justify-center rounded-pill border border-border text-[12.5px] text-danger transition-colors hover:border-border-strong focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
              onclick={() => removeItem(index)}
            >
              ×
            </button>
          </div>

          {#if row.mode === "ref"}
            {#if row.surahNum === null}
              <input
                type="search"
                bind:value={row.query}
                placeholder={h.pickSurah}
                autocomplete="off"
                aria-label={h.pickSurah}
                class="h-9 w-full rounded-pill border border-border bg-transparent px-3 text-sm text-foreground placeholder:text-muted focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
              />
              {#if row.query.trim().length > 0}
                <ul class="divide-y divide-border overflow-hidden rounded-md border border-border">
                  {#each rowSuggestions[index] ?? [] as suggestion (suggestion.num)}
                    <li>
                      <button
                        type="button"
                        class="flex w-full items-center gap-3 px-4 py-2.5 text-start transition-colors hover:bg-surface-hover"
                        onclick={() => pickItemSurah(index, suggestion)}
                      >
                        <span class="min-w-0 flex-1">
                          <span class="block truncate text-sm font-medium text-foreground"
                            >{suggestion.num}. {suggestion.name}</span
                          >
                          <span class="block truncate text-[12.5px] text-muted"
                            >{suggestion.meaning}</span
                          >
                        </span>
                        <span
                          lang="ar"
                          dir="rtl"
                          class="flex-none font-arabic text-[17px] text-foreground-secondary"
                          >{suggestion.arabic}</span
                        >
                      </button>
                    </li>
                  {/each}
                </ul>
              {/if}
            {:else}
              <span
                class="flex h-9 items-center justify-between gap-2 rounded-pill border border-border px-3"
              >
                <span class="truncate text-sm text-foreground">{row.surahName}</span>
                <button
                  type="button"
                  class="flex h-6 w-6 flex-none items-center justify-center rounded-full border border-border text-[12px] text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
                  aria-label={h.pickSurah}
                  onclick={() => clearItemSurah(index)}
                >
                  ×
                </button>
              </span>
            {/if}
            {#if row.surahNum !== null}
              <div class="flex gap-3">
                <label class="flex min-w-0 flex-1 flex-col gap-1">
                  <span class="eyebrow">{h.rangeStart}</span>
                  <input
                    type="text"
                    inputmode="numeric"
                    bind:value={row.ayahStart}
                    class="h-9 w-full rounded-pill border border-border bg-transparent px-3 text-sm text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
                  />
                </label>
                <label class="flex min-w-0 flex-1 flex-col gap-1">
                  <span class="eyebrow">{h.rangeEnd}</span>
                  <input
                    type="text"
                    inputmode="numeric"
                    bind:value={row.ayahEnd}
                    class="h-9 w-full rounded-pill border border-border bg-transparent px-3 text-sm text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
                  />
                </label>
              </div>
            {/if}
          {:else}
            <select
              bind:value={row.slug}
              aria-label={h.pickSnippet}
              class="h-9 rounded-pill border border-border bg-transparent px-3 text-sm text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
            >
              {#each SNIPPETS as snippet (snippet.slug)}
                <option value={snippet.slug}>{snippetTitle(snippet.title)}</option>
              {/each}
            </select>
          {/if}
        </div>
      {/each}
    </div>
    {#if builderItems.length < MAX_COLLECTION_ITEMS}
      <div class="flex">
        <button
          type="button"
          class="flex h-8 items-center rounded-pill border border-border px-3 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          onclick={addItem}
        >
          {h.collectionAdd}
        </button>
      </div>
    {/if}
  {:else}
    <div class="flex flex-col gap-1">
      <span class="eyebrow">{h.pickSurah}</span>
      {#if pickedSurahNum === null}
        <input
          type="search"
          bind:value={surahQuery}
          placeholder={h.pickSurah}
          autocomplete="off"
          aria-label={h.pickSurah}
          class="h-9 w-full rounded-pill border border-border bg-transparent px-3 text-sm text-foreground placeholder:text-muted focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        />
        {#if surahQuery.trim().length > 0}
          <ul class="mt-1 divide-y divide-border overflow-hidden rounded-md border border-border">
            {#each surahSuggestions as suggestion (suggestion.num)}
              <li>
                <button
                  type="button"
                  class="flex w-full items-center gap-3 px-4 py-2.5 text-start transition-colors hover:bg-surface-hover"
                  onclick={() => pickSurah(suggestion)}
                >
                  <span class="min-w-0 flex-1">
                    <span class="block truncate text-sm font-medium text-foreground"
                      >{suggestion.num}. {suggestion.name}</span
                    >
                    <span class="block truncate text-[12.5px] text-muted">{suggestion.meaning}</span>
                  </span>
                  <span
                    lang="ar"
                    dir="rtl"
                    class="flex-none font-arabic text-[17px] text-foreground-secondary"
                    >{suggestion.arabic}</span
                  >
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      {:else}
        <span
          class="flex h-9 items-center justify-between gap-2 rounded-pill border border-border px-3"
        >
          <span class="truncate text-sm text-foreground">{pickedSurahName}</span>
          <button
            type="button"
            class="flex h-6 w-6 flex-none items-center justify-center rounded-full border border-border text-[12px] text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
            aria-label={h.pickSurah}
            onclick={clearPickedSurah}
          >
            ×
          </button>
        </span>
      {/if}
    </div>

    {#if kind === "ayah" && pickedSurahNum !== null}
      <label class="flex flex-col gap-1">
        <span class="eyebrow">{h.ayahNumber}</span>
        <input
          type="text"
          inputmode="numeric"
          bind:value={ayahInput}
          placeholder={h.ayahNumber}
          class="h-9 w-full rounded-pill border border-border bg-transparent px-3 text-sm text-foreground placeholder:text-muted focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        />
      </label>
    {/if}

    {#if kind === "range" && pickedSurahNum !== null}
      <div class="flex gap-3">
        <label class="flex min-w-0 flex-1 flex-col gap-1">
          <span class="eyebrow">{h.rangeStart}</span>
          <input
            type="text"
            inputmode="numeric"
            bind:value={rangeStartInput}
            class="h-9 w-full rounded-pill border border-border bg-transparent px-3 text-sm text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          />
        </label>
        <label class="flex min-w-0 flex-1 flex-col gap-1">
          <span class="eyebrow">{h.rangeEnd}</span>
          <input
            type="text"
            inputmode="numeric"
            bind:value={rangeEndInput}
            class="h-9 w-full rounded-pill border border-border bg-transparent px-3 text-sm text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          />
        </label>
      </div>
    {/if}
  {/if}

  <label class="flex flex-col gap-1">
    <span class="eyebrow">{h.target}</span>
    <input
      type="text"
      inputmode="numeric"
      bind:value={targetInput}
      class="h-9 w-full rounded-pill border border-border bg-transparent px-3 text-sm text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    />
  </label>

  <label class="flex flex-col gap-1">
    <span class="eyebrow">{h.translation}</span>
    <select
      bind:value={sourceIdInput}
      class="h-9 rounded-pill border border-border bg-transparent px-3 text-sm text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    >
      <option value="">{h.translationArabic}</option>
      {#each translationOptions as option (option.id)}
        <option value={option.id}>{option.language} · {option.label}</option>
      {/each}
    </select>
  </label>

  <ScheduleEditor bind:slots={scheduleSlots} />

  {#if error.length > 0}
    <p role="alert" class="text-[13px] text-danger">{error}</p>
  {/if}

  <div class="flex gap-2">
    <Button type="button" variant="secondary" onclick={onClose}>{h.cancel}</Button>
    <Button type="submit" variant="primary">{h.save}</Button>
  </div>
</form>
