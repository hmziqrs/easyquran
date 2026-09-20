<script lang="ts">
  import { Dialog } from "bits-ui";
  import type { TranslationProvenance } from "$lib/quran/catalogue";
  import { flagFor, translationSourceOf } from "$lib/quran/catalogue";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import type { ReadingCandidate } from "./reading-mode-guard.svelte";

  // Same provenance palette as the TranslationModal chips (round-2 visual
  // language); the Arabic candidate carries no chip (provenance: null).
  const PROVENANCE_DOT = {
    qul: "bg-violet-500",
    quranenc: "bg-sky-500",
    tanzil: "bg-emerald-500",
  } satisfies Record<TranslationProvenance, string>;

  let {
    open = $bindable(false),
    candidates = [],
    onConfirm,
  }: {
    open?: boolean;
    candidates?: ReadingCandidate[];
    onConfirm: (candidate: ReadingCandidate) => void;
  } = $props();

  const copy = getReaderUiCopy();

  // Radio value per candidate: "arabic" for the Arabic current source, the
  // translation id otherwise. Re-seeded to the preselected current source
  // (candidates[0], per readingCandidates) every time the dialog opens.
  let selected = $state("");

  $effect(() => {
    if (open) selected = candidates[0]?.id ?? "arabic";
  });

  function labelFor(c: ReadingCandidate): string {
    if (!c.entry) return copy.sources.arabic;
    return c.entry.translator ?? c.entry.name;
  }
  function languageFor(c: ReadingCandidate): string {
    if (!c.entry) return copy.sources.arabic;
    return c.entry.language;
  }
  function flagForCandidate(c: ReadingCandidate): string {
    return flagFor(c.entry?.languageCode ?? "ar").flag;
  }

  function confirm(): void {
    const chosen = candidates.find((c) => (c.id ?? "arabic") === selected);
    if (!chosen) return;
    open = false;
    onConfirm(chosen);
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-50 bg-black/40 backdrop-blur-[1px]" />
    <Dialog.Content
      class="fixed left-1/2 top-1/2 z-50 flex max-h-[85vh] w-[min(92vw,460px)] -translate-x-1/2 -translate-y-1/2 flex-col gap-4 overflow-hidden rounded-lg border border-border bg-popover bg-clip-padding p-5 text-popover-foreground shadow-lg"
    >
      <Dialog.Title class="text-lg font-semibold">
        {copy.translations.readingConfirmTitle}
      </Dialog.Title>

      {#if candidates.length <= 1}
        <Dialog.Description class="text-sm leading-relaxed text-foreground-secondary">
          {copy.translations.readingConfirmSingle(labelFor(candidates[0] ?? { id: null, entry: null }))}
        </Dialog.Description>
      {:else}
        <Dialog.Description class="text-sm leading-relaxed text-foreground-secondary">
          {copy.translations.readingConfirmBody}
        </Dialog.Description>
        <fieldset class="flex min-h-0 flex-col gap-2 overflow-y-auto rounded-lg border border-border bg-background-subtle p-2">
          <legend class="mb-1 px-1.5 text-xs font-medium uppercase tracking-wider text-muted-foreground">
            {copy.translations.readingConfirmChoose}
          </legend>
          <div class="flex flex-col gap-1" role="radiogroup" aria-label={copy.translations.readingConfirmChoose}>
            {#each candidates as c (c.id ?? "arabic")}
              {@const value = c.id ?? "arabic"}
              <label
                data-candidate={value}
                class="flex cursor-pointer items-center gap-3 rounded-md px-2.5 py-2.5 text-sm text-foreground-secondary transition-colors hover:bg-surface-hover hover:text-foreground"
              >
                <input
                  type="radio"
                  name="reading-source"
                  {value}
                  checked={selected === value}
                  onchange={() => (selected = value)}
                  class="h-4 w-4 flex-none cursor-pointer accent-primary"
                />
                <span class="min-w-0 flex-1">
                  <span class="block truncate font-medium text-foreground">{labelFor(c)}</span>
                  <span class="block truncate text-xs text-muted-foreground">
                    <span aria-hidden="true">{flagForCandidate(c)}</span>
                    {languageFor(c)}
                  </span>
                </span>
                {#if c.entry}
                  <span
                    class="inline-flex flex-none items-center gap-1.5 rounded-pill bg-surface-hover px-2 py-0.5 text-[11px] font-medium text-muted-foreground"
                  >
                    <span
                      class="size-1.5 flex-none rounded-full {PROVENANCE_DOT[translationSourceOf(c.entry.id)]}"
                      aria-hidden="true"
                    ></span>
                    {copy.translations.sourceLabel(translationSourceOf(c.entry.id))}
                  </span>
                {/if}
              </label>
            {/each}
          </div>
        </fieldset>
      {/if}

      <div class="flex items-center justify-end gap-2">
        <Dialog.Close>
          {#snippet child({ props })}
            <button
              {...props}
              type="button"
              class="flex h-9 cursor-pointer items-center rounded-md px-4 text-sm font-medium text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground"
            >
              {copy.translations.readingConfirmCancel}
            </button>
          {/snippet}
        </Dialog.Close>
        <button
          type="button"
          onclick={confirm}
          data-reading-confirm
          class="flex h-9 cursor-pointer items-center rounded-md bg-foreground px-4.5 text-sm font-medium text-background transition-[filter] hover:brightness-[0.94]"
        >
          {copy.translations.readingConfirmApply}
        </button>
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
