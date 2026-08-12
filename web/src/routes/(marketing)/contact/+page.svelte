<script lang="ts">
  import { page } from "$app/state";
  import { Container, Eyebrow, Icon, Seo } from "$lib/components";
  import { resolveContactCopy } from "$lib/i18n/contact-copy";
  import { marketingLocaleFromPath } from "$lib/i18n/marketing-copy";
  import { externalLinkAttrs } from "$lib/utils";

  let { data } = $props();

  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  const copy = $derived(resolveContactCopy(locale));
</script>

<Seo
  path="/contact"
  schemaSubtype="ContactPage"
  title={copy.seo.title}
  description={copy.seo.description}
  inLanguage={locale}
/>

<Container class="flex max-w-[1180px] flex-col gap-12 pt-[72px] pb-24">
  <div class="flex w-full flex-col gap-3 text-center">
    <Eyebrow class="text-accent">{copy.eyebrow}</Eyebrow>
    <h1 class="text-[40px] leading-[1.1] tracking-[-0.03em] sm:text-[46px]">{copy.heading}</h1>
    <p class="mx-auto max-w-[60ch] text-[18px] leading-[1.65] text-fg-2">{copy.intro}</p>
  </div>

  <div class="grid w-full gap-4 md:grid-cols-2">
    <a
      href="mailto:{data.owner.email}"
      class="group flex flex-col gap-4 rounded-2xl border border-line-2 bg-bg-2 p-7 transition-colors hover:border-line-3"
    >
      <div class="flex size-9 items-center justify-center rounded-[10px] bg-accent-soft text-accent">
        <Icon name="mail" size={17} />
      </div>
      <div class="flex flex-col gap-1.5">
        <div class="text-[18px] font-semibold">{copy.emailTitle}</div>
        <p class="text-[15px] leading-[1.6] text-fg-2">{copy.emailBody}</p>
      </div>
      <div class="mt-auto flex items-center gap-2 text-[15px] font-medium text-accent">
        {data.owner.email}
        <Icon
          name="arrow-right"
          size={15}
          class="transition-transform group-hover:translate-x-0.5"
        />
      </div>
    </a>

    <a
      href={data.owner.x}
      {...externalLinkAttrs(data.owner.x, { me: true })}
      class="group flex flex-col gap-4 rounded-2xl border border-line-2 bg-bg-2 p-7 transition-colors hover:border-line-3"
    >
      <div class="flex size-9 items-center justify-center rounded-[10px] bg-fg text-bg">
        <Icon name="x-brand" size={15} />
      </div>
      <div class="flex flex-col gap-1.5">
        <div class="text-[18px] font-semibold">{copy.xTitle}</div>
        <p class="text-[15px] leading-[1.6] text-fg-2">{copy.xBody}</p>
      </div>
      <div class="mt-auto flex items-center gap-2 text-[15px] font-medium text-accent">
        {data.owner.xHandle}
        <Icon
          name="arrow-right"
          size={15}
          class="transition-transform group-hover:translate-x-0.5"
        />
      </div>
    </a>
  </div>

  <div class="flex w-full flex-col gap-[3px] rounded-xl border border-line bg-bg-2 px-[30px] py-5 text-center">
    <span class="text-xs text-fg-3">{copy.replyLabel}</span>
    <span class="text-[15px] text-fg">{copy.replyValue}</span>
  </div>
</Container>
