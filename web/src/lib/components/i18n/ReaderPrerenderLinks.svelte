<script lang="ts">
  import { building } from "$app/env";
  import { publicHref, type PublicHref } from "#lib/i18n/public-href.js";

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
