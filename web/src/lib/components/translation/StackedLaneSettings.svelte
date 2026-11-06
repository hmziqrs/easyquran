<script lang="ts">
  import { TRANSLATION_CATALOGUE_BY_ID } from "#lib/quran/catalogue.js";
  import {
    LANE_STEP_MAX,
    LANE_STEP_MIN,
    stackedTranslations,
    type LaneLayout,
  } from "#lib/stores/stacked-translations.svelte.js";
  import { appLocale } from "#lib/i18n/app-locale.js";
  import {
    reader_lanes_layout_label,
    reader_lanes_layout_parallel,
    reader_lanes_layout_stacked,
    reader_lanes_settings_title,
    reader_lanes_text_larger,
    reader_lanes_text_reset,
    reader_lanes_text_smaller,
  } from "#lib/i18n/m/reader.js";
  import { cn } from "#lib/utils.js";

  /**
   * Display controls for the selected extra translations: side by side or stacked, and a
   * text size per lane. Device-local, and client-only: the same texts are fetched either way.
   */
  const msg = { locale: appLocale() };
  const LAYOUTS: readonly LaneLayout[] = ["stacked", "parallel"];

  function layoutLabel(layout: LaneLayout): string {
    if (layout === "parallel") return reader_lanes_layout_parallel(undefined, msg);
    return reader_lanes_layout_stacked(undefined, msg);
  }

  function laneName(id: string): string {
    return TRANSLATION_CATALOGUE_BY_ID.get(id)?.name ?? id;
  }
</script>

<section data-lane-settings class="flex flex-col gap-3">
  <h3 class="text-[11.5px] font-semibold uppercase tracking-[0.1em] text-muted-foreground">
    {reader_lanes_settings_title(undefined, msg)}
  </h3>

  <div class="flex items-center justify-between gap-3">
    <span id="lane-layout-label" class="text-[13px] font-medium text-foreground">
      {reader_lanes_layout_label(undefined, msg)}
    </span>
    <div
      role="group"
      aria-labelledby="lane-layout-label"
      class="flex flex-none items-center gap-0.5 rounded-pill border border-border p-0.5"
    >
      {#each LAYOUTS as layout (layout)}
        {@const active = stackedTranslations.layout === layout}
        <button
          type="button"
          data-layout={layout}
          aria-pressed={active}
          onclick={() => stackedTranslations.setLayout(layout)}
          class={cn(
            "h-8 cursor-pointer touch-manipulation rounded-pill px-3 text-[13px] transition-colors",
            active ? "bg-primary-soft text-foreground" : "text-foreground-secondary hover:text-foreground",
          )}
        >
          {layoutLabel(layout)}
        </button>
      {/each}
    </div>
  </div>

  <ul class="flex flex-col gap-1.5">
    {#each stackedTranslations.ids as id (id)}
      {@const step = stackedTranslations.laneStep(id)}
      {@const name = laneName(id)}
      <li data-lane-size={id} class="flex items-center justify-between gap-3">
        <span class="min-w-0 truncate text-[13px] text-foreground"><bdi>{name}</bdi></span>
        <div class="flex flex-none items-center gap-0.5">
          <button
            type="button"
            data-lane-smaller
            aria-label={reader_lanes_text_smaller({ name }, msg)}
            disabled={step <= LANE_STEP_MIN}
            onclick={() => stackedTranslations.stepLane(id, -1)}
            class="h-8 w-8 cursor-pointer touch-manipulation rounded-md text-[13px] text-foreground transition-colors hover:bg-surface-hover disabled:cursor-not-allowed disabled:opacity-40"
          >
            A&minus;
          </button>
          <button
            type="button"
            data-lane-reset
            aria-label={reader_lanes_text_reset({ name }, msg)}
            disabled={step === 0}
            onclick={() => stackedTranslations.resetLane(id)}
            class="h-8 cursor-pointer touch-manipulation rounded-md px-2 text-[12px] text-foreground-secondary transition-colors hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40"
          >
            A
          </button>
          <button
            type="button"
            data-lane-larger
            aria-label={reader_lanes_text_larger({ name }, msg)}
            disabled={step >= LANE_STEP_MAX}
            onclick={() => stackedTranslations.stepLane(id, 1)}
            class="h-8 w-8 cursor-pointer touch-manipulation rounded-md text-[15px] text-foreground transition-colors hover:bg-surface-hover disabled:cursor-not-allowed disabled:opacity-40"
          >
            A+
          </button>
        </div>
      </li>
    {/each}
  </ul>
</section>
