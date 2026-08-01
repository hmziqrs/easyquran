<script lang="ts">
  import { variantDef } from "../../_variants/registry";
  import LandingA from "../../_variants/landing/LandingA.svelte";
  import LandingB from "../../_variants/landing/LandingB.svelte";
  import LandingC from "../../_variants/landing/LandingC.svelte";

  let { data } = $props();
  const surah = $derived(data.surah);
  const def = $derived(variantDef("landing", data.variant));
</script>

<svelte:head>
  <title>Landing {data.variant.toUpperCase()} · {def?.name} — EasyQuran design</title>
</svelte:head>

{#if data.variant === "a"}
  <LandingA verses={surah.verses} arabicName={surah.arabic} />
{:else if data.variant === "b"}
  <LandingB verses={surah.verses} arabicName={surah.arabic} name={surah.name} />
{:else}
  <LandingC
    verses={surah.verses}
    arabicName={surah.arabic}
    name={surah.name}
    ayahCount={surah.ayahCount}
  />
{/if}

{#if def}
  <footer class="border-t border-line bg-bg-elev px-5 py-8">
    <div class="mx-auto flex w-full max-w-[860px] flex-col gap-2">
      <span class="font-mono text-[11px] uppercase tracking-[0.12em] text-fg-4">
        Variant {def.id.toUpperCase()} · {def.name}
      </span>
      <p class="text-[14.5px] leading-[1.6] text-fg-2">{def.pitch}</p>
      <p class="text-[13.5px] leading-[1.55] text-fg-4">
        <span class="font-medium text-fg-3">Costs you:</span>
        {def.tradeoff}
      </p>
    </div>
  </footer>
{/if}
