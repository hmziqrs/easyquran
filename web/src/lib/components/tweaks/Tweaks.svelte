<!--
  Tweaks — floating appearance panel: theme and accent. Wired straight to the
  prefs store. The round button toggles it; everything persists to
  localStorage and applies live.
-->
<script lang="ts">
  import { prefs } from "$lib/stores/prefs.svelte";
  import { ACCENTS, type ThemeMode } from "$lib/config/site";
  import { cn } from "$lib/utils";

  let open = $state(false);
  const themes: ThemeMode[] = ["dark", "light"];

  const pill = "rounded-md border px-3 py-1 text-xs capitalize transition-colors duration-150";
  const on = "border-accent bg-accent-soft text-fg";
  const off = "border-line-2 text-fg-2 hover:text-fg";
</script>

<div class="fixed bottom-5 right-5 z-[1000] flex flex-col items-end gap-3">
  {#if open}
    <div
      class="w-[260px] rounded-xl border border-line-2 bg-bg-1/95 p-3.5 shadow-[0_18px_40px_rgba(0,0,0,0.4)] backdrop-blur"
    >
      <div class="mb-3 flex items-center justify-between">
        <span class="font-mono text-xs uppercase tracking-wide text-fg-3">Appearance</span>
        <button
          type="button"
          class="text-fg-3 transition-colors hover:text-fg"
          onclick={() => (open = false)}
          aria-label="Close appearance panel">✕</button
        >
      </div>

      <div class="grid gap-3.5">
        <div>
          <div class="mb-1.5 text-xs text-fg-3">Theme</div>
          <div class="flex gap-1.5">
            {#each themes as t (t)}
              <button
                type="button"
                class={cn(pill, prefs.theme === t ? on : off)}
                onclick={() => prefs.setTheme(t)}>{t}</button
              >
            {/each}
          </div>
        </div>

        <div>
          <div class="mb-1.5 text-xs text-fg-3">Accent</div>
          <div class="flex flex-wrap gap-2">
            {#each ACCENTS as a (a.id)}
              <button
                title={a.label}
                aria-label={`Accent: ${a.label}`}
                class={cn(
                  "size-[30px] rounded-lg border-2 transition-transform",
                  prefs.accent === a.id ? "scale-110 border-fg" : "border-line",
                )}
                style={`background:${a.hex}`}
                onclick={() => prefs.setAccent(a.id)}
                aria-pressed={prefs.accent === a.id}
              ></button>
            {/each}
          </div>
        </div>
      </div>
    </div>
  {/if}

  <button
    type="button"
    onclick={() => (open = !open)}
    aria-label="Customize appearance"
    aria-pressed={open}
    class="flex size-10 items-center justify-center rounded-full border border-line-2 bg-bg-1/95 text-fg-2 shadow-lg backdrop-blur transition-colors hover:text-fg"
  >
    {#if open}<span class="text-sm">✕</span>{:else}<span class="text-lg leading-none">◐</span>{/if}
  </button>
</div>
