<!--
  Tweaks — floating settings panel: appearance (theme, accent), notifications,
  and data & privacy. Wired straight to the prefs/consent/notifications stores.
  The round button toggles it; everything persists to localStorage and applies
  live.
-->
<script lang="ts">
  import { prefs } from "$lib/stores/prefs.svelte";
  import { consent } from "$lib/stores/consent.svelte";
  import { ACCENTS, type ThemeMode } from "$lib/config/site";
  import { Notifications } from "$lib/components/notifications";
  import { cn } from "$lib/utils";

  let open = $state(false);
  let triggerButton = $state<HTMLButtonElement>();
  let firstControl = $state<HTMLButtonElement>();
  const themes: ThemeMode[] = ["dark", "light"];

  const pill = "rounded-md border px-3 py-1 text-xs capitalize transition-colors duration-150";
  const on = "border-accent bg-accent-soft text-fg";
  const off = "border-line-2 text-fg-2 hover:text-fg";

  // A privacy toggle reuses the accent pill: ON = data shared, OFF = opted out.
  function consentPill(value: boolean): string {
    return cn(pill, value ? on : off);
  }

  // Escape closes the panel and restores focus to the trigger.
  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && open) {
      open = false;
      triggerButton?.focus();
    }
  }

  // When the panel opens, move focus to its first control.
  $effect(() => {
    if (open && firstControl) {
      firstControl.focus();
    }
  });
</script>

<svelte:window onkeydown={onKeydown} />

<div class="fixed bottom-5 right-5 z-[1000] flex flex-col items-end gap-3">
  {#if open}
    <div
      id="tweaks-panel"
      role="group"
      aria-label="Settings"
      class="w-[260px] rounded-xl border border-line-2 bg-bg-1/95 p-3.5 shadow-[0_18px_40px_rgba(0,0,0,0.4)] backdrop-blur"
    >
      <div class="mb-3 flex items-center justify-between">
        <span class="font-mono text-xs uppercase tracking-wide text-fg-3">Settings</span>
        <button
          type="button"
          class="text-fg-3 transition-colors hover:text-fg"
          onclick={() => (open = false)}
          bind:this={firstControl}
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

        <Notifications />

        <div>
          <div class="mb-1.5 text-xs text-fg-3">Data &amp; privacy</div>
          <div class="flex flex-col gap-1.5">
            <button
              type="button"
              aria-pressed={consent.analytics}
              onclick={() => consent.setAnalytics(!consent.analytics)}
              class={consentPill(consent.analytics)}
            >
              Analytics {consent.analytics ? "on" : "off"}
            </button>
            <button
              type="button"
              aria-pressed={consent.performance}
              title="Reloads the page to apply — Firebase Performance can only be switched at startup"
              onclick={() => {
                consent.setPerformance(!consent.performance);
                // The Performance SDK can't be toggled after init, so reload to
                // re-run initPerformance() with the new consent flag.
                location.reload();
              }}
              class={consentPill(consent.performance)}
            >
              Performance {consent.performance ? "on" : "off"}
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <button
    type="button"
    bind:this={triggerButton}
    onclick={() => (open = !open)}
    aria-label="Customize appearance"
    aria-expanded={open}
    aria-controls="tweaks-panel"
    class="flex size-10 items-center justify-center rounded-full border border-line-2 bg-bg-1/95 text-fg-2 shadow-lg backdrop-blur transition-colors hover:text-fg"
  >
    {#if open}<span class="text-sm">✕</span>{:else}<span class="text-lg leading-none">◐</span>{/if}
  </button>
</div>
