<!--
  Terms of Service — mirrors comp lines 500–517. Same skeleton as /privacy
  minus the short-version callout: narrow 720px column, brand eyebrow, big
  h1, "last updated" meta, the TERMS_SECTIONS rendered as h2 + body blocks,
  and a "see also" footer line linking to /privacy.
-->
<script lang="ts">
  import { Container, Eyebrow, Seo } from "$lib/components";
  import { LEGAL_UPDATED, TERMS_SECTIONS } from "$lib/data/content";
</script>

<Seo path="/terms" />

<Container class="max-w-[1180px] pt-[72px] pb-[96px]">
  <div class="flex w-full flex-col gap-[26px]">
  <div class="flex flex-col gap-2.5">
    <Eyebrow class="text-accent">Legal</Eyebrow>
    <h1 class="text-[36px] leading-[1.1] tracking-[-0.03em] sm:text-[42px]">
      Terms of Service
    </h1>
    <p class="text-sm text-fg-3">
      Last updated {LEGAL_UPDATED} &middot; placeholder text, not legal advice
    </p>
  </div>

  <div class="flex flex-col gap-[26px]">
    {#each TERMS_SECTIONS as s (s.h)}
      <div class="flex flex-col gap-[9px]">
        <h2 class="text-[19px]">{s.h}</h2>
        <p class="text-base leading-[1.75] text-fg-2">{s.p}</p>
      </div>
    {/each}
  </div>

  <div class="h-px bg-line-2"></div>

  <p class="text-[15px] text-fg-2">
    See also our
    <a
      href="/privacy"
      class="text-accent underline underline-offset-2 hover:text-accent/80"
    >
      Privacy Policy</a
    >.
  </p>
  </div>
</Container>
