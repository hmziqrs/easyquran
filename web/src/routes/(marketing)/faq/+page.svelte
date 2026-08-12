<script lang="ts">
  import { page } from "$app/state";
  import { Container, Eyebrow, Icon, Seo } from "$lib/components";
  import {
    Accordion,
    AccordionContent,
    AccordionItem,
    AccordionTrigger,
  } from "$lib/components/ui/accordion";
  import { resolveFaqCopy } from "$lib/i18n/faq-copy";
  import { marketingLocaleFromPath } from "$lib/i18n/marketing-copy";

  let value = $state<string | undefined>(undefined);

  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  const copy = $derived(resolveFaqCopy(locale));
</script>

<Seo
  path="/faq"
  faq={copy.entries}
  title={copy.seo.title}
  description={copy.seo.description}
  inLanguage={locale}
/>

<Container class="max-w-[1180px] pt-[72px] pb-[96px]">
  <div class="flex w-full flex-col gap-[30px]">
  <div class="flex flex-col gap-3">
    <Eyebrow class="text-accent">{copy.eyebrow}</Eyebrow>
    <h1 class="text-[40px] leading-[1.1] tracking-[-0.03em] sm:text-[46px]">{copy.heading}</h1>
  </div>

  <Accordion type="single" bind:value class="border-t border-line-2">
    {#each copy.entries as entry, i (entry.id)}
      {@const open = value === String(i)}
      <AccordionItem value={String(i)} class="border-line-2">
        <AccordionTrigger
          class="flex-row items-center gap-[18px] rounded-none border-0 px-1 py-[22px] hover:no-underline [&_[data-slot=accordion-trigger-icon]]:hidden"
        >
          <span class="flex-1 text-start text-[17.5px] font-medium text-fg">{entry.q}</span>
          <Icon
            name={open ? "minus" : "plus"}
            size={19}
            class="shrink-0 text-fg-3"
          />
        </AccordionTrigger>
        <AccordionContent class="px-1 pb-6 pe-[60px]">
          <p class="text-base leading-[1.7] text-fg-2">{entry.a}</p>
        </AccordionContent>
      </AccordionItem>
    {/each}
  </Accordion>

  <p class="text-base text-fg-2">
    {copy.missingPrompt}
    <a
      href="/contact"
      class="text-accent underline underline-offset-2 hover:text-accent/80"
    >
      {copy.missingLink}</a
    >.
  </p>
  </div>
</Container>
