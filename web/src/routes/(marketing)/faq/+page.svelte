<!--
  FAQ — accordion of the most common questions. Mirrors comp lines 395–416:
  narrow column, brand eyebrow, big h1, a single-collapsible Accordion over
  FAQS (question = trigger + plus/minus Icon, answer = relaxed body text),
  and a "didn't find it?" footer line that links to /contact.
-->
<script lang="ts">
  import { Container, Eyebrow, Icon, Seo } from "$lib/components";
  import {
    Accordion,
    AccordionContent,
    AccordionItem,
    AccordionTrigger,
  } from "$lib/components/ui/accordion";
  import { FAQS } from "$lib/data/content";

  // Single-open, fully collapsible. `undefined` = every item closed.
  let value = $state<string | undefined>(undefined);
</script>

<Seo path="/faq" faq={FAQS} />

<Container class="max-w-[1180px] pt-[72px] pb-[96px]">
  <div class="flex w-full flex-col gap-[30px]">
  <div class="flex flex-col gap-3">
    <Eyebrow class="text-accent">FAQ</Eyebrow>
    <h1 class="text-[40px] leading-[1.1] tracking-[-0.03em] sm:text-[46px]">
      Questions, answered plainly.
    </h1>
  </div>

  <Accordion type="single" bind:value class="border-t border-line-2">
    {#each FAQS as f, i (f.q)}
      {@const open = value === String(i)}
      <AccordionItem value={String(i)} class="border-line-2">
        <AccordionTrigger
          class="flex-row items-center gap-[18px] rounded-none border-0 px-1 py-[22px] hover:no-underline [&_[data-slot=accordion-trigger-icon]]:hidden"
        >
          <span class="flex-1 text-left text-[17.5px] font-medium text-fg">{f.q}</span>
          <Icon
            name={open ? "minus" : "plus"}
            size={19}
            class="shrink-0 text-fg-3"
          />
        </AccordionTrigger>
        <AccordionContent class="px-1 pb-6 pr-[60px]">
          <p class="text-base leading-[1.7] text-fg-2">{f.a}</p>
        </AccordionContent>
      </AccordionItem>
    {/each}
  </Accordion>

  <p class="text-base text-fg-2">
    Didn&rsquo;t find it?
    <a
      href="/contact"
      class="text-accent underline underline-offset-2 hover:text-accent/80"
    >
      Send us a question</a
    >.
  </p>
  </div>
</Container>
