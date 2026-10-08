<script lang="ts">
  import { page } from "$app/state";
  import Brand from "#lib/components/brand/Brand.svelte";
  import Icon from "#lib/components/icon/Icon.svelte";
  import { routeContextFromParams, surahPathFor } from "#lib/data/quran.js";
  import { resolveLandingCopy } from "#lib/i18n/landing-copy.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import type { SurahCard } from "../../(marketing)/+page.server";
  import type { HomeAccent, HomeFont, HomeSurface, HomeVariant } from "./variants";

  let {
    surahs,
    surahCount,
    juzCount,
    pageCount,
    variant,
    font,
    accent,
    surface,
    mode,
  }: {
    surahs: SurahCard[];
    surahCount: number;
    juzCount: number;
    pageCount: number;
    variant: HomeVariant;
    font: HomeFont;
    accent: HomeAccent;
    surface: HomeSurface;
    mode: "light" | "dark";
  } = $props();

  const ctx = $derived(routeContextFromParams(page.params));
  const bismillah = resolveLandingCopy("en").closingBismillah;
  const opening = $derived(surahs.find((surah) => surah.num === 1));
  const openingHref = $derived(opening ? publicHref(surahPathFor(ctx, opening)) : publicHref("/surah"));
  const categories = ["Often opened", "In order", "Short surahs"] as const;
  let category = $state<(typeof categories)[number]>("Often opened");
  const selectedSurahs = $derived.by(() => {
    if (category === "In order") return surahs.slice(0, 6);
    const numbers = category === "Short surahs" ? [103, 108, 109, 112, 113, 114] : [1, 18, 36, 55, 56, 67];
    return numbers.flatMap((number) => {
      const match = surahs.find((surah) => surah.num === number);
      return match ? [match] : [];
    });
  });
</script>

<div class="home-preview" lang="en" dir="ltr" data-variant={variant} data-font={font} data-accent={accent} data-surface={surface} data-mode={mode}>
  <header class="site-header container">
    <Brand />
    <nav aria-label="Main navigation">
      <a href={publicHref("/surah")}>Read the Qur’an</a>
      <a href={publicHref("/about")}>About</a>
      <a href={publicHref("/faq")}>FAQ</a>
    </nav>
    <a class="sign-in" href={publicHref("/login")}>Sign in <Icon name="arrow-right" size={15} /></a>
  </header>

  <section class="hero container" aria-labelledby="home-heading">
    <div class="hero-copy">
      <p class="hero-kicker">A space for your daily reading</p>
      <h1 id="home-heading">The Qur’an,<br /><span>a little closer.</span></h1>
      <p class="intro">Open a chapter. Find a verse. Make a little room for reading, wherever you are.</p>
      <div class="hero-actions">
        <a class="button primary" href={openingHref}>Start reading <Icon name="arrow-right" size={19} /></a>
        <a class="text-link" href="#browse">Explore the surahs <Icon name="rows" size={18} /></a>
      </div>
    </div>

    {#if variant !== "mono"}
      <div class="opening-panel">
        <div class="opening-meta"><span>Begin at the beginning</span><Icon name="book" size={20} /></div>
        <p class="bismillah" lang="ar" dir="rtl">{bismillah}</p>
        <div class="opening-bottom">
          <div><p class="opening-name">{opening?.name ?? "Al-Fatihah"}</p><p class="opening-detail">The Opening <span aria-hidden="true">·</span> {opening?.ayahCount ?? 7} ayahs</p></div>
          <a class="round-link" href={openingHref} aria-label="Read Al-Fatihah"><Icon name="arrow-right" size={21} /></a>
        </div>
      </div>
    {/if}
  </section>

  <div class="search-section container">
    <form method="GET" action={publicHref("/search")} role="search" class="search-form">
      <Icon name="search" size={21} />
      <label class="sr-only" for="home-search">Search the Qur’an</label>
      <input id="home-search" name="q" type="search" placeholder="Search a surah, ayah, or word…" autocomplete="off" />
      <button type="submit">Search <Icon name="arrow-right" size={17} /></button>
    </form>
  </div>

  <nav class="reader-shortcuts container" aria-label="Ways to read">
    <a href={publicHref("/surah")}><Icon name="book" size={21} /><span><strong>{surahCount}</strong> surahs</span><Icon name="arrow-right" size={16} /></a>
    <a href={publicHref("/juz")}><Icon name="continuous" size={21} /><span><strong>{juzCount}</strong> juz</span><Icon name="arrow-right" size={16} /></a>
    <a href={publicHref("/pages")}><Icon name="note" size={21} /><span><strong>{pageCount}</strong> pages</span><Icon name="arrow-right" size={16} /></a>
    <a href={publicHref("/yours")}><Icon name="bookmark" size={21} /><span>Your bookmarks</span><Icon name="arrow-right" size={16} /></a>
  </nav>

  <section id="browse" class="browse container" aria-labelledby="browse-heading">
    <div class="section-heading">
      <div><h2 id="browse-heading">Find your place.</h2><p>A familiar chapter, or somewhere new to begin.</p></div>
      <a class="text-link" href={publicHref("/surah")}>All {surahCount} surahs <Icon name="arrow-right" size={18} /></a>
    </div>
    <div class="browse-filters" role="group" aria-label="Choose surahs to display">
      {#each categories as item (item)}
        <button type="button" aria-pressed={category === item} onclick={() => (category = item)}>{item}</button>
      {/each}
    </div>
    <ul class="surah-list">
      {#each selectedSurahs as surah (surah.num)}
        <li>
          <a class="surah-row" href={publicHref(surahPathFor(ctx, surah))}>
            <span class="surah-number">{String(surah.num).padStart(2, "0")}</span>
            <span class="surah-copy"><strong>{surah.name}</strong><span>{surah.meaning} <span aria-hidden="true">·</span> {surah.ayahCount} ayahs</span></span>
            <span class="surah-arabic"><span lang="ar" dir="rtl">{surah.arabic}</span></span>
            <span class="row-arrow"><Icon name="arrow-right" size={16} /></span>
          </a>
        </li>
      {/each}
    </ul>
  </section>

  <section id="why" class="why-section" aria-labelledby="why-heading">
    <div class="why-inner container">
      <div class="why-copy"><h2 id="why-heading">More room<br /> for the words.</h2><p>A clear page, thoughtful tools, and nothing competing for your attention.</p><a class="text-link" href={publicHref("/about")}>Why we built EasyQuran <Icon name="arrow-right" size={18} /></a></div>
      <div class="reading-principles">
        <div><Icon name="book" size={24} /><h3>Read at your pace</h3><p>Clear Arabic script. Adjustable type. Translations alongside the text when you want them.</p></div>
        <div><Icon name="bookmark" size={24} /><h3>Keep your place</h3><p>Save the verses you want to return to. Pick up your reading where you left off.</p></div>
        <div><Icon name="shield" size={24} /><h3>Come as you are</h3><p>Start without an account. Free to read, with no ads between you and the page.</p></div>
      </div>
    </div>
  </section>

  <section class="closing container" aria-labelledby="closing-heading">
    <div><h2 id="closing-heading">A chapter. A moment. Your time.</h2><p>There’s no right amount to read. Just a place to begin.</p></div>
    <a class="button primary" href={publicHref("/surah")}>Open the reader <Icon name="arrow-right" size={19} /></a>
  </section>

  <footer class="site-footer container">
    <div><Brand /><p>Free to read. Made with care.</p></div>
    <nav aria-label="Footer"><a href={publicHref("/about")}>About</a><a href={publicHref("/contact")}>Contact</a><a href={publicHref("/privacy")}>Privacy</a><a href={publicHref("/terms")}>Terms</a></nav>
  </footer>
</div>

<style>
  .home-preview {
    --home-bg: #fdfdfd;
    --home-panel: #f5f5f5;
    --home-card: #fdfdfd;
    --home-ink: #232326;
    --home-muted: #68686e;
    --home-line: #e4e4e7;
    --home-accent: #345bd2;
    --home-accent-soft: #edf1fd;
    --home-on-accent: #fdfdfd;
    --home-radius: 12px;
    --home-font: "Onest Variable", sans-serif;
    --primary: var(--home-accent);
    --primary-foreground: var(--home-on-accent);
    --foreground: var(--home-ink);
    background: var(--home-bg);
    color: var(--home-ink);
    font-family: var(--home-font);
    font-size: 15px;
    line-height: 1.6;
    color-scheme: light;
  }
  .home-preview[data-mode="dark"] {
    --home-bg: #171719;
    --home-panel: #202023;
    --home-card: #1c1c1f;
    --home-ink: #ededee;
    --home-muted: #aaaab1;
    --home-line: #37373c;
    --home-accent: #a1b7ff;
    --home-accent-soft: #242e4c;
    --home-on-accent: #182140;
    color-scheme: dark;
  }
  .home-preview[data-accent="graphite"] { --home-accent: #333338; --home-accent-soft: #eeeeef; }
  .home-preview[data-accent="slate"] { --home-accent: #466376; --home-accent-soft: #edf1f4; }
  .home-preview[data-accent="berry"] { --home-accent: #934b6a; --home-accent-soft: #f7edf2; }
  .home-preview[data-mode="dark"][data-accent="graphite"] { --home-accent: #dddddf; --home-accent-soft: #2c2c30; --home-on-accent: #232326; }
  .home-preview[data-mode="dark"][data-accent="slate"] { --home-accent: #abc6d7; --home-accent-soft: #26333c; --home-on-accent: #1e2d36; }
  .home-preview[data-mode="dark"][data-accent="berry"] { --home-accent: #e5aec6; --home-accent-soft: #3a2831; --home-on-accent: #39232d; }
  .home-preview[data-surface="soft"] { --home-bg: #f4f4f5; --home-panel: #ebebee; }
  .home-preview[data-surface="soft"][data-mode="dark"] { --home-bg: #1e1e21; --home-panel: #27272c; --home-card: #242428; }
  .home-preview[data-font="nunito"] { --home-font: "Nunito Variable", sans-serif; }
  .home-preview[data-font="jetbrains"] { --home-font: "JetBrains Mono Variable", monospace; font-size: 13px; }
  .container { width: min(1120px, calc(100% - 96px)); margin-inline: auto; }
  .home-preview :global(a), .home-preview button, .home-preview input { -webkit-tap-highlight-color: transparent; }
  .home-preview :global(a:focus-visible), .home-preview button:focus-visible, .home-preview input:focus-visible { outline: 2px solid var(--home-accent); outline-offset: 5px; }
  .home-preview h1, .home-preview h2, .home-preview h3, .home-preview p { margin: 0; }
  .home-preview h1, .home-preview h2, .home-preview h3 { color: var(--home-ink); font-family: inherit; }
  .home-preview a { text-decoration: none; }
  .site-header { display: flex; align-items: center; gap: 36px; height: 80px; border-bottom: 1px solid var(--home-line); }
  .site-header nav { display: flex; align-items: center; gap: 30px; margin-inline-start: auto; font-size: 13px; font-weight: 500; }
  .site-header nav a { color: var(--home-muted); }
  .site-header nav a:hover, .site-footer nav a:hover { color: var(--home-accent); }
  .sign-in { display: inline-flex; align-items: center; gap: 10px; color: var(--home-ink); font-size: 13px; font-weight: 600; }
  .hero { display: grid; grid-template-columns: 1.15fr 0.85fr; gap: 70px; align-items: center; padding-block: 64px 40px; }
  .hero-kicker { color: var(--home-muted); font-size: 12px; font-weight: 500; letter-spacing: 0.025em; }
  .hero h1 { font-size: clamp(42px, 4.8vw, 64px); letter-spacing: -0.055em; line-height: 1.11; font-weight: 580; padding-block: 22px; }
  .hero h1 span { color: var(--home-accent); }
  .intro { max-width: 37ch; color: var(--home-muted); font-size: 17px; line-height: 1.8; }
  .hero-actions { display: flex; align-items: center; flex-wrap: wrap; gap: 26px; margin-top: 30px; }
  .button { display: inline-flex; align-items: center; justify-content: center; gap: 18px; min-height: 48px; padding: 12px 20px; border-radius: calc(var(--home-radius) * 0.67); font-size: 13px; font-weight: 600; white-space: nowrap; }
  .primary { background: var(--home-accent); color: var(--home-on-accent); }
  .button:hover { opacity: 0.88; }
  .button:active, .round-link:active { transform: translateY(1px); }
  .text-link { display: inline-flex; align-items: center; gap: 12px; font-size: 13px; font-weight: 550; color: var(--home-ink); }
  .text-link:hover { color: var(--home-accent); }
  .opening-panel { display: flex; flex-direction: column; min-height: 302px; background: var(--home-panel); border-radius: var(--home-radius); padding: 26px; }
  .opening-meta { display: flex; align-items: center; justify-content: space-between; gap: 15px; color: var(--home-muted); font-size: 11px; }
  .bismillah { display: flex; flex: 1; align-items: center; justify-content: center; font-family: var(--font-arabic); font-size: clamp(25px, 2.6vw, 36px); line-height: 2.3; padding-block: 27px; color: var(--home-ink); }
  .opening-bottom { display: flex; align-items: center; justify-content: space-between; border-top: 1px solid var(--home-line); padding-top: 22px; gap: 16px; }
  .opening-name { font-size: 15px; font-weight: 600; }
  .opening-detail { color: var(--home-muted); font-size: 11px; margin-top: 3px !important; }
  .round-link { display: grid; place-items: center; width: 42px; height: 42px; border-radius: calc(var(--home-radius) * 0.67); background: var(--home-accent); color: var(--home-on-accent); }
  .search-section { display: flex; align-items: center; gap: 28px; padding-bottom: 40px; }
  .search-form { display: flex; align-items: center; gap: 16px; flex: 1; min-width: 0; border: 1px solid var(--home-line); background: var(--home-card); border-radius: calc(var(--home-radius) * 0.67); padding: 8px 8px 8px 20px; color: var(--home-muted); }
  .search-form input { flex: 1; min-width: 0; width: 100%; background: transparent; color: var(--home-ink); border: 0; font-family: inherit; font-size: 13px; }
  .search-form input::placeholder { color: var(--home-muted); opacity: 1; }
  .search-form:focus-within { border-color: var(--home-accent); }
  .search-form button { display: inline-flex; align-items: center; justify-content: center; gap: 14px; min-height: 40px; padding: 8px 17px; background: var(--home-panel); color: var(--home-ink); border-radius: calc(var(--home-radius) * 0.4); font-size: 12px; font-weight: 600; cursor: pointer; }
  .search-form button:hover { background: var(--home-accent-soft); color: var(--home-accent); }
  .reader-shortcuts { display: grid; grid-template-columns: repeat(4, 1fr); border-block: 1px solid var(--home-line); }
  .reader-shortcuts a { display: flex; align-items: center; gap: 12px; color: var(--home-muted); padding: 22px 24px; font-size: 12px; }
  .reader-shortcuts a:first-child { padding-inline-start: 0; }
  .reader-shortcuts a:last-child { padding-inline-end: 0; }
  .reader-shortcuts a + a { border-inline-start: 1px solid var(--home-line); }
  .reader-shortcuts strong { font-weight: 600; color: var(--home-ink); }
  .reader-shortcuts a :global(svg:last-child) { margin-inline-start: auto; }
  .reader-shortcuts a:hover { color: var(--home-accent); }
  .browse { padding-block: 68px 74px; scroll-margin-top: 160px; }
  .section-heading { display: flex; align-items: center; justify-content: space-between; gap: 20px; margin-bottom: 28px; }
  .home-preview h2 { font-size: 32px; font-weight: 550; letter-spacing: -0.045em; line-height: 1.23; }
  .section-heading p { margin-top: 10px; font-size: 13px; color: var(--home-muted); }
  .browse-filters { display: flex; gap: 26px; border-bottom: 1px solid var(--home-line); margin-bottom: 18px; }
  .browse-filters button { position: relative; padding: 0 0 13px; font-size: 12px; font-weight: 500; color: var(--home-muted); cursor: pointer; }
  .browse-filters button[aria-pressed="true"] { color: var(--home-accent); }
  .browse-filters button[aria-pressed="true"]::after { position: absolute; content: ""; height: 2px; background: var(--home-accent); bottom: -1px; inset-inline: 0; }
  .surah-list { list-style: none; display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; padding: 0; margin: 0; }
  .surah-row { display: flex; align-items: center; gap: 14px; height: 92px; padding: 16px; background: var(--home-card); border: 1px solid var(--home-line); border-radius: calc(var(--home-radius) * 0.67); color: var(--home-ink); }
  .surah-row:hover { border-color: var(--home-accent); background: var(--home-accent-soft); }
  .surah-number { font-family: "JetBrains Mono Variable", monospace; font-size: 11px; font-weight: 400; color: var(--home-muted); }
  .surah-copy { display: flex; flex-direction: column; min-width: 0; gap: 4px; }
  .surah-copy strong { font-size: 13px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .surah-copy > span { font-size: 10px; color: var(--home-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .surah-arabic { margin-inline-start: auto; font-family: var(--font-arabic); font-size: 23px; color: var(--home-muted); white-space: nowrap; }
  .row-arrow { display: none; color: var(--home-muted); }
  .why-section { background: var(--home-panel); }
  .why-inner { display: grid; grid-template-columns: 0.9fr 1.1fr; gap: 110px; padding-block: 62px; }
  .why-copy h2 { font-size: 40px; }
  .why-copy > p { margin-block: 20px 28px; max-width: 31ch; font-size: 14px; line-height: 1.9; color: var(--home-muted); }
  .reading-principles { display: flex; flex-direction: column; gap: 26px; }
  .reading-principles > div { display: grid; grid-template-columns: 24px 1fr; column-gap: 20px; row-gap: 7px; align-items: center; }
  .reading-principles :global(svg) { color: var(--home-accent); }
  .reading-principles h3 { font-size: 15px; font-weight: 600; letter-spacing: -0.025em; }
  .reading-principles p { grid-column: 2; color: var(--home-muted); max-width: 45ch; font-size: 12px; line-height: 1.9; }
  .closing { display: flex; justify-content: space-between; align-items: center; gap: 30px; padding-block: 68px; }
  .closing h2 { font-size: 28px; }
  .closing p { margin-top: 12px; font-size: 13px; color: var(--home-muted); }
  .site-footer { border-top: 1px solid var(--home-line); padding-block: 30px; display: flex; align-items: center; justify-content: space-between; gap: 28px; }
  .site-footer p { margin-top: 10px; color: var(--home-muted); font-size: 11px; }
  .site-footer nav { display: flex; flex-wrap: wrap; gap: 26px; color: var(--home-muted); font-size: 12px; }

  .home-preview[data-variant="mono"] { --home-radius: 6px; }
  [data-variant="mono"] .hero { display: block; text-align: center; padding-block: 66px 30px; }
  [data-variant="mono"] .hero h1 { font-size: clamp(44px, 5.2vw, 68px); font-weight: 480; }
  [data-variant="mono"] .hero h1 br { display: none; }
  [data-variant="mono"] .hero h1 span { display: block; }
  [data-variant="mono"] .intro { margin-inline: auto; max-width: 46ch; }
  [data-variant="mono"] .hero-actions { justify-content: center; }
  [data-variant="mono"] .search-section { max-width: 660px; padding-block: 12px 42px; }
  [data-variant="mono"] .surah-list { grid-template-columns: repeat(2, minmax(0, 1fr)); column-gap: 44px; row-gap: 0; }
  [data-variant="mono"] .surah-row { padding-inline: 4px; border: 0; border-bottom: 1px solid var(--home-line); border-radius: 0; background: transparent; }
  [data-variant="mono"] .row-arrow { display: block; margin-inline-start: 15px; }
  [data-variant="mono"] .surah-number { min-width: 24px; }
  [data-variant="mono"] .why-section { background: transparent; border-block: 1px solid var(--home-line); }
  [data-variant="mono"] .closing { text-align: center; flex-direction: column; }

  .home-preview[data-variant="slate"] { --home-radius: 3px; }
  [data-variant="slate"] .hero { gap: 80px; padding-block: 52px 36px; grid-template-columns: 1.1fr 0.9fr; }
  [data-variant="slate"] .hero h1 { font-size: clamp(35px, 3.8vw, 50px); letter-spacing: -0.07em; }
  [data-variant="slate"] .intro { font-size: 13px; }
  [data-variant="slate"] .opening-panel { min-height: 268px; border: 1px solid var(--home-line); background: transparent; }
  [data-variant="slate"] .opening-meta { font-size: 10px; }
  [data-variant="slate"] .hero-kicker { font-size: 10px; }
  [data-variant="slate"] .hero-actions { gap: 20px; }
  [data-variant="slate"] .hero-actions .text-link { font-size: 11px; }
  [data-variant="slate"] .surah-list { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0; border-top: 1px solid var(--home-line); }
  [data-variant="slate"] .surah-row { height: 88px; background: transparent; border: 0; border-bottom: 1px solid var(--home-line); padding-inline: 12px 24px; }
  [data-variant="slate"] .surah-list li:nth-child(even) { border-inline-start: 1px solid var(--home-line); }
  [data-variant="slate"] .row-arrow { display: block; }
  [data-variant="slate"] .browse { padding-block: 46px 52px; }
  [data-variant="slate"] h2 { font-size: 28px; letter-spacing: -0.06em; }
  [data-variant="slate"] .why-inner { padding-block: 48px; gap: 64px; }

  .home-preview[data-variant="berry"] { --home-radius: 18px; }
  [data-variant="berry"] .hero { margin-top: 40px; padding: 40px; gap: 40px; background: var(--home-card); border-radius: var(--home-radius); }
  [data-variant="berry"] .hero h1 { font-size: clamp(38px, 4.5vw, 58px); }
  [data-variant="berry"] .opening-panel { background: var(--home-accent-soft); }
  [data-variant="berry"] .search-section { padding-block: 24px 32px; }
  [data-variant="berry"] .reader-shortcuts { border-top: 0; }
  [data-variant="berry"] .why-section { width: min(1120px, calc(100% - 96px)); margin-inline: auto; border-radius: var(--home-radius); background: var(--home-card); }
  [data-variant="berry"] .why-inner { width: auto; padding: 44px; gap: 72px; }
  [data-variant="berry"] .why-copy h2 { font-size: 36px; }

  [data-font="jetbrains"] .hero h1 { font-size: clamp(34px, 3.5vw, 47px); }
  [data-font="jetbrains"] .intro { font-size: 13px; }
  [data-font="jetbrains"] .surah-list { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  [data-font="jetbrains"] .hero-actions { gap: 20px; }
  [data-font="jetbrains"] .text-link { font-size: 11px; }

  @media (max-width: 1100px) {
    .container { width: calc(100% - 64px); }
    .hero { gap: 35px; }
    .hero-actions { gap: 18px; }
    .text-link { font-size: 12px; }
    .surah-list { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .reader-shortcuts a { padding-inline: 18px; gap: 9px; font-size: 11px; }
    .reader-shortcuts a :global(svg:last-child) { display: none; }
    .why-inner { gap: 60px; }
    [data-variant="slate"] .hero { gap: 30px; }
    [data-variant="berry"] .hero { padding: 30px; gap: 30px; }
    [data-variant="berry"] .why-section { width: calc(100% - 64px); }
  }

  @media (max-width: 767px) {
    .container { width: calc(100% - 40px); }
    .site-header { height: 76px; gap: 20px; }
    .site-header nav { display: none; }
    .sign-in { margin-inline-start: auto; font-size: 12px; }
    .hero, [data-variant="slate"] .hero { grid-template-columns: 1fr; gap: 30px; padding-block: 40px 26px; }
    .hero h1 { font-size: clamp(38px, 12vw, 48px); padding-block: 18px; }
    .hero h1 br { display: none; }
    .hero h1 span { display: block; }
    .intro { font-size: 15px; max-width: 40ch; }
    .hero-actions { gap: 20px; margin-top: 24px; }
    .opening-panel, [data-variant="slate"] .opening-panel { min-height: 210px; padding: 20px; }
    .bismillah { font-size: 31px; padding-block: 22px; }
    .opening-bottom { padding-top: 14px; }
    .search-section { padding-block: 0 26px; }
    .search-form { padding-inline-start: 13px; gap: 10px; }
    .search-form input { font-size: 12px; }
    .search-form button { padding-inline: 12px; }
    .search-form button :global(svg) { display: none; }
    .reader-shortcuts { grid-template-columns: repeat(2, 1fr); }
    .reader-shortcuts a { padding: 16px 12px !important; font-size: 12px; }
    .reader-shortcuts a:nth-child(3) { border-inline-start: 0; }
    .reader-shortcuts a:nth-child(n + 3) { border-top: 1px solid var(--home-line); }
    .browse { padding-block: 40px 44px; scroll-margin-top: 220px; }
    .section-heading { align-items: flex-start; flex-direction: column; gap: 16px; }
    .home-preview h2 { font-size: 29px; }
    .section-heading p { font-size: 12px; }
    .browse-filters { gap: 24px; }
    .browse-filters button { font-size: 11px; min-height: 36px; }
    .surah-list, [data-variant="mono"] .surah-list, [data-variant="slate"] .surah-list, [data-font="jetbrains"] .surah-list { grid-template-columns: minmax(0, 1fr); }
    .surah-row { height: 84px; }
    .surah-copy strong { font-size: 14px; }
    .surah-copy > span { font-size: 11px; }
    .row-arrow { display: block; margin-inline-start: 8px; }
    .why-inner, [data-variant="slate"] .why-inner { grid-template-columns: 1fr; gap: 34px; padding-block: 36px; }
    .why-copy h2 { font-size: 34px; }
    .why-copy h2 br { display: none; }
    .why-copy > p { max-width: 38ch; margin-block: 14px 20px; }
    .reading-principles { gap: 24px; }
    .reading-principles p { font-size: 12px; }
    .closing { flex-direction: column; align-items: flex-start; padding-block: 40px; gap: 24px; }
    .closing h2 { font-size: 27px; max-width: 20ch; }
    .closing p { max-width: 32ch; line-height: 1.9; }
    .site-footer { align-items: flex-start; flex-direction: column; gap: 22px; }
    .site-footer nav { gap: 24px; }
    [data-variant="mono"] .hero { padding-block: 40px 24px; }
    [data-variant="mono"] .hero h1 { font-size: clamp(36px, 12vw, 47px); }
    [data-variant="mono"] .closing { align-items: center; }
    [data-variant="slate"] .surah-list li:nth-child(even) { border-inline-start: 0; }
    [data-variant="berry"] .hero { width: calc(100% - 24px); margin-top: 16px; padding: 28px 20px 20px; gap: 26px; }
    [data-variant="berry"] .hero h1 { font-size: clamp(35px, 11vw, 43px); }
    [data-variant="berry"] .why-section { width: calc(100% - 24px); }
    [data-variant="berry"] .why-inner { grid-template-columns: 1fr; padding: 28px 20px; gap: 30px; }
    [data-variant="berry"] .why-copy h2 { font-size: 30px; }
    [data-font="jetbrains"] .hero h1 { font-size: clamp(29px, 8.9vw, 35px); }
  }

  @media (prefers-reduced-motion: no-preference) {
    .button, .round-link, .surah-row { transition: background-color 140ms, border-color 140ms, transform 140ms, opacity 140ms; }
  }
</style>
