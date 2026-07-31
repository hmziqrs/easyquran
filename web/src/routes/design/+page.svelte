<!--
  /design — the index. Lists both variant sets with their pitch and, more
  usefully, their tradeoff: the point of the gallery is to make a decision, and
  a decision needs the cost of each option stated, not just the appeal.
-->
<script lang="ts">
  import { VARIANTS, type VariantKind } from "./_variants/registry";

  const groups: { kind: VariantKind; title: string; blurb: string }[] = [
    {
      kind: "landing",
      title: "Landing page",
      blurb:
        "Three takes on the public front door. Same copy and same claims in each — only the form differs, so the comparison is about form.",
    },
    {
      kind: "reader",
      title: "Reader",
      blurb:
        "Three takes on the reading surface, each rendering the real Uthmani text of Surah Al-Mulk from the same source the shipping reader uses.",
    },
  ];
</script>

<div class="mx-auto w-full max-w-[1100px] px-5 py-14">
  <header class="flex flex-col gap-3 border-b border-line pb-9">
    <span class="eyebrow text-accent">Review surface</span>
    <h1 class="max-w-[20ch] text-[40px] leading-[1.08] tracking-[-0.03em]">
      Pick a direction.
    </h1>
    <p class="max-w-[62ch] text-[17px] leading-[1.6] text-fg-2">
      Every variant is built from the same design tokens as the live site, so the
      theme tweaker in the bottom-right re-skins all of them live. Change the
      surface family and the accent while you compare — a layout that only works
      in one palette is the wrong layout.
    </p>
  </header>

  {#each groups as g (g.kind)}
    <section class="flex flex-col gap-6 border-b border-line py-11 last:border-b-0">
      <div class="flex flex-col gap-2">
        <h2 class="text-[26px] tracking-[-0.02em]">{g.title}</h2>
        <p class="max-w-[62ch] text-[15px] leading-[1.6] text-fg-3">{g.blurb}</p>
      </div>

      <div class="grid gap-4 md:grid-cols-3">
        {#each VARIANTS[g.kind] as v (v.id)}
          <a
            href={`/design/${g.kind}/${v.id}`}
            class="group flex flex-col gap-3 rounded-[14px] border border-line bg-bg-1 p-5 transition-colors hover:border-accent-line"
          >
            <div class="flex items-baseline gap-2">
              <span class="font-mono text-xs text-accent">{v.id.toUpperCase()}</span>
              <span class="text-[17px] font-semibold">{v.name}</span>
            </div>
            <p class="text-[14px] leading-[1.6] text-fg-2">{v.pitch}</p>
            <p class="mt-auto border-t border-line pt-3 text-[13px] leading-[1.55] text-fg-4">
              <span class="font-medium text-fg-3">Costs you:</span>
              {v.tradeoff}
            </p>
          </a>
        {/each}
      </div>
    </section>
  {/each}
</div>
