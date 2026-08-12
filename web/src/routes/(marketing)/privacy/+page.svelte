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
    <Eyebrow class="text-accent">{copy.eyebrow}</Eyebrow>
    <h1 class="text-[36px] leading-[1.1] tracking-[-0.03em] sm:text-[42px]">{copy.heading}</h1>
    <p class="text-sm text-fg-3">{copy.updated} &middot; {copy.placeholderNote}</p>
  </div>

  <Panel
    variant="soft"
    class="rounded-xl border-0 px-[22px] py-5 text-[15.5px] leading-relaxed text-accent"
  >
    {summary}
  </Panel>

  <div class="flex flex-col gap-[26px]">
    {#each copy.sections as section (section.id)}
      <div class="flex flex-col gap-[9px]">
        <h2 class="text-[19px]">{section.heading}</h2>
        <p class="text-base leading-[1.75] text-fg-2">{section.body}</p>
      </div>
    {/each}
  </div>

  <div class="h-px bg-line-2"></div>

  <p class="text-[15px] text-fg-2">
    {copy.outroPrompt}
    <a
      href="/contact"
      class="text-accent underline underline-offset-2 hover:text-accent/80"
    >
      {copy.outroLink}</a
    >.
  </p>
  </div>
</Container>
