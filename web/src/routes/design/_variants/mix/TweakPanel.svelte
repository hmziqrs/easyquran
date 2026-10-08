<script lang="ts">
  import { Icon } from "#lib/components/icon/index.js";
  import {
    DEFAULT_TWEAKS,
    NUMBER_TWEAKS,
    TWEAK_GROUPS,
    withNumber,
    type LabelNames,
    type LabelPlacement,
    type OnOff,
    type Tweaks,
  } from "./tweaks";

  let {
    tweaks,
    onChange,
  }: {
    tweaks: Tweaks;
    onChange: (next: Tweaks) => void;
  } = $props();

  let open = $state(false);
  let copied = $state(false);

  const LABELS: readonly { id: LabelPlacement; name: string }[] = [
    { id: "inline", name: "Inline" },
    { id: "above", name: "Above" },
    { id: "key", name: "Key only" },
  ];
  const NAMES: readonly { id: LabelNames; name: string }[] = [
    { id: "short", name: "Short" },
    { id: "full", name: "Full" },
  ];
  const COLOR_FIELDS: readonly { key: "page" | "reader"; name: string }[] = [
    { key: "page", name: "Page" },
    { key: "reader", name: "Reader" },
  ];
  const ON_OFF: readonly { id: OnOff; name: string }[] = [
    { id: "on", name: "On" },
    { id: "off", name: "Off" },
  ];

  function onNumber(key: (typeof NUMBER_TWEAKS)[number]["key"], event: Event): void {
    // SAFETY: bound only to this panel's <input type="range">; currentTarget is that input.
    const value = Number((event.currentTarget as HTMLInputElement).value);
    if (Number.isFinite(value)) onChange(withNumber(tweaks, key, value));
  }

  function onColor(key: "page" | "reader", event: Event): void {
    // SAFETY: bound only to this panel's <input type="color">; currentTarget is that input.
    const value = (event.currentTarget as HTMLInputElement).value;
    onChange({ ...tweaks, [key]: value });
  }

  async function copyLink(): Promise<void> {
    try {
      await navigator.clipboard.writeText(window.location.href);
      copied = true;
      setTimeout(() => (copied = false), 1600);
    } catch {
      copied = false;
    }
  }

  const seg = "flex h-7 flex-1 items-center justify-center rounded-sm text-[12.5px] font-medium transition-colors";
  const segOn = "bg-foreground text-background";
  const segOff = "text-foreground-secondary hover:text-foreground";
</script>

{#snippet segmented(label: string, options: readonly { id: string; name: string }[], value: string, pick: (id: string) => void)}
  <div class="flex flex-col gap-1.5">
    <span class="text-[12.5px] text-foreground">{label}</span>
    <div class="flex gap-0.5 rounded-md border border-(--mix-line) p-0.5" role="group" aria-label={label}>
      {#each options as option (option.id)}
        <button type="button" aria-pressed={value === option.id} onclick={() => pick(option.id)} class={[seg, value === option.id ? segOn : segOff]}>
          {option.name}
        </button>
      {/each}
    </div>
  </div>
{/snippet}

<button
  type="button"
  aria-expanded={open}
  onclick={() => (open = !open)}
  class="fixed bottom-[5.5rem] right-5 z-[65] flex h-10 items-center gap-2 rounded-pill border border-(--mix-line) bg-(--mix-page) px-4 text-[13.5px] font-medium text-foreground transition-colors hover:border-primary aria-expanded:border-primary"
>
  <Icon name="swatches" size={15} />
  Tweak spacing
</button>

{#if open}
  <div
    role="dialog"
    aria-label="Tweak spacing and type"
    class="fixed bottom-[8.75rem] right-5 top-20 z-[65] flex w-[min(320px,calc(100vw-2.5rem))] flex-col rounded-lg border border-(--mix-line) bg-(--mix-page)"
  >
    <div class="flex items-center justify-between gap-2 border-b border-(--mix-line) px-4 py-3">
      <span class="text-[14px] font-semibold text-foreground">Tweak spacing</span>
      <div class="flex items-center gap-1">
        <button
          type="button"
          onclick={() => onChange(DEFAULT_TWEAKS)}
          class="h-8 rounded-md px-2.5 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:text-foreground"
        >
          Reset
        </button>
        <button
          type="button"
          onclick={() => void copyLink()}
          class="h-8 rounded-md border border-(--mix-line) px-2.5 text-[12.5px] font-medium text-foreground transition-colors hover:border-primary"
        >
          {copied ? "Copied" : "Copy link"}
        </button>
      </div>
    </div>

    <div class="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto overscroll-contain px-4 py-4">
      <p class="text-[12.5px] leading-[1.5] text-foreground-secondary">
        Applies to stack options B–E. Each piece of an ayah has its own spacing, so tune them one
        at a time; the URL keeps every value.
      </p>

      {#each TWEAK_GROUPS as group (group)}
        <section class="flex flex-col gap-3">
          <h3 class="text-[12px] font-semibold uppercase tracking-[0.08em] text-foreground-secondary">{group}</h3>
          {#each NUMBER_TWEAKS.filter((def) => def.group === group) as def (def.key)}
            <label class="flex flex-col gap-1.5">
              <span class="flex items-baseline justify-between text-[12.5px] text-foreground">
                {def.label}
                <span class="font-mono text-[11.5px] tabular-nums text-foreground-secondary">{tweaks[def.key]}{def.unit}</span>
              </span>
              <input
                type="range"
                min={def.min}
                max={def.max}
                step={def.step}
                value={tweaks[def.key]}
                oninput={(event) => onNumber(def.key, event)}
                class="w-full accent-primary"
              />
            </label>
          {/each}

          {#if group === "Ayah row"}
            {@render segmented("Tools line", ON_OFF, tweaks.tools, (id) => onChange({ ...tweaks, tools: id === "off" ? "off" : "on" }))}
            {@render segmented("Rule between ayahs", ON_OFF, tweaks.divider, (id) => onChange({ ...tweaks, divider: id === "off" ? "off" : "on" }))}
          {:else if group === "Translations"}
            {@render segmented("Translator names", NAMES, tweaks.names, (id) => onChange({ ...tweaks, names: id === "full" ? "full" : "short" }))}
          {:else if group === "Lanes"}
            {@render segmented("Name placement", LABELS, tweaks.label, (id) => {
              const hit = LABELS.find((option) => option.id === id);
              if (hit) onChange({ ...tweaks, label: hit.id });
            })}
          {/if}
        </section>
      {/each}

      <section class="flex flex-col gap-3">
        <h3 class="text-[12px] font-semibold uppercase tracking-[0.08em] text-foreground-secondary">Custom background</h3>
        <p class="text-[12.5px] leading-[1.5] text-foreground-secondary">
          Overrides the Background preset in dark mode. Clear to go back to the preset.
        </p>
        {#each COLOR_FIELDS as field (field.key)}
          <div class="flex items-center gap-3">
            <label class="flex flex-1 items-center gap-3 text-[12.5px] text-foreground">
              <input
                type="color"
                value={tweaks[field.key] === "" ? "#141414" : tweaks[field.key]}
                oninput={(event) => onColor(field.key, event)}
                class="h-8 w-10 cursor-pointer rounded-sm border border-(--mix-line) bg-transparent"
              />
              {field.name}
              <span class="font-mono text-[11.5px] text-foreground-secondary">{tweaks[field.key] || "preset"}</span>
            </label>
            {#if tweaks[field.key] !== ""}
              <button
                type="button"
                onclick={() => onChange({ ...tweaks, [field.key]: "" })}
                class="h-7 rounded-md px-2 text-[12px] text-foreground-secondary transition-colors hover:text-foreground"
              >
                Clear
              </button>
            {/if}
          </div>
        {/each}
      </section>
    </div>
  </div>
{/if}
