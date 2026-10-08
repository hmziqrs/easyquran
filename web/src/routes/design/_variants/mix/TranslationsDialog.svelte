<script lang="ts">
  import { Dialog } from "bits-ui";
  import { onMount } from "svelte";
  import { Icon } from "#lib/components/icon/index.js";
  import type { TranslationCatalogueEntry } from "#lib/data/quran-types.js";
  import { TRANSLATION_CATALOGUE, TRANSLATION_CATALOGUE_BY_ID, flagFor, nativeNameFor } from "#lib/quran/catalogue.js";

  /**
   * The mix page's translation picker: the live TranslationModal's layout (search, chosen
   * chips, language rail, list, Done) driven by the page's `?t=` list instead of the reader
   * stores — the live one locks every box when the saved reader mode is Reading, and
   * rewrites the reader URL.
   */
  let {
    open = $bindable(false),
    selected,
    max,
    onChange,
  }: {
    open?: boolean;
    selected: readonly string[];
    max: number;
    onChange: (ids: readonly string[]) => void;
  } = $props();

  interface LanguageGroup {
    readonly language: string;
    readonly code: string;
    readonly flag: string;
    readonly autonym: string | null;
    readonly entries: readonly TranslationCatalogueEntry[];
  }

  const collator = new Intl.Collator("en", { sensitivity: "base" });
  let query = $state("");
  let railLanguage = $state<string | null>(null);
  let mobilePane = $state(false);
  let boost = $state.raw<string[]>([]);

  onMount(() => {
    const codes: string[] = [];
    for (const tag of navigator.languages) {
      const base = (tag.split("-")[0] ?? "").toLowerCase();
      if (base !== "" && !codes.includes(base)) codes.push(base);
    }
    boost = codes;
  });

  const needle = $derived(query.trim().toLowerCase());
  const searching = $derived(needle.length > 0);
  const isFull = $derived(selected.length >= max);

  function matchesQuery(t: TranslationCatalogueEntry): boolean {
    const hay = [t.name, t.translator ?? "", t.language, t.languageCode, nativeNameFor(t.languageCode) ?? ""];
    return hay.some((value) => value.toLowerCase().includes(needle));
  }

  const matches = $derived(searching ? TRANSLATION_CATALOGUE.filter((t) => matchesQuery(t)) : []);

  function rank(code: string): number {
    if (code === "ar") return 0;
    if (code === "en") return 1;
    const i = boost.indexOf(code);
    return i === -1 ? Number.MAX_SAFE_INTEGER : 2 + i;
  }

  const languages = $derived.by((): LanguageGroup[] => {
    const byLanguage = new Map<string, TranslationCatalogueEntry[]>();
    for (const t of TRANSLATION_CATALOGUE) {
      const list = byLanguage.get(t.language);
      if (list) list.push(t);
      else byLanguage.set(t.language, [t]);
    }
    return [...byLanguage.entries()]
      .map(([language, entries]) => {
        const code = entries[0]?.languageCode ?? "";
        return {
          language,
          code,
          flag: flagFor(code).flag,
          autonym: nativeNameFor(code),
          entries: [...entries].sort((a, b) => collator.compare(a.name, b.name)),
        };
      })
      .sort((a, b) => rank(a.code) - rank(b.code) || collator.compare(a.language, b.language));
  });

  // Opens on the first chosen translation's language, not whatever sorts first.
  $effect(() => {
    if (!open) {
      query = "";
      railLanguage = null;
      mobilePane = false;
      return;
    }
    if (railLanguage !== null) return;
    const first = selected[0] === undefined ? undefined : TRANSLATION_CATALOGUE_BY_ID.get(selected[0]);
    railLanguage = first?.language ?? languages[0]?.language ?? null;
  });

  const activeGroup = $derived(languages.find((l) => l.language === railLanguage) ?? languages[0]);
  const chosen = $derived(
    selected.flatMap((id) => {
      const entry = TRANSLATION_CATALOGUE_BY_ID.get(id);
      return entry ? [entry] : [];
    }),
  );
  const paneVisible = $derived(mobilePane || searching);

  function toggle(id: string): void {
    if (selected.includes(id)) onChange(selected.filter((x) => x !== id));
    else if (!isFull) onChange([...selected, id]);
  }

  function selectLanguage(language: string): void {
    railLanguage = language;
    mobilePane = true;
  }

  function chosenCount(group: LanguageGroup): number {
    return group.entries.filter((t) => selected.includes(t.id)).length;
  }
</script>

{#snippet row(t: TranslationCatalogueEntry, showLanguage: boolean)}
  {@const checked = selected.includes(t.id)}
  {@const locked = !checked && isFull}
  <li>
    <label
      class={[
        "flex min-h-12 items-center gap-3 rounded-lg px-3 py-2 transition-colors",
        locked ? "cursor-not-allowed opacity-50" : "cursor-pointer hover:bg-foreground/5",
        checked && "bg-primary/10",
      ]}
    >
      <span class="relative flex size-[18px] flex-none">
        <input
          type="checkbox"
          {checked}
          disabled={locked}
          onchange={() => toggle(t.id)}
          class="peer size-[18px] cursor-pointer appearance-none rounded-[5px] border-2 border-foreground/45 transition-colors checked:border-primary checked:bg-primary disabled:cursor-not-allowed"
        />
        <Icon name="check" size={12} class="pointer-events-none absolute inset-0 m-auto hidden text-primary-foreground peer-checked:block" />
      </span>
      <span class="flex min-w-0 flex-1 flex-col">
        <span class="truncate text-sm font-medium text-foreground" dir="auto">{t.name}</span>
        <span class="truncate text-[12.5px] text-foreground-secondary">
          {#if showLanguage}{flagFor(t.languageCode).flag} {t.language} ·{/if}
          {t.translator ?? t.name}
        </span>
      </span>
    </label>
  </li>
{/snippet}

<Dialog.Root bind:open>
  <!-- No portal: the panel stays inside .mix-root so it takes the page's background preset. -->
  <Dialog.Overlay class="fixed inset-0 z-[80] bg-black/55" />
  <Dialog.Content
    class="mix-dialog fixed left-1/2 top-1/2 z-[81] flex max-h-[85vh] w-[min(94vw,780px)] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-xl border border-(--mix-line) text-foreground"
  >
    <div class="flex items-center justify-between gap-3 px-5 pb-3 pt-4">
      <Dialog.Title class="text-[17px] font-semibold leading-tight">Translations</Dialog.Title>
      <Dialog.Description class="sr-only">Choose up to {max} translations to show under the Arabic.</Dialog.Description>
      <Dialog.Close>
        {#snippet child({ props })}
          <button
            {...props}
            type="button"
            aria-label="Close"
            class="flex size-11 flex-none items-center justify-center rounded-lg text-foreground-secondary transition-colors hover:text-foreground"
          >
            <Icon name="x" size={15} />
          </button>
        {/snippet}
      </Dialog.Close>
    </div>

    <div class="px-5">
      <div class="flex h-10 items-center gap-2.5 rounded-lg border border-(--mix-line) px-3.5 transition-colors focus-within:border-primary">
        <Icon name="search" size={15} class="flex-none text-foreground-secondary" />
        <input
          type="search"
          bind:value={query}
          placeholder="Search translations"
          aria-label="Search translations"
          class="flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-foreground-secondary [&::-webkit-search-cancel-button]:hidden"
        />
      </div>
    </div>

    {#if chosen.length > 0}
      <div class="flex items-center justify-between gap-3 px-5 py-3">
        <div class="flex min-w-0 flex-1 items-center gap-1.5 overflow-x-auto overscroll-contain">
          {#each chosen as t (t.id)}
            <span class="inline-flex h-10 flex-none items-center gap-1.5 rounded-pill border border-(--mix-line) pe-1 ps-3 text-[13.5px]">
              <span aria-hidden="true">{flagFor(t.languageCode).flag}</span>
              <span class="max-w-[12rem] truncate" dir="auto">{t.name}</span>
              <button
                type="button"
                aria-label={`Remove ${t.name}`}
                onclick={() => toggle(t.id)}
                class="flex size-8 items-center justify-center rounded-pill text-foreground-secondary transition-colors hover:text-foreground"
              >
                <Icon name="x" size={12} />
              </button>
            </span>
          {/each}
        </div>
        <span class="flex flex-none items-center gap-3 text-[13px] text-foreground-secondary">
          <span class="tabular-nums">{chosen.length}/{max}</span>
          <button type="button" onclick={() => onChange([])} class="transition-colors hover:text-foreground">Clear all</button>
        </span>
      </div>
    {:else}
      <p class="px-5 py-3 text-[13px] text-foreground-secondary">Arabic only — tick a translation to stack it under each ayah.</p>
    {/if}

    <div class="flex min-h-0 flex-1 border-t border-(--mix-line) md:grid md:grid-cols-[15rem_1fr]">
      <nav
        aria-label="Languages"
        class={[
          "min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto overscroll-contain p-2 md:flex md:border-e md:border-(--mix-line)",
          paneVisible ? "hidden" : "flex",
        ]}
      >
        {#each languages as l (l.language)}
          {@const active = l.language === activeGroup?.language}
          {@const count = chosenCount(l)}
          <button
            type="button"
            aria-current={active ? "true" : undefined}
            onclick={() => selectLanguage(l.language)}
            class={[
              "flex h-[52px] flex-none items-center gap-2.5 rounded-lg px-2.5 text-start transition-colors",
              active ? "bg-primary/15 text-foreground" : "text-foreground-secondary hover:text-foreground",
            ]}
          >
            <span class="flex-none text-lg leading-none" aria-hidden="true">{l.flag}</span>
            <span class="flex min-w-0 flex-1 flex-col">
              <span class={["truncate text-[15px] leading-tight", active && "font-medium"]}>{l.language}</span>
              {#if l.autonym !== null}
                <span dir="auto" class="truncate text-[12px] leading-tight text-foreground-secondary">{l.autonym}</span>
              {/if}
            </span>
            {#if count > 0}
              <span class="flex h-5 min-w-5 flex-none items-center justify-center rounded-pill bg-primary px-1.5 text-[11px] font-semibold text-primary-foreground">
                {count}
              </span>
            {:else}
              <span class="flex-none text-xs tabular-nums text-foreground-secondary">{l.entries.length}</span>
            {/if}
          </button>
        {/each}
      </nav>

      <section class={["min-h-0 flex-1 flex-col overflow-y-auto overscroll-contain md:flex", paneVisible ? "flex" : "hidden"]}>
        <div class="flex items-center gap-2 px-4 pb-1 pt-3">
          <button
            type="button"
            onclick={() => {
              query = "";
              mobilePane = false;
            }}
            class="-ms-2 flex h-10 items-center gap-1 rounded-lg px-2 text-sm text-foreground-secondary transition-colors hover:text-foreground md:hidden"
          >
            <Icon name="arrow-right" size={14} class="rotate-180" />
            Languages
          </button>
          {#if searching}
            <p class="text-[13px] text-foreground-secondary">{matches.length} results</p>
          {:else if activeGroup}
            <h3 class="text-[15px] font-semibold text-foreground">{activeGroup.language}</h3>
            <span class="text-[13px] text-foreground-secondary">{activeGroup.entries.length} translations</span>
          {/if}
        </div>
        <ul class="flex list-none flex-col gap-0.5 px-2 pb-4">
          {#if searching}
            {#each matches as t (t.id)}
              {@render row(t, true)}
            {/each}
          {:else if activeGroup}
            {#each activeGroup.entries as t (t.id)}
              {@render row(t, false)}
            {/each}
          {/if}
        </ul>
      </section>
    </div>

    <div class="flex items-center justify-between gap-3 border-t border-(--mix-line) px-5 py-3">
      <p class="text-xs text-foreground-secondary">
        {isFull ? `That's the most: ${max} at once.` : `Up to ${max} translations under the Arabic.`}
      </p>
      <Dialog.Close>
        {#snippet child({ props })}
          <button
            {...props}
            type="button"
            class="flex h-11 flex-none items-center rounded-lg bg-primary px-5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary-hover"
          >
            Done
          </button>
        {/snippet}
      </Dialog.Close>
    </div>
  </Dialog.Content>
</Dialog.Root>

<style>
  /* One step above the page ground — the live --popover fix, following the mix's preset. */
  :global(.mix-dialog) {
    background: oklch(from var(--mix-page) calc(l + 0.05) c h);
  }
</style>
