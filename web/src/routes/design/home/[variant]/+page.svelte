<script lang="ts">
  import "@fontsource-variable/jetbrains-mono/wght.css";
  import "@fontsource-variable/onest/wght.css";

  import HomeControls from "../HomeControls.svelte";
  import HomePreview from "../HomePreview.svelte";
  import { homeVariantDefinition } from "../variants";

  let { data } = $props();
  const preset = $derived(homeVariantDefinition(data.variant));
</script>

<svelte:head>
  <title>{preset.name} homepage | EasyQuran design</title>
  <meta name="description" content={preset.description} />
</svelte:head>

{#key data.variant}
  <HomeControls variant={data.variant}>
    {#snippet children(settings)}
      <HomePreview
        surahs={data.surahs}
        surahCount={data.surahCount}
        juzCount={data.juzCount}
        pageCount={data.pageCount}
        variant={data.variant}
        {...settings}
      />
    {/snippet}
  </HomeControls>
{/key}
