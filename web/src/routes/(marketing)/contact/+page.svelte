<!--
  Contact — no form, by design. Two ways to reach us: email and X. Echoes the
  home page's feature-card pattern (icon chip + title + body) and fills the
  same 1180px container frame. The email + X come from the owner profile
  fetched server-side in +layout.server.ts (see $lib/server/owner.ts).
-->
<script lang="ts">
  import { Container, Eyebrow, Icon, Seo } from "$lib/components";
  import { externalLinkAttrs } from "$lib/utils";

  let { data } = $props();
</script>

<Seo path="/contact" schemaSubtype="ContactPage" />

<Container class="flex max-w-[1180px] flex-col gap-12 pt-[72px] pb-24">
  <!-- intro -->
  <div class="flex w-full flex-col gap-3 text-center">
    <Eyebrow class="text-accent">Contact</Eyebrow>
    <h1 class="text-[40px] leading-[1.1] tracking-[-0.03em] sm:text-[46px]">Say salam.</h1>
    <p class="mx-auto max-w-[60ch] text-[18px] leading-[1.65] text-fg-2">
      Bug reports, script corrections, or a feature you keep wishing for &mdash; pick whichever
      channel suits you. We read everything that comes in.
    </p>
  </div>

  <!-- channels -->
  <div class="grid w-full gap-4 md:grid-cols-2">
    <!-- Email -->
    <a
      href="mailto:{data.owner.email}"
      class="group flex flex-col gap-4 rounded-2xl border border-line-2 bg-bg-2 p-7 transition-colors hover:border-line-3"
    >
      <div class="flex size-9 items-center justify-center rounded-[10px] bg-accent-soft text-accent">
        <Icon name="mail" size={17} />
      </div>
      <div class="flex flex-col gap-1.5">
        <div class="text-[18px] font-semibold">Email</div>
        <p class="text-[15px] leading-[1.6] text-fg-2">
          Best for anything detailed &mdash; bug reports, corrections, or a feature idea with context.
        </p>
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

    <!-- X (Twitter) -->
    <a
      href={data.owner.x}
      {...externalLinkAttrs(data.owner.x, { me: true })}
      class="group flex flex-col gap-4 rounded-2xl border border-line-2 bg-bg-2 p-7 transition-colors hover:border-line-3"
    >
      <div class="flex size-9 items-center justify-center rounded-[10px] bg-fg text-bg">
        <Icon name="x-brand" size={15} />
      </div>
      <div class="flex flex-col gap-1.5">
        <div class="text-[18px] font-semibold">X (Twitter)</div>
        <p class="text-[15px] leading-[1.6] text-fg-2">
          Quick questions, updates and support &mdash; send a message or drop a reply.
        </p>
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

  <!-- meta -->
  <div class="flex w-full flex-col gap-[3px] rounded-xl border border-line bg-bg-2 px-[30px] py-5 text-center">
    <span class="text-xs text-fg-3">Typical reply time</span>
    <span class="text-[15px] text-fg">Two or three days</span>
  </div>
</Container>
