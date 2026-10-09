<script lang="ts">
  import { page } from "$app/state";
  import { Container, Icon, Seo } from "#lib/components/index.js";
  import { resolveContactCopy } from "#lib/i18n/contact-copy.js";
  import { marketingLocaleFromPath } from "#lib/i18n/marketing-copy.js";
  import { externalLinkAttrs } from "#lib/utils.js";

  let { data } = $props();

  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  const copy = $derived(resolveContactCopy(locale));

  const row =
    "group flex items-center gap-4 px-5 py-4 transition-colors hover:bg-surface-hover focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-focus-ring";
</script>

<Seo
  path="/contact"
  schemaSubtype="ContactPage"
  title={copy.seo.title}
  description={copy.seo.description}
  inLanguage={locale}
/>

<Container class="py-12 md:py-16">
  <div class="flex max-w-[680px] flex-col gap-8">
    <div class="flex flex-col gap-3">
      <h1 class="text-h1">{copy.heading}</h1>
      <p class="text-body-l text-foreground-secondary">{copy.intro}</p>
    </div>

    <ul class="divide-y divide-border overflow-hidden rounded-lg border border-border">
      <li>
        <a href="mailto:{data.owner.email}" class={row}>
          <span
            class="flex size-9 flex-none items-center justify-center rounded-md bg-background-subtle text-foreground"
          >
            <Icon name="mail" size={17} />
          </span>
          <span class="flex min-w-0 flex-col">
            <span class="text-caption text-muted">{copy.emailTitle}</span>
            <span class="truncate text-body font-medium text-foreground">{data.owner.email}</span>
          </span>
          <Icon name="arrow-right" size={15} class="ms-auto flex-none text-muted group-hover:text-foreground" />
        </a>
      </li>
      <li>
        <a href={data.owner.x} {...externalLinkAttrs(data.owner.x, { me: true })} class={row}>
          <span
            class="flex size-9 flex-none items-center justify-center rounded-md bg-background-subtle text-foreground"
          >
            <Icon name="x-brand" size={15} />
          </span>
          <span class="flex min-w-0 flex-col">
            <span class="text-caption text-muted">{copy.xTitle}</span>
            <span class="truncate text-body font-medium text-foreground">{data.owner.xHandle}</span>
          </span>
          <Icon name="arrow-right" size={15} class="ms-auto flex-none text-muted group-hover:text-foreground" />
        </a>
      </li>
    </ul>
  </div>
</Container>
