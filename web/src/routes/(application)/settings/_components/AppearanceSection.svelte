<script lang="ts">
  import { onMount } from "svelte";
  import { browser } from "$app/env";
  import { prefs } from "#lib/stores/prefs.svelte.js";
  import { APPEARANCE_MODES, PALETTES } from "#lib/config/site.js";
  import type { CustomSeeds } from "#lib/theme/derive.js";
  import { cn } from "#lib/utils.js";
  import type { SettingsCopy } from "#lib/i18n/settings-copy.js";
  import { SUPPORTED_UI_LOCALES, UI_LOCALES, type UiLocale } from "#lib/i18n/locales.js";
  import { publicHref } from "#lib/i18n/public-href.js";

  let {
    id,
    heading,
    copy,
    locale,
  }: { id: string; heading: string; copy: SettingsCopy["appearance"]; locale: UiLocale } = $props();

  /* Interface language lives here and nowhere else (the header carries no switcher): each option
     opens this same section in the other locale's /settings twin. A full reload, because the
     message catalog and document direction are chosen at load. */
  function settingsHrefFor(target: UiLocale): `/${string}` {
    if (target === "en") return `/settings#${id}`;
    return `/${target}/settings#${id}`;
  }
  const languages = $derived(
    SUPPORTED_UI_LOCALES.map((target) => ({
      locale: target,
      label: UI_LOCALES[target].endonym,
      direction: UI_LOCALES[target].direction,
      href: publicHref(settingsHrefFor(target)),
      current: target === locale,
    })),
  );

  const panel = $derived(copy.panel);
  const seeds: { key: keyof CustomSeeds; fallbackVar: string }[] = [
    { key: "bg", fallbackVar: "--background" },
    { key: "accent", fallbackVar: "--primary" },
    { key: "pop", fallbackVar: "--accent" },
  ];

  let copied = $state(false);
  let copyTimer: ReturnType<typeof setTimeout> | null = null;

  const pill = "rounded-pill border px-3.5 py-2 text-caption transition-colors duration-150";
  /* §38: active mode toggle carries the primary fill (hover override keeps it). */
  const pillOn =
    "border-transparent bg-primary font-medium text-primary-foreground hover:bg-primary hover:text-primary-foreground";
  const pillOff = "border-border-strong text-foreground-secondary hover:border-border hover:text-foreground";
  const quiet =
    "rounded-pill border border-border-strong px-3.5 py-2.5 text-caption text-foreground-secondary transition-colors duration-150 hover:border-border hover:text-foreground";

  function pillClass(active: boolean): string {
    return cn(pill, active ? pillOn : pillOff);
  }

  /* Swatch renders the ACCENT, not the ground (plan 00 D1): every palette shares one
     neutral ground, so ground swatches would be four identical squares. */
  function accentSwatch(accent: { light: string; dark: string }): string {
    return prefs.theme === "light" ? accent.light : accent.dark;
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

  onMount(() => () => {
    if (copyTimer) clearTimeout(copyTimer);
  });
</script>

<div id={id} tabindex="-1" class="scroll-mt-24">
  <h2 class="text-[17px] font-medium tracking-[-0.02em] text-foreground">{heading}</h2>
  <p class="mt-1 max-w-[70ch] text-[14.5px] leading-relaxed text-foreground-secondary">{copy.intro}</p>

  <div
    class="mt-4 divide-y divide-border overflow-hidden rounded-xl border border-border-strong bg-surface"
  >
    <div class="flex flex-wrap items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
      <span id="settings-language-label" class="text-[14.5px] font-medium text-foreground"
        >{copy.language}</span
      >
      <div class="flex shrink-0 gap-1.5" role="group" aria-labelledby="settings-language-label">
        {#each languages as option (option.locale)}
          <a
            href={option.href}
            hreflang={option.locale}
            lang={option.locale}
            dir={option.direction}
            aria-current={option.current ? "true" : undefined}
            data-sveltekit-reload
            class={pillClass(option.current)}>{option.label}</a
          >
        {/each}
      </div>
    </div>

    <div class="flex flex-wrap items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
      <span class="text-[14.5px] font-medium text-foreground">{panel.appearanceLabel}</span>
      <div class="flex shrink-0 gap-1.5">
        {#each APPEARANCE_MODES as m (m)}
          <button
            type="button"
            class={pillClass(prefs.mode === m)}
            aria-pressed={prefs.mode === m}
            onclick={() => prefs.setMode(m)}>{panel.appearanceNames[m]}</button
          >
        {/each}
      </div>
    </div>

    <div class="px-4 py-3.5 sm:px-5">
      <span class="text-[14.5px] font-medium text-foreground">{panel.paletteLabel}</span>
      <div class="mt-2.5 grid grid-cols-1 gap-2 sm:grid-cols-2">
        {#each PALETTES as p (p.id)}
          <button
            type="button"
            title={panel.palettes[p.id].note}
            aria-pressed={prefs.palette === p.id}
            onclick={() => prefs.setPalette(p.id)}
            class={cn(
              "flex items-center gap-3 rounded-pill border px-3 py-2.5 text-start transition-colors",
              prefs.palette === p.id
                ? "border-primary bg-primary-soft ring-2 ring-primary ring-offset-2 ring-offset-surface"
                : "border-border-strong bg-surface hover:border-border hover:text-foreground",
            )}
          >
            <span
              class="size-6 flex-none rounded-sm border border-border-strong"
              style={`background:${accentSwatch(p.accentHex)}`}
            ></span>
            <span class="min-w-0">
              <span class="block text-caption text-foreground">{panel.palettes[p.id].label}</span>
              <span class="block truncate text-caption text-muted"
                >{panel.palettes[p.id].note}</span
              >
            </span>
          </button>
        {/each}
      </div>
    </div>

    <div class="px-4 py-3.5 sm:px-5">
      <div class="flex items-center justify-between gap-4">
        <span class="text-[14.5px] font-medium text-foreground">{panel.customColours}</span>
        {#if prefs.hasCustom}
          <button
            type="button"
            class="text-caption text-muted underline underline-offset-2 transition-colors hover:text-foreground"
            onclick={() => prefs.clearCustom()}>{panel.clear}</button
          >
        {/if}
      </div>
      <div class="mt-2.5 flex flex-col gap-2">
        {#each seeds as s (s.key)}
          <div class="flex items-center gap-3">
            <input
              type="color"
              aria-label={panel.colourInputLabel(panel.seedNames[s.key])}
              value={seedValue(s.key, s.fallbackVar)}
              oninput={(e) => prefs.setCustom(s.key, e.currentTarget.value)}
              class="size-8 flex-none cursor-pointer rounded-sm border border-border-strong bg-transparent p-0.5"
            />
            <span class="flex-1 text-caption text-foreground-secondary">{panel.seedNames[s.key]}</span>
            <span class="text-caption tabular-nums text-muted">
              {prefs.custom[s.key] ?? panel.preset}
            </span>
            {#if prefs.custom[s.key]}
              <button
                type="button"
                aria-label={panel.resetToPresetLabel(panel.seedNames[s.key])}
                class="text-muted transition-colors hover:text-foreground"
                onclick={() => prefs.setCustom(s.key, undefined)}>✕</button
              >
            {/if}
          </div>
        {/each}
      </div>
      <p class="mt-2.5 text-caption leading-snug text-muted">
        {panel.derivedColours}
      </p>
    </div>
  </div>

  <div class="mt-4 flex flex-wrap gap-2">
    <button type="button" class={cn(quiet, "flex-1")} onclick={copyCss}>
      {copied ? panel.copied : panel.copyCss}
    </button>
    <button type="button" class={cn(quiet, "flex-1")} onclick={() => prefs.reset()}>
      {panel.reset}
    </button>
    <span class="sr-only" aria-live="polite">{copied ? panel.copied : ""}</span>
  </div>
</div>
