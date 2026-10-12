<script lang="ts">
  import { onMount } from "svelte";

  import { loadQuranData, peekQuranData } from "#lib/data/quran-data-client.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import { homeCards } from "#lib/home/cards.svelte.js";
  import CardTile from "#lib/home/CardTile.svelte";
  import { yoursPageHref } from "#lib/i18n/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { appLocale } from "#lib/i18n/app-locale.js";
  import { reader_cards_strip_more, reader_cards_strip_title } from "#lib/i18n/m/reader.js";

  /**
   * The landing page's personal cards strip. Prerendered HTML carries nothing —
   * the section mounts client-side and stays hidden until the device-local
   * card store actually has cards, so first visitors and crawler HTML see the
   * plain index (same pattern as /yours' post-mount widget column).
   */
  const STRIP_LIMIT = 4;

  const locale = appLocale();
  const msgOptions = { locale };

  let mounted = $state(false);
  let quranData = $state<QuranData | undefined>(peekQuranData());

  const cards = $derived(mounted ? homeCards.list : []);
  const visibleCards = $derived(cards.slice(0, STRIP_LIMIT));
  const extraCount = $derived(Math.max(0, cards.length - visibleCards.length));
  const stripTitle = $derived(reader_cards_strip_title(undefined, msgOptions));

  onMount(() => {
    mounted = true;
    homeCards.hydrate();
    if (peekQuranData() === undefined) {
      void loadQuranData().then(
        (data) => {
          quranData = data;
        },
        () => undefined,
      );
    }
  });
</script>

{#if mounted && visibleCards.length > 0}
  <section aria-label={stripTitle} class="mt-6 w-full max-w-[720px]">
    <div class="flex items-center justify-between gap-3">
      <h2 class="eyebrow">{stripTitle}</h2>
      <a
        href={publicHref(yoursPageHref())}
        data-sveltekit-preload-data="hover"
        aria-label={stripTitle}
        class="flex h-8 flex-none items-center rounded-pill border border-border px-3 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
      >
        {#if extraCount > 0}{reader_cards_strip_more({ extra: extraCount }, msgOptions)}{/if}
        <span aria-hidden="true" class="ms-1">→</span>
      </a>
    </div>
    <div class="mt-2 flex flex-col gap-2">
      {#each visibleCards as card (card.id)}
        <CardTile {card} quranData={quranData} />
      {/each}
    </div>
  </section>
{/if}
