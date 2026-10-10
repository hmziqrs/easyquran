<script lang="ts">
  import { onMount } from "svelte";
  import { TURNSTILE_SITE_KEY, loadTurnstile } from "#lib/auth/turnstile.js";

  type Props = {
    label: string;
    ontoken: (token: string | null) => void;
  };

  let { label, ontoken }: Props = $props();

  let container: HTMLDivElement;
  let widgetId: string | null = null;

  onMount(() => {
    let cancelled = false;
    void loadTurnstile()
      .then((api) => {
        if (cancelled) return;
        widgetId = api.render(container, {
          sitekey: TURNSTILE_SITE_KEY,
          theme: "auto",
          callback: (token: string) => ontoken(token),
          "error-callback": () => ontoken(null),
          "expired-callback": () => ontoken(null),
        });
      })
      .catch(() => ontoken(null));
    return () => {
      cancelled = true;
      const id = widgetId;
      widgetId = null;
      if (id && window.turnstile) window.turnstile.remove(id);
    };
  });
</script>

<div class="flex flex-col gap-2">
  <span class="text-xs text-fg-2">{label}</span>
  <div bind:this={container}></div>
</div>
