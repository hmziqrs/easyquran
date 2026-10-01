<script lang="ts">
  import ChevronDownIcon from "@lucide/svelte/icons/chevron-down";
  import LanguagesIcon from "@lucide/svelte/icons/languages";
  import PanelLeftIcon from "@lucide/svelte/icons/panel-left";
  import Bismillah from "$lib/components/brand/Bismillah.svelte";
  import { Icon, type IconName } from "$lib/components/icon";
  import { toArabicDigits } from "$lib/data/quran";
  import type { MixSurah, MixTranslation } from "../mix/types";
  import type { ReaderSystem } from "./systems";

  let {
    sys,
    surah,
    opener,
    verses,
    translations,
  }: {
    sys: ReaderSystem;
    surah: MixSurah;
    opener: string | null;
    verses: readonly string[];
    translations: readonly MixTranslation[];
  } = $props();

  const TOOLS: readonly { icon: IconName; label: string }[] = [
    { icon: "bookmark", label: "Bookmark ayah" },
    { icon: "copy", label: "Copy ayah" },
    { icon: "share", label: "Share ayah" },
    { icon: "note", label: "Note and tafsir" },
  ];
  const LINKS = ["Surahs", "Juz", "Pages", "Yours"] as const;

  let mode = $state<"verse" | "reading">("verse");
  let arabicSize = $state(34);
  const pageLine = $derived(`Page 1 of ${surah.pageCount}`);
</script>

{#snippet lane(t: MixTranslation, i: number)}
  <div
    dir={t.direction}
    lang={t.languageCode}
    class={["sys-lane", `sys-lane--${sys.lanes}`, t.direction === "rtl" && "sys-lane--rtl"]}
  >
    {#if sys.lanes === "inline"}
      <p class="sys-read">
        <span class="sys-name-wrap" dir={t.direction}><bdi class="sys-name">{t.name}</bdi></span>{t.texts[i]}
      </p>
    {:else}
      <span class="sys-name sys-name--block" dir="auto">{t.name}</span>
      <p class="sys-read">{t.texts[i]}</p>
    {/if}
  </div>
{/snippet}

{#snippet modeSwitch()}
  <div class={["sys-modes", `sys-modes--${sys.header}`]} role="group" aria-label="Reading mode">
    <button type="button" aria-pressed={mode === "verse"} onclick={() => (mode = "verse")} class="sys-mode">
      <Icon name="rows" size={13} /> Ayah-by-Ayah
    </button>
    <button type="button" aria-pressed={mode === "reading"} onclick={() => (mode = "reading")} class="sys-mode">
      <Icon name="continuous" size={13} /> Reading
    </button>
  </div>
{/snippet}

<div class="sys" style={`${sys.vars}; --sys-arabic: ${arabicSize}px`}>
  <nav class="sys-nav" aria-label="Site">
    <div class="sys-frame sys-nav-row">
      <span class="sys-brand">
        <span class="sys-mark" aria-hidden="true">ق</span>
        <span class="sys-word">easyquran</span>
      </span>
      <span class="sys-links">
        {#each LINKS as link, i (link)}
          <span class={["sys-link", i === 0 && "sys-link--on"]}>{link}</span>
        {/each}
      </span>
      <span class="sys-search">
        <Icon name="search" size={15} />
        <span>Search the Qur'an</span>
      </span>
      <span class="sys-icons" aria-hidden="true">
        <span class="sys-icon"><Icon name="moon" size={16} /></span>
        <span class="sys-icon"><Icon name="user" size={16} /></span>
      </span>
    </div>
  </nav>

  <header class="sys-bar">
    <div class="sys-frame sys-bar-row">
      <button type="button" class="sys-ctrl sys-ctrl--icon" aria-label="Browse surahs, juz and pages">
        <PanelLeftIcon class="size-[18px]" />
      </button>
      <button type="button" class="sys-ctrl" aria-label="Translations">
        <LanguagesIcon class="size-4" />
        <span>Translations</span>
        {#if translations.length > 0}<span class="sys-count">{translations.length}</span>{/if}
        <ChevronDownIcon class="size-4 sys-soft-icon" />
      </button>
      <span class="sys-bar-title">{surah.num}. {surah.name}</span>
      <span class="sys-size" role="group" aria-label="Arabic text size">
        <button type="button" onclick={() => (arabicSize = Math.max(24, arabicSize - 3))} aria-label="Smaller Arabic text">A&minus;</button>
        <span class="sys-size-rule" aria-hidden="true"></span>
        <button type="button" onclick={() => (arabicSize = Math.min(52, arabicSize + 3))} aria-label="Larger Arabic text">A+</button>
      </span>
    </div>
  </header>

  <main class="sys-frame sys-main">
    <div class={["sys-body", sys.frame === "card" && "sys-card"]}>
      {#if sys.header === "centered"}
        <div class="sys-head sys-head--centered">
          <span class="sys-eyebrow">Surah {surah.num}</span>
          <span dir="rtl" lang="ar" class="sys-head-arabic">{surah.arabic}</span>
          <h1 class="sys-title">{surah.name}</h1>
          <span class="sys-meta">{surah.meaning} · {surah.meta} · {pageLine}</span>
          {@render modeSwitch()}
        </div>
      {:else if sys.header === "strip"}
        <div class="sys-head sys-head--strip">
          <span class="sys-numbox" aria-hidden="true">{surah.num}</span>
          <div class="sys-head-text">
            <h1 class="sys-title">{surah.name} <span dir="rtl" lang="ar" class="sys-title-arabic">{surah.arabic}</span></h1>
            <span class="sys-meta">{surah.meaning} · {surah.meta} · {pageLine}</span>
          </div>
          {@render modeSwitch()}
        </div>
      {:else}
        <div class="sys-head sys-head--editorial">
          <div class="sys-head-line">
            <h1 class="sys-title">{surah.name}</h1>
            <span dir="rtl" lang="ar" class="sys-head-arabic">{surah.arabic}</span>
          </div>
          <span class="sys-meta">Surah {surah.num} · {surah.meaning} · {surah.meta} · {pageLine}</span>
          {@render modeSwitch()}
        </div>
      {/if}

      {#if opener}
        <div class="sys-opener"><Bismillah class="w-44" title={opener} /></div>
      {/if}

      {#if mode === "reading"}
        <p dir="rtl" lang="ar" class="sys-flow">
          {#each verses as ayah, i (i)}{ayah}<span class="sys-ornament">&#x06DD;{toArabicDigits(i + 1)}</span>
            {" "}{/each}
        </p>
      {:else}
        <ol class={["sys-ayahs", `sys-ayahs--${sys.verseKey}`]}>
          {#each verses as ayah, i (i)}
            <li class="sys-ayah">
              <div class="sys-ayah-top">
                {#if sys.verseKey === "margin"}
                  <span class="sys-key sys-key--margin">{i + 1}</span>
                {:else}
                  <span class={["sys-key", `sys-key--${sys.verseKey}`]}>{surah.num}:{i + 1}</span>
                {/if}
                <span class="sys-tools">
                  {#each TOOLS as tool (tool.icon)}
                    <button type="button" class="sys-tool" aria-label={tool.label}><Icon name={tool.icon} size={15} /></button>
                  {/each}
                </span>
              </div>
              <p dir="rtl" lang="ar" class="sys-arabic">
                {ayah}<span class="sys-ornament">&#x06DD;{toArabicDigits(i + 1)}</span>
              </p>
              {#if translations.length > 0}
                <div class={["sys-lanes", `sys-lanes--${sys.lanes}`]}>
                  {#each translations as t (t.id)}
                    {@render lane(t, i)}
                  {/each}
                </div>
              {/if}
            </li>
          {/each}
        </ol>
      {/if}
    </div>
  </main>
</div>

<style>
  /* Every value below comes from the system's tokens (--sys-*): one ground, one accent with
     one job, one radius, one rhythm. Nothing is styled per piece. */
  .sys {
    background: var(--sys-page);
    color: var(--sys-text);
    font-family: var(--sys-ui);
    font-weight: var(--sys-ui-weight);
    min-height: 100vh;
    padding-bottom: 96px;
  }

  .sys-frame {
    margin-inline: auto;
    max-width: calc(var(--sys-column) + 48px);
    padding-inline: 24px;
  }

  button {
    font: inherit;
  }

  /* ── Nav and sticky bar: same frame, same 56px rhythm, same controls ── */
  .sys-nav,
  .sys-bar {
    border-bottom: 1px solid var(--sys-line);
  }

  .sys-bar {
    backdrop-filter: blur(16px);
    background: var(--sys-bar);
    position: sticky;
    top: 0;
    z-index: 30;
  }

  .sys-nav-row,
  .sys-bar-row {
    align-items: center;
    display: flex;
    gap: 16px;
    height: 56px;
  }

  .sys-brand {
    align-items: center;
    display: flex;
    flex: none;
    gap: 10px;
  }

  .sys-mark {
    align-items: center;
    background: var(--sys-accent-fill);
    border-radius: var(--sys-radius);
    color: var(--sys-on-accent);
    display: flex;
    font-family: var(--font-arabic);
    font-size: 18px;
    height: 32px;
    justify-content: center;
    width: 32px;
  }

  .sys-word {
    font-size: 17px;
    font-weight: 600;
  }

  .sys-links {
    display: none;
    gap: 18px;
  }

  .sys-link {
    color: var(--sys-soft);
    font-size: 14px;
  }

  .sys-link--on {
    color: var(--sys-text);
  }

  .sys-search {
    align-items: center;
    border: 1px solid var(--sys-line);
    border-radius: var(--sys-radius);
    color: var(--sys-soft);
    display: none;
    flex: 1;
    font-size: 14px;
    gap: 10px;
    height: 40px;
    padding-inline: 14px;
  }

  .sys-icons {
    display: flex;
    gap: 8px;
    margin-inline-start: auto;
  }

  .sys-icon {
    align-items: center;
    border: 1px solid var(--sys-line);
    border-radius: var(--sys-radius);
    color: var(--sys-soft);
    display: flex;
    height: 40px;
    justify-content: center;
    width: 40px;
  }

  @media (min-width: 760px) {
    .sys-links {
      display: flex;
    }

    .sys-search {
      display: flex;
    }

    .sys-icons {
      margin-inline-start: 0;
    }
  }

  .sys-ctrl {
    align-items: center;
    border: 1px solid var(--sys-line);
    border-radius: var(--sys-radius);
    color: var(--sys-text);
    display: flex;
    flex: none;
    font-size: 14px;
    gap: 8px;
    height: 40px;
    padding-inline: 12px;
    transition: border-color 150ms;
  }

  .sys-ctrl:hover {
    border-color: var(--sys-accent);
  }

  .sys-ctrl--icon {
    justify-content: center;
    padding: 0;
    width: 40px;
  }

  .sys-ctrl :global(.sys-soft-icon) {
    color: var(--sys-soft);
  }

  .sys-count {
    align-items: center;
    background: var(--sys-accent-fill);
    border-radius: 999px;
    color: var(--sys-on-accent);
    display: flex;
    font-size: 12px;
    font-weight: 700;
    height: 20px;
    justify-content: center;
    min-width: 20px;
    padding-inline: 6px;
  }

  .sys-bar-title {
    color: var(--sys-soft);
    display: none;
    font-size: 14px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (min-width: 640px) {
    .sys-bar-title {
      display: inline;
    }
  }

  .sys-size {
    align-items: center;
    border: 1px solid var(--sys-line);
    border-radius: var(--sys-radius);
    display: flex;
    height: 40px;
    margin-inline-start: auto;
    padding: 2px;
  }

  .sys-size button {
    border-radius: calc(var(--sys-radius) - 2px);
    color: var(--sys-text);
    font-size: 14px;
    height: 34px;
    width: 36px;
  }

  .sys-size button:hover {
    color: var(--sys-accent);
  }

  .sys-size-rule {
    background: var(--sys-line);
    height: 16px;
    width: 1px;
  }

  /* ── Page body ── */
  .sys-main {
    padding-top: 24px;
  }

  .sys-card {
    border: 1px solid var(--sys-line);
    border-radius: var(--sys-radius);
  }

  .sys-head {
    display: flex;
  }

  .sys-title {
    color: var(--sys-text);
    font-weight: 600;
    letter-spacing: -0.015em;
  }

  .sys-meta {
    color: var(--sys-soft);
    font-size: 13.5px;
  }

  .sys-head--centered {
    align-items: center;
    border-bottom: 1px solid var(--sys-line);
    flex-direction: column;
    gap: 6px;
    padding: 40px 0 28px;
    text-align: center;
  }

  .sys-eyebrow {
    color: var(--sys-accent);
    font-size: 13px;
    font-weight: 600;
  }

  .sys-head-arabic {
    font-family: var(--font-arabic);
    font-size: 46px;
    line-height: 1.4;
  }

  .sys-head--centered .sys-title {
    font-size: 24px;
  }

  .sys-head--strip {
    align-items: center;
    border-bottom: 1px solid var(--sys-line);
    flex-wrap: wrap;
    gap: 12px 16px;
    padding: 20px 28px;
  }

  .sys-numbox {
    align-items: center;
    background: var(--sys-accent-wash);
    border-radius: var(--sys-radius);
    color: var(--sys-accent);
    display: flex;
    flex: none;
    font-size: 15px;
    font-variant-numeric: tabular-nums;
    font-weight: 700;
    height: 40px;
    justify-content: center;
    width: 40px;
  }

  .sys-head-text {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .sys-head--strip .sys-title {
    font-size: 22px;
  }

  .sys-title-arabic {
    font-family: var(--font-arabic);
    font-size: 26px;
    font-weight: 400;
    margin-inline-start: 6px;
  }

  .sys-head--editorial {
    flex-direction: column;
    gap: 8px;
    padding: 40px 0 32px;
  }

  .sys-head-line {
    align-items: baseline;
    display: flex;
    gap: 16px;
    justify-content: space-between;
  }

  .sys-head--editorial .sys-title {
    font-size: 34px;
    font-weight: 500;
  }

  .sys-head--editorial .sys-head-arabic {
    font-size: 36px;
  }

  .sys-head--editorial .sys-meta {
    font-size: 15px;
  }

  .sys-modes {
    display: flex;
    gap: 4px;
  }

  .sys-mode {
    align-items: center;
    color: var(--sys-soft);
    display: flex;
    font-size: 13.5px;
    gap: 6px;
    height: 36px;
    padding-inline: 14px;
    transition: color 150ms, background-color 150ms;
  }

  .sys-mode:hover {
    color: var(--sys-text);
  }

  /* Mushaf: text tabs with an accent underline for the active one. */
  .sys-modes--centered {
    margin-top: 14px;
  }

  .sys-modes--centered .sys-mode {
    border-bottom: 2px solid transparent;
    border-radius: 0;
  }

  .sys-modes--centered .sys-mode[aria-pressed="true"] {
    border-bottom-color: var(--sys-accent);
    color: var(--sys-text);
  }

  /* Ledger: a bordered segmented control, the active segment filled with the accent. */
  .sys-modes--strip {
    border: 1px solid var(--sys-line);
    border-radius: var(--sys-radius);
    padding: 2px;
  }

  .sys-modes--strip .sys-mode {
    border-radius: calc(var(--sys-radius) - 2px);
    height: 34px;
  }

  .sys-modes--strip .sys-mode[aria-pressed="true"] {
    background: var(--sys-accent-fill);
    color: var(--sys-on-accent);
  }

  /* Garden: pills, the active one washed in the accent. */
  .sys-modes--editorial {
    margin-top: 12px;
  }

  .sys-modes--editorial .sys-mode {
    border: 1px solid var(--sys-line);
    border-radius: 999px;
  }

  .sys-modes--editorial .sys-mode[aria-pressed="true"] {
    background: var(--sys-accent-wash);
    border-color: transparent;
    color: var(--sys-accent);
  }

  .sys-opener {
    color: var(--sys-text);
    display: flex;
    justify-content: center;
    padding: 36px 0 12px;
  }

  /* ── Ayahs ── */
  .sys-ayahs {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .sys-ayah {
    padding-block: calc(var(--sys-ayah-gap) / 2);
    position: relative;
  }

  .sys-ayahs--quiet .sys-ayah + .sys-ayah,
  .sys-ayahs--pill .sys-ayah + .sys-ayah {
    border-top: 1px solid var(--sys-line);
  }

  .sys-ayahs--pill .sys-ayah {
    padding-inline: 28px;
  }

  .sys-ayah-top {
    align-items: center;
    display: flex;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  .sys-key {
    color: var(--sys-soft);
    font-size: 12.5px;
    font-variant-numeric: tabular-nums;
  }

  .sys-key--pill {
    background: var(--sys-accent-wash);
    border-radius: var(--sys-radius);
    color: var(--sys-accent);
    font-weight: 700;
    padding: 4px 10px;
  }

  .sys-key--margin {
    color: var(--sys-accent);
    font-size: 15px;
    font-weight: 600;
  }

  @media (min-width: 1100px) {
    .sys-key--margin {
      inset-inline-start: -56px;
      position: absolute;
      top: calc(var(--sys-ayah-gap) / 2 + 8px);
    }
  }

  .sys-tools {
    display: flex;
    gap: 2px;
    margin-inline-start: auto;
  }

  .sys-tool {
    align-items: center;
    border-radius: var(--sys-radius);
    color: var(--sys-soft);
    display: flex;
    height: 32px;
    justify-content: center;
    transition: color 150ms;
    width: 32px;
  }

  .sys-tool:hover {
    color: var(--sys-accent);
  }

  .sys-arabic,
  .sys-flow {
    color: var(--sys-text);
    font-family: var(--reader-arabic-family, var(--font-arabic));
    font-size: var(--sys-arabic);
  }

  .sys-arabic {
    line-height: 2.1;
    padding-bottom: 12px;
  }

  .sys-flow {
    line-height: 2.35;
    padding: 24px 28px;
    text-align: justify;
    text-align-last: center;
  }

  .sys-ornament {
    color: var(--sys-accent);
    font-size: 0.86em;
    margin-inline-start: 0.12em;
  }

  /* ── Translation lanes ── */
  .sys-lanes {
    display: flex;
    flex-direction: column;
    gap: var(--sys-lane-gap);
  }

  .sys-lane {
    max-width: 72ch;
  }

  .sys-lane--rtl {
    margin-left: auto;
  }

  .sys-lane--panel {
    background: var(--sys-accent-wash);
    border-radius: var(--sys-radius);
    padding: 12px 16px;
  }

  .sys-read {
    color: var(--sys-text);
    font-family: var(--sys-read);
    font-size: var(--sys-read-size);
    line-height: var(--sys-read-lead);
    text-wrap: pretty;
  }

  .sys-lane--rtl .sys-read {
    font-family: "Noto Naskh Arabic", var(--font-arabic-ui);
    font-size: calc(var(--sys-read-size) * 1.12);
    line-height: calc(var(--sys-read-lead) + 0.3);
  }

  /* Names can be Latin or Arabic-script (جالندہری) whatever the lane: Nunito first, Naskh
     for the Arabic letters Nunito does not have. */
  .sys-name {
    color: var(--sys-accent);
    font-family: "Nunito Variable", "Noto Naskh Arabic", var(--font-arabic-ui), sans-serif;
    font-size: var(--sys-label-size);
    font-weight: 600;
  }

  .sys-name--block {
    display: block;
    margin-bottom: 4px;
  }

  .sys-name-wrap {
    margin-inline-end: 0.6em;
  }
</style>
