<script lang="ts">
  import { page } from "$app/state";
  import { Button, Container, Eyebrow, Seo } from "$lib/components";
  import { SITE } from "$lib/config/site";
  import { resolveAboutCopy } from "$lib/i18n/about-copy";
  import { marketingLocaleFromPath } from "$lib/i18n/marketing-copy";
  import { externalLinkAttrs } from "$lib/utils";

  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  const copy = $derived(resolveAboutCopy(locale));
</script>

<Seo
  path="/about"
  schemaSubtype="AboutPage"
  title={copy.seo.title}
  description={copy.seo.description}
  inLanguage={locale}
/>

<Container class="flex max-w-[1180px] flex-col gap-[40px] pt-[72px] pb-24">
  <div class="flex w-full flex-col gap-[30px]">
    <div class="flex flex-col gap-3.5">
      <Eyebrow>{copy.eyebrow}</Eyebrow>
      <h1 class="text-h1">{copy.heading}</h1>
      <p class="text-body-xl leading-relaxed text-foreground-secondary">{copy.intro}</p>
    </div>

    <div aria-hidden="true" class="border-t border-border"></div>

    <div class="flex flex-col gap-[22px] text-body leading-relaxed text-foreground-secondary">
      {#each copy.paragraphs as paragraph (paragraph.id)}
        <p>{paragraph.body}</p>
      {/each}
    </div>
  </div>

  <div class="grid grid-cols-1 gap-3.5 md:grid-cols-3">
    {#each copy.stats as stat (stat.id)}
      <div class="flex flex-col gap-1.5 rounded-md bg-background-subtle p-5">
        <span class="text-h3 font-semibold tracking-tight text-foreground">{stat.value}</span>
        <span class="text-body-s text-foreground-secondary">{stat.label}</span>
      </div>
    {/each}
  </div>

  <section class="flex flex-col gap-3 rounded-md border border-border px-[30px] py-[26px]">
    <h2 class="text-body font-semibold tracking-tight text-foreground">{copy.sourcesHeading}</h2>
    <p class="text-body leading-relaxed text-foreground-secondary">
      {copy.sourcesLead}<a
        class="text-foreground underline underline-offset-2 hover:text-primary"
        href={SITE.tanzilUrl}
        {...externalLinkAttrs(SITE.tanzilUrl)}>{copy.sourcesTanzilLabel}</a
      >{copy.sourcesTail}
    </p>
  </section>

  <div class="flex flex-wrap items-center gap-5 rounded-md border border-border px-[30px] py-[26px]">
    <p class="min-w-[220px] flex-1 text-body leading-relaxed text-foreground-secondary">
      {copy.creditProjectBy}
      <a
        class="text-foreground underline underline-offset-2 hover:text-primary"
        href={SITE.ownerUrl}
        {...externalLinkAttrs(SITE.ownerUrl, { me: true })}>hmziq.rs</a
      >, {copy.creditBuiltBy}
      <a
        class="text-foreground underline underline-offset-2 hover:text-primary"
        href={SITE.makerUrl}
        {...externalLinkAttrs(SITE.makerUrl, { me: true })}>oxlabs.dev</a
      >. {copy.creditNote}
    </p>
    <Button variant="accent" href="/contact">{copy.cta}</Button>
  </div>
</Container>
