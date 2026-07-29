<!--
  Home (marketing index) — the hero only. A fuller demo (app-preview mockup,
  feature cards, "three taps" + verse-of-the-day, final CTA) was removed because
  it advertised features (search, bookmarks, recitation) that aren't built yet.
-->
<script lang="ts">
  import { Button, Container, Seo } from "$lib/components";
</script>

<Seo path="/" />

<!-- Hero -->
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
