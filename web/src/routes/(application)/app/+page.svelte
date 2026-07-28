<script lang="ts">
  import { Card, Container, Eyebrow, Seo } from "$lib/components";
  import { APP_PAGES } from "$lib/config/site";
</script>

<Seo path="/app" noindex />

<Container width="narrow" class="py-10">
  <Eyebrow>Your reading home</Eyebrow>
  <h1 class="mt-3 text-xl">Continue reading</h1>
  <p class="mt-3 text-sm text-fg-3">
    Placeholder shell. This is where the last-read position, a resume card and recent surahs go.
  </p>

  <div class="mt-8 grid gap-3">
    {#each APP_PAGES as p (p.id)}
      <a href={p.href} class="block">
        <Card interactive class="flex items-center justify-between gap-4">
          <span class="text-sm">{p.label}</span>
          <span class="text-fg-3" aria-hidden="true">→</span>
        </Card>
      </a>
    {/each}
  </div>
</Container>
