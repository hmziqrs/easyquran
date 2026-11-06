<script lang="ts">
  import {
    Popover,
    PopoverContent,
    PopoverTrigger,
  } from "#lib/components/ui/popover/index.js";
  import type { UiLocale } from "#lib/i18n/locales.js";
  import {
    reader_word_attribution,
    reader_word_failed,
    reader_word_licence,
    reader_word_loading,
    reader_word_meaning,
    reader_word_missing,
    reader_word_transliteration,
  } from "#lib/i18n/m/reader.js";
  import { getLocale } from "#lib/paraglide/runtime.js";
  import { loadWordTable, lookupWord } from "#lib/quran/words/engine.js";
  import { sliceWords } from "#lib/quran/words/tokens.js";
  import type { WordEntry, WordKey, WordSource } from "#lib/quran/words/types.js";

  /**
   * One ayah split into word tokens (F21). Arabic text is rendered from the
   * slices, which join back to `text` byte for byte. Loaded only when a word
   * source is registered (VerseRow dynamic-imports this file).
   */
  let {
    text,
    vKey,
    source,
  }: {
    text: string;
    vKey: string;
    source: WordSource;
  } = $props();

  const surah = $derived(Number(vKey.split(":")[0]));
  const ayah = $derived(Number(vKey.split(":")[1]));
  const slices = $derived(sliceWords(text));

  // One table per surah, fetched on first focus or press of any word here.
  // The table is not deeply reactive: it is only ever replaced whole.
  let table = $state.raw<ReadonlyMap<WordKey, WordEntry> | null>(null);
  let failed = $state(false);
  let requested = false;

  function warm(): void {
    if (table !== null || requested) return;
    requested = true;
    failed = false;
    void loadWordTable(source, surah).then(
      (loaded) => {
        table = loaded;
      },
      () => {
        failed = true;
        requested = false;
      },
    );
  }

  function locale() {
    // SAFETY: paraglide is compiled for exactly the UI locales (en/ar), so
    // getLocale() only ever returns a UiLocale at runtime.
    return { locale: getLocale() as UiLocale };
  }

  function entryFor(word: number): WordEntry | undefined {
    if (table === null) return undefined;
    return lookupWord(table, { surah, ayah, word });
  }
</script>

{#each slices as slice, index (index)}
  {#if slice.word === null}{slice.text}{:else}
    <Popover>
      <PopoverTrigger>
        {#snippet child({ props })}
          <button
            type="button"
            {...props}
            class="word-token"
            data-word={`${vKey}:${slice.word}`}
            onpointerdown={warm}
            onfocus={warm}>{slice.text}</button
          >
        {/snippet}
      </PopoverTrigger>
      <PopoverContent class="word-popover" align="center">
        {#if failed}
          <p class="word-popover__note">{reader_word_failed(undefined, locale())}</p>
        {:else if table === null}
          <p class="word-popover__note">{reader_word_loading(undefined, locale())}</p>
        {:else}
          {@const entry = entryFor(slice.word)}
          {#if entry === undefined}
            <p class="word-popover__note">
              {reader_word_missing({ source: source.attribution }, locale())}
            </p>
          {:else}
            <dl class="word-popover__body">
              <dt>{reader_word_meaning(undefined, locale())}</dt>
              <dd lang={source.meaningLanguage} dir={source.meaningDirection}>{entry.meaning}</dd>
              <dt>{reader_word_transliteration(undefined, locale())}</dt>
              <dd dir="ltr">{entry.transliteration}</dd>
            </dl>
          {/if}
        {/if}
        <p class="word-popover__credit">
          {reader_word_attribution({ attribution: source.attribution }, locale())}
          ·
          {reader_word_licence({ licence: source.licence }, locale())}
        </p>
      </PopoverContent>
    </Popover>
  {/if}
{/each}

<style>
  .word-token {
    background: none;
    border: 0;
    color: inherit;
    cursor: pointer;
    font: inherit;
    padding: 0;
    text-decoration: underline dotted;
    text-decoration-thickness: 1px;
    text-underline-offset: 6px;
    touch-action: manipulation;
  }

  .word-token:focus-visible {
    outline: 2px solid var(--focus-ring);
    outline-offset: 2px;
    border-radius: 2px;
  }

  :global(.word-popover) {
    font-family: var(--font-sans);
    font-size: 13px;
    width: min(18rem, calc(100vw - 2rem));
  }

  .word-popover__body {
    display: grid;
    gap: 0.25rem 0.75rem;
    grid-template-columns: max-content 1fr;
    margin: 0;
  }

  .word-popover__body dt {
    color: var(--muted);
  }

  .word-popover__body dd {
    margin: 0;
  }

  .word-popover__note,
  .word-popover__credit {
    color: var(--muted);
    margin: 0;
  }

  .word-popover__credit {
    font-size: 12px;
  }
</style>
