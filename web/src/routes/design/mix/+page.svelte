<script lang="ts">
  import { onMount } from "svelte";
  import { goto, replaceState } from "$app/navigation";
  import { page } from "$app/state";
  import { Icon } from "$lib/components/icon";
  import { loadArabicFont } from "$lib/fonts/arabic-fonts";
  import { trailingDebounce } from "$lib/storage";
  import { BG_COLORS, readMix, translationsSearch } from "../_variants/mix/axes";
  import Browse from "../_variants/mix/Browse.svelte";
  import MixPanel from "../_variants/mix/MixPanel.svelte";
  import StickyBar from "../_variants/mix/StickyBar.svelte";
  import SurahHead from "../_variants/mix/SurahHead.svelte";
  import TranslationsDialog from "../_variants/mix/TranslationsDialog.svelte";
  import TweakPanel from "../_variants/mix/TweakPanel.svelte";
  import { readTweaks, tweakStyle, writeTweaks, type Tweaks } from "../_variants/mix/tweaks";
  import Verses from "../_variants/mix/Verses.svelte";

  let { data } = $props();

  // Sample last-read spot (the live reader shows the reader's real one).
  const RESUME_NAME = "Al-Baqarah";
  const RESUME_KEY = "2:286";
  const MIN_ARABIC = 22;
  const MAX_ARABIC = 52;

  const mix = $derived(readMix(page.url.searchParams));
  // Writable deriveds: a real navigation (an axis link) re-reads them from the URL; slider
  // and drawer changes are local and written back with replaceState.
  let tweaks = $derived(readTweaks(page.url.searchParams));
  let browseOpen = $derived(page.url.searchParams.get("drawer") === "1");
  let mode = $state<"verse" | "reading">("verse");
  let arabicSize = $state(33);
  let toastClosed = $state(false);
  let translationsOpen = $state(false);
  // Ticks show at once; the page's translation texts follow when the reload lands.
  let chosenIds = $derived(data.translations.map((t) => t.id));

  function changeTranslations(ids: readonly string[]): void {
    chosenIds = [...ids];
    void goto(translationsSearch(params, mix, ids), {
      noScroll: true,
      keepFocus: true,
      replaceState: true,
    });
  }

  // replaceState does not refresh page.url, so links are built from this merged copy.
  const params = $derived.by(() => {
    const next = new URLSearchParams(page.url.searchParams);
    writeTweaks(next, tweaks);
    if (!browseOpen) next.delete("drawer");
    return next;
  });

  const writeUrl = trailingDebounce(() => {
    const url = new URL(window.location.href);
    writeTweaks(url.searchParams, tweaks);
    if (!browseOpen) url.searchParams.delete("drawer");
    replaceState(url, page.state);
  }, 250);

  function changeTweaks(next: Tweaks): void {
    tweaks = next;
    writeUrl.schedule();
  }

  function closeBrowse(): void {
    browseOpen = false;
    writeUrl.schedule();
  }

  const ground = $derived(BG_COLORS[mix.bg]);
  const rootStyle = $derived(
    [
      tweakStyle(tweaks),
      `--reader-arabic-size: ${arabicSize}px`,
      `--mix-page-dark: ${tweaks.page || ground.page}`,
      `--mix-reader-dark: ${tweaks.reader || ground.reader}`,
      `--mix-line-dark: ${ground.line}`,
    ].join("; "),
  );

  onMount(() => {
    // Urdu translations render in Naskh in options B–E (same lazy loader as the reader).
    void loadArabicFont("noto-naskh-arabic");
    return () => writeUrl.cancel();
  });
</script>

<svelte:head>
  <title>Reader mix — EasyQuran design</title>
</svelte:head>

<MixPanel {mix} {params} translations={data.translations} missing={data.missing} />

<div class="mix-root min-h-screen" style={rootStyle}>
  <StickyBar
    bar={mix.bar}
    resume={mix.resume}
    surah={data.surah}
    translations={data.translations}
    resumeName={RESUME_NAME}
    resumeKey={RESUME_KEY}
    showSize={mix.head !== "a"}
    onBrowse={() => (browseOpen = true)}
    onTranslations={() => (translationsOpen = true)}
    onSmaller={() => (arabicSize = Math.max(MIN_ARABIC, arabicSize - 3))}
    onBigger={() => (arabicSize = Math.min(MAX_ARABIC, arabicSize + 3))}
  />

  <div class="mx-auto flex w-full max-w-[1180px] flex-col gap-4 px-5 py-6 sm:px-7 lg:px-10">
    {#if mix.resume === "a"}
      <button
        type="button"
        class="flex items-center gap-3 rounded-md bg-primary-soft px-[18px] py-[13px] text-start transition-[filter] duration-150 hover:brightness-[0.98]"
      >
        <Icon name="play" size={15} class="flex-none text-primary" />
        <span class="text-sm text-primary">Continue reading — Surah 2 {RESUME_KEY}</span>
        <span class="ms-auto text-[13px] text-primary">Jump <span aria-hidden="true">→</span></span>
      </button>
    {/if}

    <!-- overflow-clip, not hidden: the Columns header must stay sticky inside the card. -->
    <div class={["overflow-clip", `frame-${mix.frame}`]}>
      <SurahHead
        head={mix.head}
        resume={mix.resume}
        surah={data.surah}
        {mode}
        resumeName={RESUME_NAME}
        resumeKey={RESUME_KEY}
        onMode={(next) => (mode = next)}
        onSmaller={() => (arabicSize = Math.max(MIN_ARABIC, arabicSize - 3))}
        onBigger={() => (arabicSize = Math.min(MAX_ARABIC, arabicSize + 3))}
      />
      <Verses
        stack={mix.stack}
        {mode}
        surahNum={data.surah.num}
        opener={data.opener}
        verses={data.verses}
        translations={data.translations}
        {tweaks}
      />
    </div>
  </div>

  {#if mix.resume === "c" && !toastClosed}
    <div
      role="status"
      class="fixed bottom-5 left-5 z-40 flex max-w-[calc(100vw-6.5rem)] items-center gap-3 rounded-lg border border-(--mix-line) bg-(--mix-page) py-2 pe-2 ps-4"
    >
      <span class="min-w-0 text-[14px] text-foreground">
        Continue <span class="font-semibold">{RESUME_NAME} {RESUME_KEY}</span>
      </span>
      <button
        type="button"
        class="flex h-8 flex-none items-center rounded-md bg-primary px-3 text-[13px] font-medium text-primary-foreground transition-colors hover:bg-primary-hover"
      >
        Jump
      </button>
      <button
        type="button"
        aria-label="Dismiss"
        onclick={() => (toastClosed = true)}
        class="flex size-8 flex-none items-center justify-center rounded-md text-foreground-secondary transition-colors hover:text-foreground"
      >
        <Icon name="x" size={14} />
      </button>
    </div>
  {/if}

  <Browse
    lists={mix.lists}
    open={browseOpen}
    surahs={data.surahs}
    juz={data.juz}
    pages={data.pages}
    activeSurah={data.surah.num}
    activeJuz={data.activeJuz}
    activePage={data.activePage}
    onClose={closeBrowse}
  />

  <TweakPanel {tweaks} onChange={changeTweaks} />

  <TranslationsDialog
    bind:open={translationsOpen}
    selected={chosenIds}
    max={data.maxTranslations}
    onChange={changeTranslations}
  />
</div>

<style>
  /* Grounds: light mode keeps the theme's tokens; dark mode takes the Background preset (or
     the Tweak panel's custom colours). Accent text: the dark --primary (L 0.50) is tuned as a
     fill under white text and reads dim as text on near-black, so dark mode lifts it. */
  .mix-root {
    --mix-page: var(--background);
    --mix-reader: var(--reader-background);
    --mix-line: var(--reader-divider);
    --mix-accent-text: var(--primary);
    background: var(--mix-page);
  }

  :global(:root:not([data-mode="light"])) .mix-root {
    --mix-page: var(--mix-page-dark);
    --mix-reader: var(--mix-reader-dark);
    --mix-line: var(--mix-line-dark);
    --mix-accent-text: color-mix(in oklch, var(--primary) 55%, white);
  }

  .frame-a {
    background: var(--mix-reader);
    border: 1px solid var(--mix-line);
    border-radius: var(--radius-lg, 14px);
  }

  .frame-b {
    background: var(--mix-reader);
    border-radius: var(--radius-lg, 14px);
  }

  /* Flat: no card at all — the column reads straight off the page. */
  .frame-c {
    --mix-reader: var(--mix-page);
    background: transparent;
  }
</style>
