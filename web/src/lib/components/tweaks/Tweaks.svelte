<script lang="ts">
  import { onMount } from "svelte";
  import { browser } from "$app/environment";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { consent } from "$lib/stores/consent.svelte";
  import { APPEARANCE_MODES, PALETTES, type PaletteId } from "$lib/config/site";
  import type { CustomSeeds } from "$lib/theme/derive";
  import { Notifications } from "$lib/components/notifications";
  import { OfflinePack, OfflinePackBar } from "$lib/components/status";
  import { offline } from "$lib/offline/offline-store.svelte";
  import { cn } from "$lib/utils";
  import { uiDirection, type UiLocale } from "$lib/i18n/locales";
  import type { TweaksResolvedCopy } from "$lib/i18n/marketing-copy";

  let {
    locale,
    triggerLabel,
    loadCopy,
    showReaderTools = true,
  }: {
    /**
     * UI locale of the surrounding chrome. This portal owns its direction so its placement and
     * alignment stay correct even if a parent reader passage has a different content direction.
     */
    locale: UiLocale;
    /** Rendered on the closed trigger, so it is the only appearance string a page eagerly needs. */
    triggerLabel: string;
    /**
     * Resolves the panel copy on first open. Deliberately a loader, not a value: the appearance
     * panel owns ~40 messages that nothing renders until the user asks for it, and eagerly
     * resolving them put every one of those strings in every page's bundle.
     * See docs/quran-system.md (Part 2, Message chunking).
     */
    loadCopy: () => Promise<TweaksResolvedCopy>;
    showReaderTools?: boolean;
  } = $props();

  const direction = $derived(uiDirection(locale));

  let copy = $state<TweaksResolvedCopy>();
  let copyRequest: Promise<void> | undefined;

  /** §25 palette/appearance copy — resolved from the same lazy appearance namespace on first open. */
  interface PalettePanelCopy {
    appearanceLabel: string;
    paletteLabel: string;
    systemLabel: string;
    palettes: Record<PaletteId, { label: string; note: string }>;
  }

  let paletteCopy = $state<PalettePanelCopy>();
  let paletteRequest: Promise<void> | undefined;

  function ensureCopy(): void {
    copyRequest ??= loadCopy().then((resolved) => {
      copy = resolved;
    });
    paletteRequest ??= (async () => {
      const appearance = await import("$lib/i18n/m/appearance");
      const theme = await import("$lib/i18n/m/theme");
      const options = { locale } as const;
      paletteCopy = {
        appearanceLabel: appearance.tweaks_appearance(undefined, options),
        paletteLabel: appearance.tweaks_palette(undefined, options),
        systemLabel: theme.theme_system(undefined, options),
        palettes: {
          sacred: {
            label: appearance.tweaks_palette_sacred(undefined, options),
            note: appearance.tweaks_palette_sacred_note(undefined, options),
          },
          ink: {
            label: appearance.tweaks_palette_ink(undefined, options),
            note: appearance.tweaks_palette_ink_note(undefined, options),
          },
          sepia: {
            label: appearance.tweaks_palette_sepia(undefined, options),
            note: appearance.tweaks_palette_sepia_note(undefined, options),
          },
          sapphire: {
            label: appearance.tweaks_palette_sapphire(undefined, options),
            note: appearance.tweaks_palette_sapphire_note(undefined, options),
          },
        },
      };
    })();
  }

  async function toggle(): Promise<void> {
    if (open) {
      open = false;
      return;
    }
    ensureCopy();
    await copyRequest;
    await paletteRequest;
    open = true;
  }

  let open = $state(false);
  let triggerButton = $state<HTMLButtonElement>();
  let firstControl = $state<HTMLButtonElement>();
  let panelEl = $state<HTMLDivElement>();
  let copied = $state(false);
  let copyTimer: ReturnType<typeof setTimeout> | null = null;

  function modeLabel(mode: string): string {
    if (mode === "system") return paletteCopy?.systemLabel ?? "System";
    if (mode === "dark") return copy?.themeNames.dark ?? "Dark";
    return copy?.themeNames.light ?? "Light";
  }

  const seeds: { key: keyof CustomSeeds; fallbackVar: string }[] = [
    { key: "bg", fallbackVar: "--background" },
    { key: "accent", fallbackVar: "--primary" },
    { key: "pop", fallbackVar: "--accent" },
  ];

  const pill = "rounded-md border px-3 py-1 text-xs transition-colors duration-150";
  const on = "border-primary bg-primary-soft text-foreground";
  const off = "border-border-strong text-foreground-secondary hover:text-foreground";

  function pillClass(active: boolean): string {
    return cn(pill, active ? on : off);
  }

  function swatchHex(lightHex: string, darkHex: string): string {
    return prefs.theme === "light" ? lightHex : darkHex;
  }

  function resolveHex(varName: string): string {
    if (!browser) return "#000000";
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
    // SAFETY: pointer events always dispatch with an EventTarget that is a DOM Node
    // (element or text node); the null branch below still guards the typed null.
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

  $effect(() => {
    if (
      showReaderTools &&
      (offline.status === "downloading" || offline.status === "staging")
    ) {
      ensureCopy();
    }
  });

  onMount(() => () => {
    if (copyTimer) clearTimeout(copyTimer);
  });
</script>

<svelte:window onkeydown={onKeydown} onpointerdown={onPointerDown} />

<div
  lang={locale}
  dir={direction}
  class="fixed end-5 bottom-5 z-[1000] flex flex-col items-end gap-3"
>
  {#if open && copy && paletteCopy}
    <div
      id="tweaks-panel"
      bind:this={panelEl}
      role="dialog"
      aria-modal="false"
      aria-label={copy.settings}
      class="flex max-h-[min(80vh,640px)] w-[288px] flex-col overflow-hidden rounded-xl border border-border-strong bg-surface/95 shadow-[0_18px_40px_rgba(0,0,0,0.4)] backdrop-blur"
    >
      <div class="overflow-y-auto overflow-x-hidden p-3.5">
      <div class="mb-3 flex items-center justify-between gap-2">
        <span class="font-mono text-xs uppercase tracking-wide text-muted">{copy.settings}</span>
        <button
          type="button"
          class="text-muted transition-colors hover:text-foreground"
          onclick={() => (open = false)}
          bind:this={firstControl}
          aria-label={copy.closePanel}>✕</button
        >
      </div>

      <div class="grid grid-cols-1 gap-3.5">
        <div>
          <div class="mb-1.5 text-xs text-muted">{paletteCopy.appearanceLabel}</div>
          <div class="flex gap-1.5">
            {#each APPEARANCE_MODES as m (m)}
              <button
                type="button"
                class={pillClass(prefs.mode === m)}
                aria-pressed={prefs.mode === m}
                onclick={() => prefs.setMode(m)}>{modeLabel(m)}</button
              >
            {/each}
          </div>
        </div>

        <div>
          <div class="mb-1.5 text-xs text-muted">{paletteCopy.paletteLabel}</div>
          <div class="flex flex-col gap-1">
            {#each PALETTES as p (p.id)}
              <button
                type="button"
                title={paletteCopy.palettes[p.id].note}
                aria-pressed={prefs.palette === p.id}
                onclick={() => prefs.setPalette(p.id)}
                class={cn(
                  "flex items-center gap-2.5 rounded-lg border px-2 py-1.5 text-start transition-colors",
                  prefs.palette === p.id
                    ? "border-primary bg-primary-soft"
                    : "border-border hover:border-border-strong",
                )}
              >
                <span
                  class="size-5 flex-none rounded-md border border-border-strong"
                  style={`background:${swatchHex(p.lightHex, p.darkHex)}`}
                ></span>
                <span class="min-w-0">
                  <span class="block text-xs text-foreground">{paletteCopy.palettes[p.id].label}</span>
                  <span class="block truncate text-[11px] text-muted"
                    >{paletteCopy.palettes[p.id].note}</span
                  >
                </span>
              </button>
            {/each}
          </div>
        </div>

        <div>
          <div class="mb-1.5 flex items-center justify-between">
            <span class="text-xs text-muted">{copy.customColours}</span>
            {#if prefs.hasCustom}
              <button
                type="button"
                class="text-[11px] text-muted underline underline-offset-2 transition-colors hover:text-foreground"
                onclick={() => prefs.clearCustom()}>{copy.clear}</button
              >
            {/if}
          </div>
          <div class="flex flex-col gap-1.5">
            {#each seeds as s (s.key)}
              <div class="flex items-center gap-2">
                <input
                  type="color"
                  aria-label={copy.colourInputLabel(copy.seedNames[s.key])}
                  value={seedValue(s.key, s.fallbackVar)}
                  oninput={(e) => prefs.setCustom(s.key, e.currentTarget.value)}
                  class="size-7 flex-none cursor-pointer rounded-md border border-border-strong bg-transparent p-0.5"
                />
                <span class="flex-1 text-xs text-foreground-secondary">{copy.seedNames[s.key]}</span>
                <span class="font-mono text-[11px] text-muted">
                  {prefs.custom[s.key] ?? copy.preset}
                </span>
                {#if prefs.custom[s.key]}
                  <button
                    type="button"
                    aria-label={copy.resetToPresetLabel(copy.seedNames[s.key])}
                    class="text-muted transition-colors hover:text-foreground"
                    onclick={() => prefs.setCustom(s.key, undefined)}>✕</button
                  >
                {/if}
              </div>
            {/each}
          </div>
          <p class="mt-1.5 text-[11px] leading-snug text-muted">
            {copy.derivedColours}
          </p>
        </div>

        <div class="flex gap-1.5">
          <button type="button" class={cn(pill, off, "flex-1")} onclick={copyCss}>
            {copied ? copy.copied : copy.copyCss}
          </button>
          <button type="button" class={cn(pill, off, "flex-1")} onclick={() => prefs.reset()}>
            {copy.reset}
          </button>
        </div>

        {#if showReaderTools}
          <hr class="border-border" />
          {#if copy.notifications}
            <Notifications copy={copy.notifications} />
          {/if}
          <hr class="border-border" />
          {#if copy.offlinePack}
            <OfflinePack copy={copy.offlinePack} />
          {/if}
        {/if}

        <div>
          <div class="mb-1.5 text-xs text-muted">{copy.dataPrivacy}</div>
          <div class="flex flex-col gap-1.5">
            <button
              type="button"
              aria-pressed={consent.analytics}
              onclick={() => consent.setAnalytics(!consent.analytics)}
              class={pillClass(consent.analytics)}
            >
              {copy.toggleStatusLabel(
                copy.analytics,
                consent.analytics ? copy.on : copy.off,
              )}
            </button>
            <button
              type="button"
              aria-pressed={consent.performance}
              title={copy.performanceReload}
              onclick={() => {
                consent.setPerformance(!consent.performance);
                location.reload();
              }}
              class={pillClass(consent.performance)}
            >
              {copy.toggleStatusLabel(
                copy.performance,
                consent.performance ? copy.on : copy.off,
              )}
            </button>
          </div>
        </div>
      </div>
      </div>
    </div>
  {/if}

  {#if showReaderTools && copy?.offlinePack}
    <OfflinePackBar copy={copy.offlinePack} />
  {/if}

  <button
    type="button"
    bind:this={triggerButton}
    onclick={toggle}
    aria-label={triggerLabel}
    aria-expanded={open}
    aria-controls="tweaks-panel"
    class="flex size-10 items-center justify-center rounded-full border border-border-strong bg-surface/95 text-foreground-secondary shadow-lg backdrop-blur transition-colors hover:text-foreground"
  >
    {#if open}<span class="text-sm">✕</span>{:else}<span class="text-lg leading-none">◐</span>{/if}
  </button>
</div>
