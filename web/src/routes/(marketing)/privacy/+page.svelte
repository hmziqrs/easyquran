<!--
  Privacy Policy — mirrors comp lines 479–498. Narrow 720px column, brand
  eyebrow, big h1, a "last updated" meta line, a short-version callout (the
  one piece /terms lacks), the PRIVACY_SECTIONS rendered as h2 + body blocks,
  and a "questions?" footer line linking to /contact.
-->
<script lang="ts">
  import { Container, Eyebrow, Panel, Seo } from "$lib/components";
  import { LEGAL_UPDATED, PRIVACY_SECTIONS } from "$lib/data/content";
</script>

<Seo path="/privacy" />

<Container class="max-w-[1180px] pt-[72px] pb-[96px]">
  <div class="flex w-full flex-col gap-[26px]">
  <div class="flex flex-col gap-2.5">
    <Eyebrow class="text-accent">Legal</Eyebrow>
    <h1 class="text-[36px] leading-[1.1] tracking-[-0.03em] sm:text-[42px]">
      Privacy Policy
    </h1>
    <p class="text-sm text-fg-3">
      Last updated {LEGAL_UPDATED} &middot; placeholder text, not legal advice
    </p>
  </div>

  <Panel
    variant="soft"
    class="rounded-xl border-0 px-[22px] py-5 text-[15.5px] leading-relaxed text-accent"
  >
    The short version: we don&rsquo;t collect anything about you. Bookmarks, notes and
    reading position stay in your browser.
  </Panel>

  <div class="flex flex-col gap-[26px]">
    {#each PRIVACY_SECTIONS as s (s.h)}
      <div class="flex flex-col gap-[9px]">
        <h2 class="text-[19px]">{s.h}</h2>
        <p class="text-base leading-[1.75] text-fg-2">{s.p}</p>
      </div>
    {/each}
  </div>

  <div class="h-px bg-line-2"></div>

  <p class="text-[15px] text-fg-2">
    Questions about this policy?
    <a
      href="/contact"
      class="text-accent underline underline-offset-2 hover:text-accent/80"
    >
      Contact us</a
    >.
  </p>
  </div>
</Container>
