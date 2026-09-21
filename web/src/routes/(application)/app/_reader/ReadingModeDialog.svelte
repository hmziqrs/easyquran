<script lang="ts">
  import { Dialog } from "bits-ui";
  import { flagFor } from "$lib/quran/catalogue";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import type { ReadingCandidate } from "./reading-mode-guard.svelte";

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

  function nameFor(c: ReadingCandidate): string {
    if (!c.entry) return copy.sources.arabic;
    return c.entry.name;
  }
  function languageFor(c: ReadingCandidate): string {
    if (!c.entry) return copy.sources.arabic;
    return c.entry.language;
  }
  function flagForCandidate(c: ReadingCandidate): string {
    return flagFor(c.entry?.languageCode ?? "ar").flag;
  }
  // Mirrors the TranslationModal row anatomy: the author line exists only when
  // it adds information beyond the name shown right above it.
  function hasAuthorLine(c: ReadingCandidate): boolean {
    if (!c.entry?.translator) return false;
    return c.entry.translator.trim().toLowerCase() !== c.entry.name.trim().toLowerCase();
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
      class="fixed left-1/2 top-1/2 z-50 flex max-h-[85vh] w-[min(92vw,460px)] -translate-x-1/2 -translate-y-1/2 flex-col gap-4 overflow-hidden rounded-xl border border-border bg-popover bg-clip-padding pt-[calc(1.25rem+env(safe-area-inset-top))] pb-[calc(1.25rem+env(safe-area-inset-bottom))] ps-[calc(1.25rem+env(safe-area-inset-left))] pe-[calc(1.25rem+env(safe-area-inset-right))] text-popover-foreground shadow-lg"
    >
      <!-- S11 (stress A3): base padding + env(safe-area-inset-*) keeps content
           clear of notches/home indicators where insets are reported, and
           collapses to the plain 1.25rem everywhere else. -->
      <Dialog.Title class="text-[17px] font-semibold leading-tight">
        {copy.translations.readingConfirmTitle}
      </Dialog.Title>

      {#if candidates.length <= 1}
        <Dialog.Description class="text-sm leading-relaxed text-foreground-secondary">
          {copy.translations.readingConfirmSingle(nameFor(candidates[0] ?? { id: null, entry: null }))}
        </Dialog.Description>
      {:else}
        <Dialog.Description class="text-sm leading-relaxed text-foreground-secondary">
          {copy.translations.readingConfirmBody}
        </Dialog.Description>
        <fieldset class="flex min-h-0 flex-col gap-1 overflow-y-auto overscroll-contain rounded-lg border border-border bg-background-subtle p-2">
          <legend class="mb-1 px-1.5 text-xs font-medium text-muted-foreground">
            {copy.translations.readingConfirmChoose}
          </legend>
          <div class="flex flex-col gap-0.5" role="radiogroup" aria-label={copy.translations.readingConfirmChoose}>
            {#each candidates as c (c.id ?? "arabic")}
              {@const value = c.id ?? "arabic"}
              <!-- S9/S13 (stress A1/A4): the label already makes the whole row
                   the radio's target; min-h-11 lifts it to a >=44px target and
                   touch-manipulation kills the double-tap-zoom window. -->
              <label
                data-candidate={value}
                class="flex min-h-11 cursor-pointer touch-manipulation items-center gap-3 rounded-lg px-2.5 py-2 text-sm text-foreground-secondary transition-colors hover:bg-surface-hover hover:text-foreground"
              >
                <input
                  type="radio"
                  name="reading-source"
                  {value}
                  checked={selected === value}
                  onchange={() => (selected = value)}
                  class="size-[18px] flex-none cursor-pointer accent-primary"
                />
                <span class="flex w-[92px] flex-none items-center gap-1.5 truncate text-xs text-muted-foreground">
                  <span aria-hidden="true">{flagForCandidate(c)}</span>
                  {languageFor(c)}
                </span>
                <span class="min-w-0 flex-1 py-0.5">
                  <span class="block truncate font-medium text-foreground">{nameFor(c)}</span>
                  {#if hasAuthorLine(c)}
                    <span class="block truncate text-[12.5px] leading-snug text-muted-foreground">
                      {c.entry?.translator}
                    </span>
                  {/if}
                </span>
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
              class="flex h-11 cursor-pointer touch-manipulation items-center rounded-lg px-4 text-sm font-medium text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground"
            >
              {copy.translations.readingConfirmCancel}
            </button>
          {/snippet}
        </Dialog.Close>
        <button
          type="button"
          onclick={confirm}
          data-reading-confirm
          class="flex h-11 cursor-pointer touch-manipulation items-center rounded-lg bg-primary px-5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary-hover"
        >
          {copy.translations.readingConfirmApply}
        </button>
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
