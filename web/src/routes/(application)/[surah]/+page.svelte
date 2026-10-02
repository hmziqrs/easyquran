<script lang="ts">
  import SurahPageRoute from "../_reader/SurahPageRoute.svelte";

  let { data } = $props();
</script>

{#key data.pageData.surah.num}
  <SurahPageRoute {data} />
{/key}
