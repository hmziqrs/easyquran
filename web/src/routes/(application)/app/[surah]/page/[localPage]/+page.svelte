<script lang="ts">
  import SurahPageRoute from "../../../_reader/SurahPageRoute.svelte";

  let { data } = $props();
</script>

{#key `${data.pageData.surah.num}:${data.pageData.page.localPage}`}
  <SurahPageRoute {data} />
{/key}
