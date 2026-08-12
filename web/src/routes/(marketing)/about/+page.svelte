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
      <Eyebrow class="text-accent">{copy.eyebrow}</Eyebrow>
      <h1 class="text-[40px] leading-[1.1] tracking-[-0.03em] sm:text-[46px]">{copy.heading}</h1>
      <p class="text-lg leading-relaxed text-fg-2">{copy.intro}</p>
    </div>

    <div aria-hidden="true" class="border-t border-line"></div>

    <div class="flex flex-col gap-[22px] text-base leading-relaxed text-fg-2">
      {#each copy.paragraphs as paragraph (paragraph.id)}
        <p>{paragraph.body}</p>
      {/each}
    </div>
  </div>

  <div class="grid grid-cols-1 gap-3.5 md:grid-cols-3">
    {#each copy.stats as stat (stat.id)}
      <div class="flex flex-col gap-1.5 rounded-xl bg-bg-2 p-5">
        <span class="text-[1.625rem] font-semibold tracking-tight text-fg">{stat.value}</span>
        <span class="text-sm text-fg-2">{stat.label}</span>
      </div>
    {/each}
  </div>

  <div class="flex flex-wrap items-center gap-5 rounded-xl border border-line px-[30px] py-[26px]">
    <p class="min-w-[220px] flex-1 text-base leading-relaxed text-fg-2">
      {copy.creditProjectBy}
      <a
        class="text-fg underline underline-offset-2 hover:text-accent"
        href={SITE.ownerUrl}
        {...externalLinkAttrs(SITE.ownerUrl, { me: true })}>hmziq.rs</a
      >, {copy.creditBuiltBy}
      <a
        class="text-fg underline underline-offset-2 hover:text-accent"
        href={SITE.makerUrl}
        {...externalLinkAttrs(SITE.makerUrl, { me: true })}>oxlabs.dev</a
      >. {copy.creditNote}
    </p>
    <Button variant="accent" href="/contact">{copy.cta}</Button>
  </div>
</Container>
