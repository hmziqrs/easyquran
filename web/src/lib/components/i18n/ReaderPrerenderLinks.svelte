<script lang="ts">
  import { building } from "$app/environment";
  import { publicHref, type PublicHref } from "$lib/i18n/public-href";

  interface Props {
    hrefs: readonly PublicHref[];
    enabled?: boolean;
  }

  let { hrefs, enabled = building }: Props = $props();
</script>

{#if enabled}
  <div hidden data-reader-prerender-links>
    {#each hrefs as href (href)}
      <a href={publicHref(href)}>{href}</a>
    {/each}
  </div>
{/if}
