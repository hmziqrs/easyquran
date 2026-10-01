<script lang="ts">
  import { onMount } from "svelte";
  import { loadArabicFont } from "$lib/fonts/arabic-fonts";
  import SystemReader from "../../_variants/system/SystemReader.svelte";
  import { systemById } from "../../_variants/system/systems";

  let { data } = $props();
  const sys = $derived(systemById(data.variant));

  onMount(() => {
    void loadArabicFont("noto-naskh-arabic");
  });
</script>

<svelte:head>
  <title>System {data.variant.toUpperCase()} · {sys?.name} — EasyQuran design</title>
</svelte:head>

{#if sys}
  <section class="border-b border-border px-6 py-8">
    <div class="mx-auto flex max-w-[1200px] flex-col gap-5">
      <div class="flex max-w-[70ch] flex-col gap-2">
        <span class="text-[13px] font-semibold text-foreground-secondary">System {sys.id.toUpperCase()}</span>
        <h1 class="text-[28px] font-semibold tracking-[-0.02em] text-foreground">{sys.name}</h1>
        <p class="text-[15px] leading-[1.6] text-foreground-secondary">{sys.pitch}</p>
      </div>
      <div class="flex flex-wrap gap-x-10 gap-y-4">
        <dl class="grid flex-1 grid-cols-[6rem_1fr] gap-x-4 gap-y-2 text-[14px]" style="min-width: min(100%, 30rem)">
          {#each sys.rules as rule (rule.label)}
            <dt class="font-semibold text-foreground">{rule.label}</dt>
            <dd class="text-foreground-secondary">{rule.value}</dd>
          {/each}
        </dl>
        <div class="flex flex-wrap items-start gap-3">
          {#each sys.swatches as swatch (swatch.name)}
            <div class="flex w-24 flex-col gap-1.5">
              <span class="h-12 rounded-md border border-border" style:background={swatch.value}></span>
              <span class="text-[12.5px] font-semibold text-foreground">{swatch.name}</span>
            </div>
          {/each}
        </div>
      </div>
    </div>
  </section>

  <SystemReader
    {sys}
    surah={data.surah}
    opener={data.opener}
    verses={data.verses}
    translations={data.translations}
  />
{/if}
