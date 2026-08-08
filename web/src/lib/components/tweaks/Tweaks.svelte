<script lang="ts">
  import { onMount } from "svelte";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { consent } from "$lib/stores/consent.svelte";
  import { ACCENTS, SURFACES, type ThemeMode } from "$lib/config/site";
  import type { CustomSeeds } from "$lib/theme/derive";
  import { Notifications } from "$lib/components/notifications";
  import { OfflinePack } from "$lib/components/status";
  import { cn } from "$lib/utils";

  let open = $state(false);
  let triggerButton = $state<HTMLButtonElement>();
  let firstControl = $state<HTMLButtonElement>();
  let panelEl = $state<HTMLDivElement>();
  let copied = $state(false);
  let copyTimer: ReturnType<typeof setTimeout> | null = null;

  const themes: ThemeMode[] = ["dark", "light"];

  const seeds: { key: keyof CustomSeeds; label: string; fallbackVar: string }[] = [
    { key: "bg", label: "Background", fallbackVar: "--bg" },
    { key: "accent", label: "Accent", fallbackVar: "--accent" },
    { key: "pop", label: "Pop", fallbackVar: "--pop" },
  ];

  const pill = "rounded-md border px-3 py-1 text-xs capitalize transition-colors duration-150";
  const on = "border-accent bg-accent-soft text-fg";
  const off = "border-line-2 text-fg-2 hover:text-fg";

  function pillClass(active: boolean): string {
    return cn(pill, active ? on : off);
  }

  function resolveHex(varName: string): string {
    if (typeof window === "undefined") return "#000000";
    const raw = getComputedStyle(document.documentElement).getPropertyValue(varName).trim();
    if (/^#[0-9a-f]{6}$/i.test(raw)) return raw;
    if (!raw) return "#000000";

    const probe = document.createElement("span");
    probe.style.cssText = `position:absolute;visibility:hidden;color:${raw}`;
    document.body.appendChild(probe);
    const rgb = getComputedStyle(probe).color;
    probe.remove();

    const m = /rgba?\((\d+),\s*(\d+),\s*(\d+)/.exec(rgb);
    if (!m) return "#000000";
    return `#${m
      .slice(1, 4)
      .map((n) => Number(n).toString(16).padStart(2, "0"))
      .join("")}`;
  }

  function seedValue(key: keyof CustomSeeds, fallbackVar: string): string {
    return prefs.custom[key] ?? resolveHex(fallbackVar);
  }

  async function copyCss() {
    try {
      await navigator.clipboard.writeText(prefs.css());
      copied = true;
      if (copyTimer) clearTimeout(copyTimer);
      copyTimer = setTimeout(() => (copied = false), 1600);
    } catch {
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && open) {
      open = false;
      triggerButton?.focus();
    }
  }

  function onPointerDown(event: PointerEvent) {
    if (!open) return;
    const target = event.target as Node | null;
    if (!target) return;
    if (panelEl?.contains(target)) return;
    if (triggerButton?.contains(target)) return;
    open = false;
  }

  $effect(() => {
    if (open && firstControl) {
      firstControl.focus();
    }
  });

  onMount(() => () => {
    if (copyTimer) clearTimeout(copyTimer);
  });
</script>

<svelte:window onkeydown={onKeydown} onpointerdown={onPointerDown} />

<div class="fixed bottom-5 right-5 z-[1000] flex flex-col items-end gap-3">
  {#if open}
    <div
      id="tweaks-panel"
      bind:this={panelEl}
      role="dialog"
      aria-modal="false"
      aria-label="Settings"
      class="max-h-[min(80vh,640px)] w-[288px] overflow-y-auto rounded-xl border border-line-2 bg-bg-1/95 p-3.5 shadow-[0_18px_40px_rgba(0,0,0,0.4)] backdrop-blur"
    >
      <div class="mb-3 flex items-center justify-between">
        <span class="font-mono text-xs uppercase tracking-wide text-fg-3">Theme</span>
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
          <div class="mb-1.5 text-xs text-fg-3">Mode</div>
          <div class="flex gap-1.5">
            {#each themes as t (t)}
              <button
                type="button"
                class={pillClass(prefs.theme === t)}
                onclick={() => prefs.setTheme(t)}>{t}</button
              >
            {/each}
          </div>
        </div>

        <div>
          <div class="mb-1.5 text-xs text-fg-3">Surface</div>
          <div class="flex flex-col gap-1">
            {#each SURFACES as s (s.id)}
              <button
                type="button"
                title={s.note}
                aria-pressed={prefs.surface === s.id}
                onclick={() => prefs.setSurface(s.id)}
                class={cn(
                  "flex items-center gap-2.5 rounded-lg border px-2 py-1.5 text-left transition-colors",
                  prefs.surface === s.id ? "border-accent bg-accent-soft" : "border-line hover:border-line-2",
                )}
              >
                <span
                  class="size-5 flex-none rounded-md border border-line-2"
                  style={`background:${prefs.theme === "light" ? s.lightHex : s.darkHex}`}
                ></span>
                <span class="min-w-0">
                  <span class="block text-xs text-fg">{s.label}</span>
                  <span class="block truncate text-[11px] text-fg-4">{s.note}</span>
                </span>
              </button>
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
                  prefs.accent === a.id && !prefs.custom.accent ? "scale-110 border-fg" : "border-line",
                )}
                style={`background:${a.hex}`}
                onclick={() => prefs.setAccent(a.id)}
                aria-pressed={prefs.accent === a.id && !prefs.custom.accent}
              ></button>
            {/each}
          </div>
        </div>

        <div>
          <div class="mb-1.5 flex items-center justify-between">
            <span class="text-xs text-fg-3">Custom colours</span>
            {#if prefs.hasCustom}
              <button
                type="button"
                class="text-[11px] text-fg-3 underline underline-offset-2 transition-colors hover:text-fg"
                onclick={() => prefs.clearCustom()}>clear</button
              >
            {/if}
          </div>
          <div class="flex flex-col gap-1.5">
            {#each seeds as s (s.key)}
              <div class="flex items-center gap-2">
                <input
                  type="color"
                  aria-label={`${s.label} colour`}
                  value={seedValue(s.key, s.fallbackVar)}
                  oninput={(e) => prefs.setCustom(s.key, e.currentTarget.value)}
                  class="size-7 flex-none cursor-pointer rounded-md border border-line-2 bg-transparent p-0.5"
                />
                <span class="flex-1 text-xs text-fg-2">{s.label}</span>
                <span class="font-mono text-[11px] text-fg-4">
                  {prefs.custom[s.key] ?? "preset"}
                </span>
                {#if prefs.custom[s.key]}
                  <button
                    type="button"
                    aria-label={`Reset ${s.label} to preset`}
                    class="text-fg-4 transition-colors hover:text-fg"
                    onclick={() => prefs.setCustom(s.key, undefined)}>✕</button
                  >
                {/if}
              </div>
            {/each}
          </div>
          <p class="mt-1.5 text-[11px] leading-snug text-fg-4">
            Surfaces, hairlines and text steps are derived from the background;
            the soft washes from the accent.
          </p>
        </div>

        <div class="flex gap-1.5">
          <button type="button" class={cn(pill, off, "flex-1")} onclick={copyCss}>
            {copied ? "Copied" : "Copy CSS"}
          </button>
          <button type="button" class={cn(pill, off, "flex-1")} onclick={() => prefs.reset()}>
            Reset
          </button>
        </div>

        <hr class="border-line" />

        <Notifications />

        <hr class="border-line" />

        <OfflinePack />

        <div>
          <div class="mb-1.5 text-xs text-fg-3">Data &amp; privacy</div>
          <div class="flex flex-col gap-1.5">
            <button
              type="button"
              aria-pressed={consent.analytics}
              onclick={() => consent.setAnalytics(!consent.analytics)}
              class={pillClass(consent.analytics)}
            >
              Analytics {consent.analytics ? "on" : "off"}
            </button>
            <button
              type="button"
              aria-pressed={consent.performance}
              title="Reloads the page to apply — Firebase Performance can only be switched at startup"
              onclick={() => {
                consent.setPerformance(!consent.performance);
                location.reload();
              }}
              class={pillClass(consent.performance)}
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
