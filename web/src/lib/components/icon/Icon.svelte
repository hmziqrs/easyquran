<script lang="ts">
  import { ICONS, type IconDef, type IconName } from "./icons";
  import { getLocale } from "$lib/paraglide/runtime.js";
  import { uiDirection, type UiLocale } from "$lib/i18n/locales";

  let {
    name,
    size = 16,
    class: className = "",
    title,
  }: { name: IconName; size?: number | string; class?: string; title?: string } = $props();

  const icon = $derived(ICONS[name]);
  const labelled = $derived(title !== undefined);

  /* Directional glyphs mirror under RTL (plan 04 step 5 punt, landed here per plan 06):
     flipping is a property of the icon, not of each call site. Mirroring happens on the
     PATH, inside the viewBox, so a consumer's own rotate-* utility still composes. Locale
     switches go through full navigations (data-sveltekit-reload), so reading it once per
     render is sufficient. */
  const DIRECTIONAL: ReadonlySet<IconName> = new Set(["arrow-right"]);
  // SAFETY: paraglide is compiled for exactly the UI locales (en/ar in messages/), so
  // getLocale() only ever returns a UiLocale at runtime — the same cast the search
  // palette component uses.
  const rtl = uiDirection(getLocale() as UiLocale) === "rtl";

  function mirrorTransform(def: IconDef): string | undefined {
    if (!DIRECTIONAL.has(name) || !rtl) return undefined;
    const width = Number.parseFloat(def.viewBox.split(/\s+/)[2] ?? "0");
    if (!Number.isFinite(width) || width <= 0) return undefined;
    return `scale(-1 1) translate(${-width} 0)`;
  }
</script>

{#if icon}
  <svg
    class={className}
    width={size}
    height={size}
    viewBox={icon.viewBox}
    fill="currentColor"
    role={labelled ? "img" : "presentation"}
    aria-hidden={labelled ? undefined : "true"}
    aria-label={title}
  >
    {#if labelled}<title>{title}</title>{/if}
    <path d={icon.d} transform={mirrorTransform(icon)} />
  </svg>
{/if}
