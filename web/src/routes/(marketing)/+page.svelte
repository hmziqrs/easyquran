<!--
  Home (marketing index) — the five sections from the design comp (lines 66–202):
  hero, app-preview mockup, three feature cards, the "three taps" split with the
  verse-of-the-day card, and the final CTA. All inline styles in the comp are
  translated to Tailwind utilities over our tokens; Arabic runs `font-arabic`
  + dir="rtl".
-->
<script lang="ts">
  import { Button, Container, Icon, Seo } from "$lib/components";
  import { SURAHS, VERSE_OF_DAY, surahByNum } from "$lib/data/quran";
  import type { IconName } from "$lib/components";

  // The four surahs shown in the preview's mini-list (matches the comp order).
  const previewSurahs = [1, 2, 103, 112].map((n) => surahByNum(n));
  // Al-Fatihah, for the preview verse card.
  const fatihah = SURAHS[0];

  interface Feature {
    icon: IconName;
    chip: string;
    title: string;
    body: string;
  }

  const features: Feature[] = [
    {
      icon: "search",
      chip: "bg-accent text-accent-fg",
      title: "Instant search",
      body: "Type a surah name, a number, or a few Arabic words. Results appear as you type.",
    },
    {
      icon: "bookmark",
      chip: "bg-fg text-bg",
      title: "Bookmarks & notes",
      body: "Save the verses you return to and write your own reflection beside them.",
    },
    {
      icon: "book",
      chip: "bg-pop text-accent-fg",
      title: "Picks up where you left",
      body: "Your last read position is remembered, so you never hunt for your place.",
    },
  ];

  const steps = [
    {
      n: 1,
      title: "Open — straight into the text",
      body: "No tour, no friction. The reader is the first thing you see.",
    },
    {
      n: 2,
      title: "Pick a surah, or continue",
      body: "The list is always one click away, and “continue reading” sits at the top.",
    },
    {
      n: 3,
      title: "Read at your own size",
      body: "Scale the Arabic script, dim the page, and let the recitation follow along.",
    },
  ];

  // The comp's lift shadow, themed for light + dark.
  const lift =
    "shadow-[0_24px_50px_-22px_rgba(0,0,0,0.22)] dark:shadow-[0_24px_50px_-22px_rgba(0,0,0,0.6)]";
</script>

<Seo path="/" />

<!-- 1 · Hero -->
<section class="pt-22 pb-7">
  <Container class="max-w-[1180px] flex flex-col items-center gap-[22px] text-center">
    <span
      class="inline-flex items-center gap-2 rounded-full bg-accent-soft px-3.5 py-[7px] text-[13px] font-medium text-accent"
    >
      Free forever · no ads · works offline
    </span>
    <h1
      class="max-w-[19ch] text-balance text-5xl leading-[1.05] tracking-tight md:text-6xl"
    >
      The simplest way to read the Qur'an.
    </h1>
    <p class="max-w-[52ch] text-pretty text-[19px] leading-[1.6] text-fg-2">
      Open it, read, close it. Search, bookmarks and recitation are there when you
      need them — invisible when you don't.
    </p>
    <div class="mt-1.5 flex flex-wrap justify-center gap-2.5">
      <Button variant="accent" size="lg" href="/app">Start reading</Button>
      <Button variant="ghost" size="lg" href="/about">Why we built it</Button>
    </div>
  </Container>
</section>

<!-- 2 · App preview mockup -->
<section class="pt-11">
  <Container class="max-w-[1180px]">
    <div
      class="flex w-full max-w-[880px] flex-col gap-3.5 rounded-t-[18px] border border-b-0 border-line-2 bg-bg-1 px-5 pb-10 pt-5 {lift}"
    >
      <a
        href="/app"
        class="flex items-center gap-2.5 rounded-[10px] border border-line-2 bg-bg-3 px-3.5 py-3 text-[15px] text-fg-3 transition-colors hover:border-line-3 hover:text-fg-2"
      >
        <Icon name="search" size={15} class="shrink-0" />
        <span>Search a surah, verse or word…</span>
        <kbd
          class="ml-auto rounded-[5px] bg-bg-2 px-[7px] py-[3px] font-mono text-xs text-fg-3"
          >⌘K</kbd
        >
      </a>

      <div class="grid grid-cols-1 gap-3.5 md:grid-cols-[220px_1fr]">
        <!-- surah mini-list -->
        <div class="flex flex-col gap-[5px]">
          {#each previewSurahs as s, i (s.num)}
            <div
              class="flex items-center justify-between rounded-[9px] px-[13px] py-[11px] {i === 0
                ? 'bg-accent-soft'
                : ''}"
            >
              <span
                class="text-sm {i === 0 ? 'font-semibold text-accent' : 'text-fg-2'}"
              >
                {s.num} · {s.name}
              </span>
              <span
                dir="rtl"
                class="font-arabic text-right text-[16px] {i === 0
                  ? 'text-accent'
                  : 'text-fg-3'}"
              >
                {s.arabic}
              </span>
            </div>
          {/each}
        </div>

        <!-- verse card -->
        <div
          class="flex flex-col gap-[18px] rounded-xl border border-line-2 bg-bg-3 px-[30px] py-[26px]"
        >
          <div class="flex items-center justify-between">
            <span class="text-xs font-semibold uppercase tracking-[0.1em] text-accent"
              >Al-Fatihah</span
            >
            <div class="flex gap-[7px]" aria-hidden="true">
              <span class="size-[26px] rounded-[7px] bg-accent-soft"></span>
              <span class="size-[26px] rounded-[7px] bg-bg-2"></span>
            </div>
          </div>
          <p
            dir="rtl"
            class="font-arabic text-right text-[31px] leading-[2.1] text-fg"
          >
            {fatihah.verses[0]}
          </p>
          <div class="h-px bg-line"></div>
          <p
            dir="rtl"
            class="font-arabic text-right text-[31px] leading-[2.1] text-fg opacity-55"
          >
            {fatihah.verses[1]}
          </p>
        </div>
      </div>
    </div>
  </Container>
</section>

<!-- 3 · Feature cards -->
<section class="py-16">
  <Container class="max-w-[1180px]">
    <div class="grid grid-cols-1 gap-[18px] md:grid-cols-3">
      {#each features as f (f.title)}
        <div class="flex flex-col gap-2 rounded-[14px] bg-bg-2 px-7 py-[30px]">
          <div
            class="mb-2 flex size-8 items-center justify-center rounded-[9px] {f.chip}"
          >
            <Icon name={f.icon} size={16} />
          </div>
          <div class="text-[18px] font-semibold">{f.title}</div>
          <p class="text-[15px] leading-[1.6] text-fg-2">{f.body}</p>
        </div>
      {/each}
    </div>
  </Container>
</section>

<!-- 4 · Three taps + verse of the day -->
<section class="border-y border-line bg-bg-2">
  <Container
    class="max-w-[1180px] grid grid-cols-1 items-center gap-12 py-[76px] md:grid-cols-2 md:gap-[64px]"
  >
    <div class="flex flex-col gap-[26px]">
      <h2 class="max-w-[20ch] text-[40px] leading-[1.12] tracking-[-0.03em]">
        Three taps from opening the app to reading.
      </h2>
      <div class="flex flex-col gap-5">
        {#each steps as step (step.n)}
          <div class="flex gap-4">
            <div
              class="flex size-7 flex-none items-center justify-center rounded-lg bg-accent-soft text-sm font-semibold text-accent"
            >
              {step.n}
            </div>
            <div class="flex flex-col gap-[3px]">
              <div class="text-[17px] font-semibold">{step.title}</div>
              <p class="text-[15px] leading-[1.6] text-fg-2">{step.body}</p>
            </div>
          </div>
        {/each}
      </div>
    </div>

    <!-- verse of the day -->
    <div
      class="flex flex-col gap-6 rounded-xl border border-line-2 bg-bg-3 px-[46px] py-[44px] {lift}"
    >
      <div
        class="flex items-center justify-between text-xs uppercase tracking-[0.12em]"
      >
        <span class="text-pop">{VERSE_OF_DAY.ref}</span>
        <span class="text-fg-3">{VERSE_OF_DAY.caption}</span>
      </div>
      <p
        dir="rtl"
        class="font-arabic text-right text-[37px] leading-[2.05] text-fg"
      >
        {VERSE_OF_DAY.arabic}
      </p>
      <div class="h-px bg-line"></div>
      <div class="flex items-center gap-3.5">
        <button
          type="button"
          aria-label="Play verse of the day recitation"
          class="flex size-[42px] flex-none items-center justify-center rounded-full bg-accent text-accent-fg transition-[filter] hover:brightness-110"
        >
          <Icon name="play" size={14} class="ml-0.5" />
        </button>
        <div class="flex flex-1 flex-col gap-[7px]">
          <div class="h-1 overflow-hidden rounded-full bg-line-2">
            <div class="h-full w-[38%] rounded-full bg-pop"></div>
          </div>
          <div class="flex justify-between text-xs text-fg-3">
            <span>{VERSE_OF_DAY.reciter}</span>
            <span>0:14 / 0:37</span>
          </div>
        </div>
      </div>
    </div>
  </Container>
</section>

<!-- 5 · Final CTA -->
<section class="pt-20 pb-24">
  <Container class="max-w-[1180px] flex flex-col items-center gap-5 text-center">
    <h2 class="text-[38px] tracking-[-0.03em]">Start with Al-Fatihah.</h2>
    <p class="max-w-[44ch] text-[17px] leading-[1.6] text-fg-2">
      Today it runs right in your browser &mdash; nothing to install. Native desktop
      and mobile apps are next, with hadith, audio and translations to follow.
    </p>
    <div class="mt-1.5">
      <Button variant="accent" size="lg" href="/app">Open the app</Button>
    </div>
  </Container>
</section>
