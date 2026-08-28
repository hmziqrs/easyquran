<script lang="ts">
  import { page } from "$app/state";
  import { Band, Eyebrow, Seo } from "$lib/components";
  import { resolveTermsCopy } from "$lib/i18n/terms-copy";
  import { marketingLocaleFromPath } from "$lib/i18n/marketing-copy";

  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  const copy = $derived(resolveTermsCopy(locale));
</script>

<Seo path="/terms" title={copy.seo.title} description={copy.seo.description} inLanguage={locale} />

<Band>
  <div class="flex w-full flex-col gap-[26px]">
  <div class="flex flex-col gap-2.5">
    <Eyebrow>{copy.eyebrow}</Eyebrow>
    <h1 class="text-h1">{copy.heading}</h1>
    <p class="text-caption text-muted">{copy.updated} &middot; {copy.placeholderNote}</p>
  </div>

  <div class="flex flex-col gap-[26px]">
    {#each copy.sections as section (section.id)}
      <div class="flex flex-col gap-[9px]">
        <h2 class="text-body-l">{section.heading}</h2>
        <p class="text-body leading-[1.75] text-foreground-secondary">{section.body}</p>
      </div>
    {/each}
  </div>

  <div class="h-px bg-border-strong"></div>

  <p class="text-body-s text-foreground-secondary">
    {copy.outroPrompt}
    <a
      href="/privacy"
      class="text-primary underline underline-offset-2 hover:text-primary-hover"
    >
      {copy.outroLink}</a
    >.
  </p>
  </div>
</Band>
