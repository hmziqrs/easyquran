<script lang="ts">
  import { page } from "$app/state";
  import { Band, Eyebrow, Icon, Seo } from "$lib/components";
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

<Band>
  <div class="flex w-full flex-col gap-[30px]">
  <div class="flex flex-col gap-3">
    <Eyebrow>{copy.eyebrow}</Eyebrow>
    <h1 class="text-h1">{copy.heading}</h1>
  </div>

  <Accordion type="single" bind:value class="border-t border-border-strong">
    {#each copy.entries as entry, i (entry.id)}
      {@const open = value === String(i)}
      <AccordionItem value={String(i)} class="border-border-strong">
        <AccordionTrigger
          class="flex-row items-center gap-[18px] rounded-none border-0 px-1 py-[22px] hover:no-underline [&_[data-slot=accordion-trigger-icon]]:hidden"
        >
          <span class="flex-1 text-start text-body-l font-medium text-foreground">{entry.q}</span>
          <Icon
            name={open ? "minus" : "plus"}
            size={19}
            class="shrink-0 text-muted"
          />
        </AccordionTrigger>
        <AccordionContent class="px-1 pb-6 pe-[60px]">
          <p class="text-body leading-[1.7] text-foreground-secondary">{entry.a}</p>
        </AccordionContent>
      </AccordionItem>
    {/each}
  </Accordion>

  <p class="text-body text-foreground-secondary">
    {copy.missingPrompt}
    <a
      href="/contact"
      class="text-primary underline underline-offset-2 hover:text-primary-hover"
    >
      {copy.missingLink}</a
    >.
  </p>
  </div>
</Band>
