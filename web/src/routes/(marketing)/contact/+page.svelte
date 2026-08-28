<script lang="ts">
  import { page } from "$app/state";
  import { Band, Eyebrow, Icon, Seo } from "$lib/components";
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

<Band contentClass="flex w-full flex-col gap-12">
  <div class="flex w-full flex-col gap-3 text-center">
    <Eyebrow>{copy.eyebrow}</Eyebrow>
    <h1 class="text-h1">{copy.heading}</h1>
    <p class="mx-auto max-w-[60ch] text-body-xl text-foreground-secondary">{copy.intro}</p>
  </div>

  <div class="grid w-full gap-4 md:grid-cols-2">
    <a
      href="mailto:{data.owner.email}"
      class="group flex flex-col gap-4 rounded-lg border border-border bg-background-subtle p-7 transition-colors hover:border-primary"
    >
      <div class="flex size-9 items-center justify-center rounded-md bg-primary-soft text-primary">
        <Icon name="mail" size={17} />
      </div>
      <div class="flex flex-col gap-1.5">
        <div class="text-body-l font-semibold">{copy.emailTitle}</div>
        <p class="text-body leading-[1.6] text-foreground-secondary">{copy.emailBody}</p>
      </div>
      <div class="mt-auto flex items-center gap-2 text-body font-medium text-primary">
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
      class="group flex flex-col gap-4 rounded-lg border border-border bg-background-subtle p-7 transition-colors hover:border-primary"
    >
      <div class="flex size-9 items-center justify-center rounded-md bg-foreground text-background">
        <Icon name="x-brand" size={15} />
      </div>
      <div class="flex flex-col gap-1.5">
        <div class="text-body-l font-semibold">{copy.xTitle}</div>
        <p class="text-body leading-[1.6] text-foreground-secondary">{copy.xBody}</p>
      </div>
      <div class="mt-auto flex items-center gap-2 text-body font-medium text-primary">
        {data.owner.xHandle}
        <Icon
          name="arrow-right"
          size={15}
          class="transition-transform group-hover:translate-x-0.5"
        />
      </div>
    </a>
  </div>

  <div class="flex w-full flex-col gap-[3px] rounded-md border border-border bg-background-subtle px-[30px] py-5 text-center">
    <span class="text-caption text-muted">{copy.replyLabel}</span>
    <span class="text-body text-foreground">{copy.replyValue}</span>
  </div>
</Band>
