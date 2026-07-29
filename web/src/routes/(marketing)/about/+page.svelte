<!--
  About — what EasyQuran is today and where it's heading. A single narrow
  column of reading copy, three stat tiles, and a contact CTA. Comp lines
  361–393.
-->
<script lang="ts">
  import { Button, Container, Eyebrow, Seo } from "$lib/components";
  import { ABOUT_STATS } from "$lib/data/content";
  import { SITE } from "$lib/config/site";
  import { externalLinkAttrs } from "$lib/utils";
</script>

<Seo path="/about" schemaSubtype="AboutPage" />

<!--
  Fills the same 1180px frame as the home page: heading, reading copy, stat
  tiles and CTA bar all span the full container width.
-->
<Container class="flex max-w-[1180px] flex-col gap-[40px] pt-[72px] pb-24">
  <div class="flex w-full flex-col gap-[30px]">
    <div class="flex flex-col gap-3.5">
      <Eyebrow class="text-accent">About</Eyebrow>
      <h1 class="text-[40px] leading-[1.1] tracking-[-0.03em] sm:text-[46px]">
        A reader today. A platform in the making.
      </h1>
      <p class="text-lg leading-relaxed text-fg-2">
        easyquran starts as a simple, free Qur&rsquo;an reader &mdash; the kind you open for five minutes
        after Fajr. The Arabic text, shown clearly and read fast. That&rsquo;s what&rsquo;s here today, and
        it&rsquo;s the foundation for something bigger.
      </p>
    </div>

    <div aria-hidden="true" class="border-t border-line"></div>

    <div class="flex flex-col gap-[22px] text-base leading-relaxed text-fg-2">
      <p>
        Most Qur&rsquo;an apps try to do everything at once. We&rsquo;re building easyquran the other way
        around &mdash; start with the text done right, then grow carefully around it: a page that opens
        fast, shows the script clearly, and gets out of the way.
      </p>
      <p>
        Today that means a list of surahs, a reader, search and bookmarks &mdash; Arabic text only, for
        now. On the way: hadith and their explanations, an audio player, a deeds feature, and
        translations in a selected set of languages. After that come native desktop and mobile apps, and
        the option to create an account so your bookmarks, notes and deeds follow you across them.
      </p>
      <p>
        The Arabic text follows the Uthmani script. Recitation, translations, tafsir and hadith are
        added carefully and credited &mdash; we&rsquo;d rather ship fewer sources well than many sources loosely.
      </p>
    </div>
  </div>

  <div class="grid grid-cols-1 gap-3.5 md:grid-cols-3">
    {#each ABOUT_STATS as stat (stat.label)}
      <div class="flex flex-col gap-1.5 rounded-xl bg-bg-2 p-5">
        <span class="text-[1.625rem] font-semibold tracking-tight text-fg">{stat.value}</span>
        <span class="text-sm text-fg-2">{stat.label}</span>
      </div>
    {/each}
  </div>

  <div class="flex flex-wrap items-center gap-5 rounded-xl border border-line px-[30px] py-[26px]">
    <p class="min-w-[220px] flex-1 text-base leading-relaxed text-fg-2">
      A project by
      <a
        class="text-fg underline underline-offset-2 hover:text-accent"
        href={SITE.ownerUrl}
        {...externalLinkAttrs(SITE.ownerUrl, { me: true })}>hmziq.rs</a
      >, built by
      <a
        class="text-fg underline underline-offset-2 hover:text-accent"
        href={SITE.makerUrl}
        {...externalLinkAttrs(SITE.makerUrl, { me: true })}>oxlabs.dev</a
      >. Suggestions genuinely get read.
    </p>
    <Button variant="accent" href="/contact">Get in touch</Button>
  </div>
</Container>
