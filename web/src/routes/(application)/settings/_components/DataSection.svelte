<script lang="ts">
  import { toast } from "svelte-sonner";
  import { cn } from "#lib/utils.js";
  import type { SettingsCopy } from "#lib/i18n/settings-copy.js";
  import {
    applyImport,
    downloadExportFile,
    livePorts,
    parseExportBundle,
    type ParseFailure,
  } from "#lib/storage/data-export.js";

  let {
    id,
    heading,
    copy,
  }: {
    id: string;
    heading: string;
    copy: SettingsCopy["data"];
  } = $props();

  let fileInput = $state<HTMLInputElement>();

  const quiet =
    "rounded-lg border border-border-strong px-3.5 py-2.5 text-caption text-foreground-secondary transition-colors hover:border-border hover:text-foreground";

  function failureMessage(reason: ParseFailure): string {
    if (reason === "future-version") return copy.errors.future;
    if (reason === "not-export") return copy.errors.notExport;
    return copy.errors.malformed;
  }

  async function onPick(): Promise<void> {
    const input = fileInput;
    const file = input?.files?.[0];
    if (input) input.value = "";
    if (file === undefined) return;
    let raw: unknown;
    try {
      raw = JSON.parse(await file.text());
    } catch {
      // Unreadable bytes and invalid JSON both mean the file is not an export.
      toast(copy.errors.malformed);
      return;
    }
    const outcome = parseExportBundle(raw);
    if (!outcome.ok) {
      toast(failureMessage(outcome.reason));
      return;
    }
    const summary = applyImport(outcome.value, livePorts());
    const message = copy.imported(summary.added, summary.skipped);
    toast(summary.invalid > 0 ? `${message} ${copy.invalid(summary.invalid)}` : message);
  }
</script>

<div {id} tabindex="-1" class="scroll-mt-24">
  <h2 class="text-[17px] font-medium tracking-[-0.02em] text-foreground">{heading}</h2>
  <p class="mt-1 max-w-[70ch] text-[14.5px] leading-relaxed text-foreground-secondary">{copy.intro}</p>

  <div class="mt-6 divide-y divide-border overflow-hidden rounded-xl border border-border-strong bg-surface">
    <div class="flex flex-wrap items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
      <div class="min-w-0">
        <h3 class="text-[14.5px] font-medium text-foreground">{copy.exportLabel}</h3>
        <p class="mt-0.5 text-caption leading-snug text-muted">{copy.exportNote}</p>
      </div>
      <button type="button" onclick={() => downloadExportFile()} class={cn(quiet, "shrink-0")}>
        {copy.exportLabel}
      </button>
    </div>

    <div class="flex flex-wrap items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
      <div class="min-w-0">
        <h3 class="text-[14.5px] font-medium text-foreground">{copy.importLabel}</h3>
        <p class="mt-0.5 text-caption leading-snug text-muted">{copy.importNote}</p>
      </div>
      <input
        bind:this={fileInput}
        type="file"
        accept="application/json,.json"
        class="sr-only"
        tabindex="-1"
        aria-hidden="true"
        onchange={onPick}
      />
      <button type="button" onclick={() => fileInput?.click()} class={cn(quiet, "shrink-0")}>
        {copy.importLabel}
      </button>
    </div>
  </div>
</div>
