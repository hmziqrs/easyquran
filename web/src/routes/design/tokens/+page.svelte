<script lang="ts">
  /**
   * V1 — token sweep host (docs/plan/README.md (phase specs in git history)). Renders every semantic
   * contract token plus the plan 01 hue set as a labelled swatch with its computed value,
   * so a token missing from one of the eight `[data-palette][data-mode]` blocks shows up
   * as a wrong-coloured / "not defined" square the moment the harness captures it.
   *
   * The harness (scripts/visual/capture.ts) sets `data-palette`/`data-mode` on <html>
   * before the screenshot; the MutationObserver re-reads every computed value when that
   * happens, and CSS `var()` cascades the swatch fills for free.
   */
  interface TokenGroup {
    title: string;
    note?: string;
    tokens: string[];
  }

  const CONTRACT_TOKENS = [
    "--background",
    "--background-subtle",
    "--surface",
    "--surface-raised",
    "--surface-hover",
    "--foreground",
    "--foreground-secondary",
    "--muted",
    "--border",
    "--border-strong",
    "--primary",
    "--primary-hover",
    "--primary-foreground",
    "--primary-soft",
    "--accent",
    "--accent-strong",
    "--accent-soft",
    "--success",
    "--warning",
    "--danger",
    "--focus-ring",
    "--reader-background",
    "--quran-foreground",
    "--translation-foreground",
    "--reader-divider",
  ];

  // Additive hue set (plan 01) — undefined until that plan lands; rendered as "not defined"
  // so the sweep turns green-to-colourful exactly when the tokens appear.
  const HUE_TOKENS = [1, 2, 3, 4].flatMap((n) => [`--hue-${n}`, `--hue-${n}-soft`, `--on-hue-${n}`]);
  const LEGIBLE_TOKENS = [1, 2, 3, 4].map((n) => `--hue-${n}-legible`);

  const GROUPS: TokenGroup[] = [
    {
      title: "Ground",
      tokens: [
        "--background",
        "--background-subtle",
        "--surface",
        "--surface-raised",
        "--surface-hover",
      ],
    },
    { title: "Foreground", tokens: ["--foreground", "--foreground-secondary", "--muted"] },
    { title: "Borders", tokens: ["--border", "--border-strong"] },
    {
      title: "Primary",
      tokens: ["--primary", "--primary-hover", "--primary-foreground", "--primary-soft"],
    },
    { title: "Accent", tokens: ["--accent", "--accent-strong", "--accent-soft"] },
    { title: "Status", tokens: ["--success", "--warning", "--danger", "--focus-ring"] },
    {
      title: "Reader",
      tokens: [
        "--reader-background",
        "--quran-foreground",
        "--translation-foreground",
        "--reader-divider",
      ],
    },
    {
      title: "Hues (plan 01, additive)",
      note: "Fill, soft tint, and the foreground for text on that fill. Undefined until plan 01 lands.",
      tokens: HUE_TOKENS,
    },
    {
      title: "Hue legibility (plan 01, dark-only)",
      note: "L≈0.78 version of each hue for numerals on a dark soft chip; equals --hue-N in light.",
      tokens: LEGIBLE_TOKENS,
    },
  ];

  const PAIR_CHIPS: { label: string; fill: string; fg: string }[] = [
    { label: "--primary-foreground on --primary", fill: "--primary", fg: "--primary-foreground" },
    ...[1, 2, 3, 4].map((n) => ({
      label: `--on-hue-${n} on --hue-${n}`,
      fill: `--hue-${n}`,
      fg: `--on-hue-${n}`,
    })),
  ];

  const ALL_TOKENS = [...CONTRACT_TOKENS, ...HUE_TOKENS, ...LEGIBLE_TOKENS];

  interface TokenValue {
    specified: string;
    resolved: string;
  }

  let palette = $state("(default)");
  let mode = $state("(default)");
  let values = $state<Record<string, TokenValue>>({});
  let probe = $state<HTMLDivElement | null>(null);

  function readValues(): [string, TokenValue][] {
    if (!probe) return [];
    const rootStyle = getComputedStyle(document.documentElement);
    const entries: [string, TokenValue][] = [];
    for (const token of ALL_TOKENS) {
      const specified = rootStyle.getPropertyValue(token).trim();
      if (!specified) {
        entries.push([token, { specified: "", resolved: "" }]);
        continue;
      }
      probe.style.backgroundColor = `var(${token})`;
      entries.push([token, { specified, resolved: getComputedStyle(probe).backgroundColor }]);
    }
    return entries;
  }

  $effect(() => {
    const root = document.documentElement;
    const read = () => {
      palette = root.dataset.palette ?? "(default)";
      mode = root.dataset.mode ?? "(default)";
      values = Object.fromEntries(readValues());
    };
    read();
    const observer = new MutationObserver(read);
    observer.observe(root, { attributes: true, attributeFilter: ["data-palette", "data-mode"] });
    return () => observer.disconnect();
  });
</script>

<svelte:head>
  <title>Tokens · V1 sweep — EasyQuran design</title>
  <meta name="robots" content="noindex, nofollow" />
</svelte:head>

<div class="mx-auto w-full max-w-[1100px] px-5 py-12">
  <header class="flex flex-col gap-3 border-b border-line pb-8">
    <span class="eyebrow text-accent">V1 · token sweep</span>
    <h1 class="text-[34px] leading-[1.1] tracking-[-0.02em]">Every token, labelled, computed.</h1>
    <p class="max-w-[62ch] text-[15px] leading-[1.6] text-fg-2">
      A wrong-coloured square here means a token is missing or malformed in one of the eight
      palette blocks — caught the moment it happens, not weeks later on one page in one palette.
    </p>
    <p class="font-mono text-xs text-fg-3">
      data-palette="<span class="text-fg">{palette}</span>" · data-mode="<span class="text-fg"
        >{mode}</span
      >"
    </p>
  </header>

  <div aria-hidden="true" bind:this={probe} class="pointer-events-none fixed h-px w-px opacity-0"></div>

  {#each GROUPS as group (group.title)}
    <section class="flex flex-col gap-4 border-b border-line py-9 last:border-b-0">
      <div class="flex flex-col gap-1">
        <h2 class="text-[20px] tracking-[-0.01em]">{group.title}</h2>
        {#if group.note}<p class="max-w-[62ch] text-[13px] leading-[1.55] text-fg-3">{group.note}</p>{/if}
      </div>
      <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {#each group.tokens as token (token)}
          {@const value = values[token]}
          <div class="flex items-center gap-3 rounded-md border border-line bg-bg-1 p-3">
            <div
              class="h-11 w-11 shrink-0 rounded-sm border border-line"
              style="background: var({token})"
            ></div>
            <div class="flex min-w-0 flex-col gap-0.5">
              <span class="font-mono text-xs font-medium text-fg">{token}</span>
              {#if value && value.specified}
                <span class="truncate font-mono text-[11px] text-fg-3">{value.specified}</span>
                <span class="truncate font-mono text-[11px] text-fg-4">{value.resolved}</span>
              {:else}
                <span class="font-mono text-[11px] font-semibold text-danger">not defined</span>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    </section>
  {/each}

  <section class="flex flex-col gap-4 pb-6">
    <div class="flex flex-col gap-1">
      <h2 class="text-[20px] tracking-[-0.01em]">Foreground-on-fill pairs</h2>
      <p class="max-w-[62ch] text-[13px] leading-[1.55] text-fg-3">
        The pairs the contrast gate asserts at 4.5:1, as real text on the real fill.
      </p>
    </div>
    <div class="flex flex-wrap gap-3">
      {#each PAIR_CHIPS as chip (chip.label)}
        {@const defined = Boolean(values[chip.fg]?.specified && values[chip.fill]?.specified)}
        <span
          class="rounded-pill px-4 py-2 text-[14px] font-medium"
          style="background: var({chip.fill}); color: var({chip.fg})"
        >
          {#if defined}Read me{:else}<span class="text-danger">pair not defined</span>{/if}
        </span>
      {/each}
    </div>
    <p class="font-mono text-[11px] text-fg-4">{PAIR_CHIPS.map((c) => c.label).join(" · ")}</p>
  </section>
</div>
