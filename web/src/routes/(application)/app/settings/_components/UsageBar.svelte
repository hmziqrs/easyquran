<script lang="ts">
  import { layoutUsageSegments, type StorageLayer } from "$lib/stores/storage-report.svelte";
  import { formatBytes } from "$lib/utils";

  const TRACK_UNITS = 400;
  const TRACK_HEIGHT = 14;

  const LAYER_FILL = {
    arabic: "var(--accent)",
    translations: "var(--pop)",
    pack: "var(--accent-line)",
    pages: "var(--bg-3)",
    data: "var(--pop-soft)",
    other: "var(--fg-4)",
  } satisfies Record<StorageLayer["id"], string>;

  let {
    layers,
    labels,
    quotaBytes,
    usedLabel,
  }: {
    layers: readonly StorageLayer[];
    labels: Readonly<Record<StorageLayer["id"], string>>;
    quotaBytes: number | null;
    usedLabel: string;
  } = $props();

  const widths = $derived(
    layoutUsageSegments(
      TRACK_UNITS,
      layers,
      quotaBytes ?? 0,
    ),
  );
  const segments = $derived.by(() => {
    let acc = 0;
    return layers.map((layer, i) => {
      const width = widths[i] ?? 0;
      const segment = { layer, width, offset: acc };
      acc += width;
      return segment;
    });
  });
  const composedTitle = $derived(usedLabel);
  const hasBar = $derived(quotaBytes !== null && quotaBytes > 0);

  function percent(bytes: number): string {
    if (quotaBytes === null || quotaBytes <= 0) return "—";
    return `${Math.round((bytes / quotaBytes) * 100)}%`;
  }

  function segmentTitle(layer: StorageLayer): string {
    return `${labels[layer.id]} · ${formatBytes(layer.bytes)} · ${percent(layer.bytes)}`;
  }
</script>

{#if hasBar}
  <svg
    viewBox="0 0 {TRACK_UNITS} {TRACK_HEIGHT}"
    preserveAspectRatio="none"
    role="img"
    aria-label={composedTitle}
    class="block h-3.5 w-full overflow-hidden rounded-full border border-line bg-bg-2"
  >
    <title>{composedTitle}</title>
    <defs>
      <pattern
        id="usage-other-stripes"
        width="6"
        height="6"
        patternUnits="userSpaceOnUse"
        patternTransform="rotate(45)"
      >
        <rect width="6" height="6" fill="var(--bg-2)"></rect>
        <rect width="3" height="6" fill="var(--fg-4)"></rect>
      </pattern>
    </defs>
    {#each segments as segment (segment.layer.id)}
      {#if segment.width > 0}
        <rect
          x={segment.offset}
          y="0"
          width={segment.width}
          height={TRACK_HEIGHT}
          fill={segment.layer.id === "other" ? "url(#usage-other-stripes)" : LAYER_FILL[segment.layer.id]}
        >
          <title>{segmentTitle(segment.layer)}</title>
        </rect>
      {/if}
    {/each}
  </svg>
{/if}

<ul class="mt-2 grid gap-1">
  {#each layers as layer (layer.id)}
    <li class="flex items-center gap-2 text-xs text-fg-3">
      <span
        class="inline-block size-2 shrink-0 rounded-sm border border-line-2"
        style={`background:${
          layer.id === "other" ? "repeating-linear-gradient(45deg, var(--fg-4) 0 2px, var(--bg-2) 2px 4px)" : LAYER_FILL[layer.id]
        }`}
      ></span>
      <span class="min-w-0 flex-1 truncate">{labels[layer.id]}</span>
      <span class="tabular-nums text-fg-2">{formatBytes(layer.bytes)}</span>
      <span class="w-10 text-end tabular-nums text-fg-4">{percent(layer.bytes)}</span>
    </li>
  {/each}
</ul>
