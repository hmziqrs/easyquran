<script lang="ts">
  import {
    BG_COLORS,
    CURRENT_MIX,
    MIX_AXES,
    RECOMMENDED_MIX,
    axisSearch,
    mixSearch,
    translationsSearch,
    type BgId,
    type MixAxisKey,
    type MixState,
  } from "./axes";
  import type { MixTranslation } from "./types";

  let {
    mix,
    params,
    translations,
    missing,
  }: {
    mix: MixState;
    /** The live query, tweaks included (replaceState leaves page.url behind). */
    params: URLSearchParams;
    translations: readonly MixTranslation[];
    missing: readonly string[];
  } = $props();

  const SAMPLE_SETS: readonly { label: string; ids: readonly string[] }[] = [
    { label: "English + Urdu", ids: ["en.sahih", "ur.jalandhry"] },
    { label: "English + Urdu + Indonesian", ids: ["en.sahih", "ur.jalandhry", "id.indonesian"] },
    { label: "Two English + Urdu", ids: ["en.sahih", "en.pickthall", "ur.jalandhry"] },
    { label: "English only", ids: ["en.sahih"] },
  ];

  const shownIds = $derived(translations.map((t) => t.id).join(","));
  const isCurrent = $derived(MIX_AXES.every((axis) => mix[axis.key] === CURRENT_MIX[axis.key]));
  const isRecommended = $derived(
    MIX_AXES.every((axis) => mix[axis.key] === RECOMMENDED_MIX[axis.key]),
  );

  function noteFor(key: MixAxisKey): string {
    const axis = MIX_AXES.find((a) => a.key === key);
    return axis?.options.find((o) => o.id === mix[key])?.note ?? "";
  }

  function isBg(id: string): id is BgId {
    return id in BG_COLORS;
  }

  function swatch(id: string): string {
    if (!isBg(id)) return "";
    const c = BG_COLORS[id];
    return `linear-gradient(90deg, ${c.page} 0 50%, ${c.reader} 50% 100%)`;
  }

  const chip =
    "inline-flex h-8 items-center gap-1.5 rounded-md border px-2.5 text-[13px] transition-colors";
  const chipOn = "border-primary bg-primary/15 text-foreground";
  const chipOff = "border-border text-foreground-secondary hover:border-primary hover:text-foreground";
</script>

<section class="border-b border-border px-5 py-6 sm:px-7 lg:px-10">
  <div class="mx-auto flex w-full max-w-[1180px] flex-col gap-5">
    <div class="flex flex-wrap items-end justify-between gap-4">
      <div class="flex max-w-[64ch] flex-col gap-1.5">
        <h1 class="text-[24px] font-semibold tracking-[-0.02em] text-foreground">Reader mix</h1>
        <p class="text-[14.5px] leading-[1.6] text-foreground-secondary">
          One pick per row. Every combination — sliders included — is its own URL, so copy the
          address bar to keep a mix. Option A is always today's reader.
        </p>
      </div>
      <div class="flex gap-2">
        <!-- kit 3 maps the old scroll-only `noscroll` to data-sveltekit-reset="false", which also preserves focus — accepted drift on this internal design route. -->
        <a
          href={mixSearch(params, RECOMMENDED_MIX)}
          data-sveltekit-reset="false"
          aria-current={isRecommended ? "true" : undefined}
          class={[chip, isRecommended ? chipOn : chipOff]}
        >
          Your pick
        </a>
        <a
          href={mixSearch(params, CURRENT_MIX)}
          data-sveltekit-reset="false"
          aria-current={isCurrent ? "true" : undefined}
          class={[chip, isCurrent ? chipOn : chipOff]}
        >
          All current
        </a>
      </div>
    </div>

    <div class="flex flex-col gap-3">
      {#each MIX_AXES as axis (axis.key)}
        <div class="grid items-start gap-x-4 gap-y-1.5 md:grid-cols-[11rem_1fr]">
          <span class="pt-1.5 text-[13.5px] font-semibold text-foreground">{axis.label}</span>
          <div class="flex flex-col gap-1.5">
            <div class="flex flex-wrap gap-1.5">
              {#each axis.options as option (option.id)}
                {@const on = mix[axis.key] === option.id}
                <a
                  href={axisSearch(params, mix, axis.key, option.id)}
                  data-sveltekit-reset="false"
                  aria-current={on ? "true" : undefined}
                  title={option.note}
                  class={[chip, on ? chipOn : chipOff]}
                >
                  {#if axis.key === "bg"}
                    <span
                      class="size-4 flex-none rounded-sm border border-white/15"
                      style:background={swatch(option.id)}
                      aria-hidden="true"
                    ></span>
                  {:else}
                    <span class="font-mono text-[11.5px] uppercase">{option.id}</span>
                  {/if}
                  {option.name}
                </a>
              {/each}
            </div>
            <p class="text-[13px] text-foreground-secondary">
              {noteFor(axis.key)}
              {#if axis.key === "bg"}
                Dark mode only; swatches show page | reader. Custom colours live in the Tweak panel.
              {/if}
            </p>
          </div>
        </div>
      {/each}
    </div>

    <!-- Not a design option: which translations the demo loads, to test 1, 2 or 3 stacked. -->
    <div class="flex flex-col gap-2 rounded-lg border border-dashed border-border p-4">
      <span class="text-[13.5px] font-semibold text-foreground">Sample text (not a design choice)</span>
      <p class="text-[13px] text-foreground-secondary">
        Which translations this demo page loads under the Arabic, so you can see each layout with
        one, two or three stacked. In the app this is whatever the reader picked in the
        Translations popup.
      </p>
      <div class="flex flex-wrap gap-1.5">
        {#each SAMPLE_SETS as set (set.label)}
          {@const on = set.ids.join(",") === shownIds}
          <a
            href={translationsSearch(params, mix, set.ids)}
            data-sveltekit-reset="false"
            aria-current={on ? "true" : undefined}
            class={[chip, on ? chipOn : chipOff]}
          >
            {set.label}
          </a>
        {/each}
      </div>
      {#if missing.length > 0}
        <p class="text-[13px] text-warning">No local copy of {missing.join(", ")}.</p>
      {/if}
    </div>
  </div>
</section>
