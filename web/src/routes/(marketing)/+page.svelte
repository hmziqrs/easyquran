<!--
  Home (marketing index) — hero, a "what's here today" value strip, and a
  "what's coming" roadmap. The value strip only states things that are actually
  wired now; the roadmap is framed as future work (a "Coming" tag on every card)
  so it never reads as a live feature. An earlier app-preview demo that faked
  unbuilt features (search bar, audio player, etc.) was removed for that reason.
-->
<script lang="ts">
  import { Button, Container, Eyebrow, Icon, Seo } from "$lib/components";
  import type { IconName } from "$lib/components";

  // What's genuinely in the reader today. Each claim is real, not aspirational.
  const values: { icon: IconName; chip: string; title: string; body: string }[] = [
    {
      icon: "arrow-right",
      chip: "bg-accent text-accent-fg",
      title: "Instant",
      body: "Opens straight to the text — no install, no account, no setup.",
    },
    {
      icon: "plus",
      chip: "bg-fg text-bg",
      title: "Your size",
      body: "Scale the Arabic script until it sits comfortably for your eyes.",
    },
    {
      icon: "check",
      chip: "bg-pop text-accent-fg",
      title: "Free, no ads",
      body: "No paywall, no ad tracking, and nothing you have to sign up for.",
    },
    {
      icon: "book",
      chip: "bg-accent-soft text-accent",
      title: "Authentic text",
      body: "Uthmani script — the same wording as the standard printed mushaf.",
    },
  ];

  // Planned, not shipped. Copy matches the about page + FAQ roadmap.
  const roadmap: { title: string; body: string }[] = [
    {
      title: "Native apps",
      body: "Desktop and mobile apps, so it feels at home on every device.",
    },
    {
      title: "Sync across devices",
      body: "An optional account carries your bookmarks, notes and last page wherever you read.",
    },
    {
      title: "Recitation, for real",
      body: "Full audio recitation — for now the player is only a preview.",
    },
    {
      title: "Hadith, tafsir & translations",
      body: "More to read beside the Qur'an, added carefully and each credited.",
    },
  ];
</script>

<Seo path="/" />

<!-- Hero -->
<section class="pt-22 pb-7">
  <Container class="max-w-[1180px] flex flex-col items-center gap-[22px] text-center">
    <span
      class="inline-flex items-center gap-2 rounded-full bg-accent-soft px-3.5 py-[7px] text-[13px] font-medium text-accent"
    >
      Free forever · no ads · no account needed
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

<!-- What's here today -->
<section class="py-16">
  <Container class="max-w-[1180px] flex flex-col gap-10">
    <div class="flex flex-col gap-3">
      <Eyebrow class="text-accent">Today</Eyebrow>
      <h2 class="max-w-[24ch] text-[32px] leading-[1.12] tracking-[-0.025em]">
        Small on purpose. Fast where it matters.
      </h2>
      <p class="max-w-[56ch] text-[17px] leading-[1.6] text-fg-2">
        Nothing between you and the text — here&rsquo;s what&rsquo;s already in the reader today.
      </p>
    </div>
    <div class="grid grid-cols-1 gap-[18px] sm:grid-cols-2 lg:grid-cols-4">
      {#each values as v (v.title)}
        <div class="flex flex-col gap-2 rounded-[14px] bg-bg-2 px-6 py-7">
          <div
            class="mb-2 flex size-8 items-center justify-center rounded-[9px] {v.chip}"
          >
            <Icon name={v.icon} size={16} />
          </div>
          <div class="text-[17px] font-semibold">{v.title}</div>
          <p class="text-[14.5px] leading-[1.6] text-fg-2">{v.body}</p>
        </div>
      {/each}
    </div>
  </Container>
</section>

<!-- What's coming -->
<section class="border-t border-line bg-bg-2">
  <Container class="max-w-[1180px] flex flex-col gap-10 py-[72px]">
    <div class="flex flex-col gap-3">
      <Eyebrow class="text-fg-3">On the way</Eyebrow>
      <h2 class="max-w-[22ch] text-[32px] leading-[1.12] tracking-[-0.025em]">
        A reader now. A platform next.
      </h2>
      <p class="max-w-[60ch] text-[17px] leading-[1.6] text-fg-2">
        Today the reader holds a curated selection of surahs. The full 114 chapters
        &mdash; and everything below &mdash; are on the way.
      </p>
    </div>
    <div class="grid grid-cols-1 gap-[18px] sm:grid-cols-2 lg:grid-cols-4">
      {#each roadmap as r (r.title)}
        <div class="flex flex-col gap-2 rounded-[14px] border border-line-2 bg-bg-3 px-6 py-7">
          <span
            class="text-[11px] font-semibold uppercase tracking-[0.12em] text-fg-3"
            >Coming</span
          >
          <div class="text-[17px] font-semibold">{r.title}</div>
          <p class="text-[14.5px] leading-[1.6] text-fg-2">{r.body}</p>
        </div>
      {/each}
    </div>
  </Container>
</section>
