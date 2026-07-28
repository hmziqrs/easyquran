<!--
  About — what EasyQuran is, who it's for, and the device-first ethos behind
  it. A single narrow column of reading copy, three stat tiles, and a contact
  CTA. Comp lines 361–393.
-->
<script lang="ts">
  import { Button, Container, Eyebrow, Seo } from "$lib/components";
  import { ABOUT_STATS } from "$lib/data/content";
</script>

<Seo path="/about" schemaSubtype="AboutPage" />

<Container width="narrow" class="flex max-w-[760px] flex-col gap-[30px] pt-[72px] pb-24">
  <div class="flex flex-col gap-3.5">
    <Eyebrow class="text-accent">About</Eyebrow>
    <h1 class="text-[40px] leading-[1.1] tracking-[-0.03em] sm:text-[46px]">
      A reader, not a platform.
    </h1>
    <p class="text-lg leading-relaxed text-fg-2">
      easyquran is a small, free Qur&rsquo;an reader for everyday use &mdash; the kind you open for
      five minutes after Fajr and don&rsquo;t think about again.
    </p>
  </div>

  <div aria-hidden="true" class="border-t border-line"></div>

  <div class="flex flex-col gap-[22px] text-base leading-relaxed text-fg-2">
    <p>
      Most Qur&rsquo;an apps grow until they are libraries: feeds, streaks, leaderboards, five kinds
      of notification. Useful for some people. We wanted the opposite &mdash; a page that opens fast,
      shows the script clearly, and gets out of the way.
    </p>
    <p>
      So the whole app is three things: a list of surahs, a reader, and a place to keep what you
      bookmark. Search is one keystroke. Notes are yours and stay on your device. There is no
      account to make, because we don&rsquo;t need to know who you are to show you a page.
    </p>
    <p>
      The Arabic text follows the Uthmani script. Recitation, tafsir and translations are added
      carefully and credited &mdash; we&rsquo;d rather ship fewer sources well than many sources loosely.
    </p>
  </div>

  <div class="grid grid-cols-3 gap-3.5 pt-2">
    {#each ABOUT_STATS as stat (stat.label)}
      <div class="flex flex-col gap-1.5 rounded-xl bg-bg-2 p-5">
        <span class="text-[1.625rem] font-semibold tracking-tight text-fg">{stat.value}</span>
        <span class="text-sm text-fg-2">{stat.label}</span>
      </div>
    {/each}
  </div>

  <div class="mt-2 flex flex-wrap items-center gap-5 rounded-xl border border-line px-[30px] py-[26px]">
    <p class="min-w-[220px] flex-1 text-base leading-relaxed text-fg-2">
      Built by two people who wanted this to exist. Suggestions genuinely get read.
    </p>
    <Button variant="accent" href="/contact">Get in touch</Button>
  </div>
</Container>
