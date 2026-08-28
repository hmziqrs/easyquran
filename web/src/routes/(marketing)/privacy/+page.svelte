<script lang="ts">
  import { page } from "$app/state";
  import { Container, Eyebrow, Panel, Seo } from "$lib/components";
  import { resolvePrivacyCopy, resolvePrivacySummary } from "$lib/i18n/privacy-copy";
  import { marketingLocaleFromPath } from "$lib/i18n/marketing-copy";

  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  const copy = $derived(resolvePrivacyCopy(locale));
  const summary = $derived(resolvePrivacySummary(locale));
</script>

<Seo
  path="/privacy"
  title={copy.seo.title}
  description={copy.seo.description}
  inLanguage={locale}
/>

<Container class="max-w-[1180px] pt-[72px] pb-[96px]">
  <div class="flex w-full flex-col gap-[26px]">
  <div class="flex flex-col gap-2.5">
    <Eyebrow>{copy.eyebrow}</Eyebrow>
    <h1 class="text-h1">{copy.heading}</h1>
    <p class="text-caption text-muted">{copy.updated} &middot; {copy.placeholderNote}</p>
  </div>

  <Panel
    variant="soft"
    class="rounded-md border-0 px-[22px] py-5 text-body leading-relaxed text-primary"
  >
    {summary}
  </Panel>

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
      href="/contact"
      class="text-primary underline underline-offset-2 hover:text-primary-hover"
    >
      {copy.outroLink}</a
    >.
  </p>
  </div>
</Container>
