<script lang="ts">
  import { toast } from "svelte-sonner";
  import { peekQuranData } from "#lib/data/quran-data-client.js";
  import type { VerseKey } from "#lib/data/quran.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Checkbox } from "#lib/components/ui/checkbox/index.js";
  import * as Dialog from "#lib/components/ui/dialog/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import { appLocale } from "#lib/i18n/app-locale.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import {
    reader_share_card_brand,
    reader_share_card_footer,
    reader_share_copy,
    reader_share_copy_link,
    reader_share_dialog_description,
    reader_share_dialog_title,
    reader_share_download,
    reader_share_format,
    reader_share_format_image,
    reader_share_format_text,
    reader_share_from,
    reader_share_image_failed,
    reader_share_image_saved,
    reader_share_include_link,
    reader_share_include_reference,
    reader_share_loading,
    reader_share_range_invalid,
    reader_share_range_too_long,
    reader_share_to,
  } from "#lib/i18n/m/reader.js";
  import { reader } from "#lib/stores/reader.svelte.js";
  import type { ShareVerse } from "#lib/stores/reader-share.svelte.js";
  import { renderVerseCard, type CardVerse } from "#lib/share/verse-card.js";
  import { SHARE_SPAN_MAX, rangeProblem, type RangeProblem } from "#lib/share/range.js";

  let {
    open = $bindable(false),
    surah,
    ayah,
    currentText,
  }: {
    open?: boolean;
    surah: number;
    /** The verse the dialog was opened from; the range starts here. */
    ayah: number;
    /** That verse's text as the row shows it, used when the surah cache has not caught up. */
    currentText: string;
  } = $props();

  const copy = getReaderUiCopy();
  const msg = { locale: appLocale() };

  const ayahCount = $derived(peekQuranData()?.surahByNum(surah)?.ayahCount ?? 0);
  // svelte-ignore state_referenced_locally -- the dialog mounts per open; the start verse is fixed for its lifetime
  let from = $state(ayah);
  // svelte-ignore state_referenced_locally -- same as `from`
  let to = $state(ayah);
  let format = $state<"text" | "image">("text");
  let includeReference = $state(true);
  let includeLink = $state(true);
  let busy = $state(false);

  const problem = $derived<RangeProblem | null>(rangeProblem(from, to, ayahCount));

  // The reader's current text for each ayah in range; null while any is still loading.
  const verses = $derived.by<ShareVerse[] | null>(() => {
    if (problem !== null) return null;
    const texts = reader.versesFor(surah);
    const out: ShareVerse[] = [];
    for (let n = from; n <= to; n += 1) {
      const text = texts[n - 1] ?? (n === ayah ? currentText : undefined);
      if (text === undefined || text === "") return null;
      // SAFETY: `${surah}:${n}` is a numeric surah and ayah in the VerseKey shape.
      out.push({ key: `${surah}:${n}` as VerseKey, text });
    }
    return out;
  });

  // SAFETY: both keys are built from numeric surah and ayah, the VerseKey shape.
  const fromKey = $derived(`${surah}:${from}` as VerseKey);
  // SAFETY: same numeric VerseKey shape as fromKey.
  const toKey = $derived(`${surah}:${to}` as VerseKey);
  const link = $derived(problem === null ? reader.rangeLink(fromKey, toKey) : "");
  const surahName = $derived(peekQuranData()?.surahByNum(surah)?.name ?? `Surah ${surah}`);

  function refFor(n: number): string {
    return `${surahName} ${surah}:${n}`;
  }

  function problemMessage(value: RangeProblem): string {
    if (value === "too-long") return reader_share_range_too_long({ max: SHARE_SPAN_MAX }, msg);
    return reader_share_range_invalid(undefined, msg);
  }

  function readInt(event: Event): number {
    // SAFETY: bound only to the two range inputs below, both <input type="number">.
    const raw = (event.currentTarget as HTMLInputElement).value;
    return raw === "" ? Number.NaN : Number(raw);
  }

  async function onCopyText(): Promise<void> {
    if (verses === null) return;
    const ok = await reader.copyRange(fromKey, toKey, verses, { includeReference, includeLink });
    if (ok) {
      toast(copy.verse.copied);
      open = false;
      return;
    }
    toast(copy.verse.shareUnavailable);
  }

  async function onCopyLink(): Promise<void> {
    const ok = await reader.copyLink(link);
    toast(ok ? copy.verse.copied : copy.verse.shareUnavailable);
  }

  async function onDownload(): Promise<void> {
    if (verses === null || busy) return;
    busy = true;
    try {
      const cardVerses: CardVerse[] = verses.map((v, index) => ({
        text: v.text,
        ref: refFor(from + index),
      }));
      const blob = await renderVerseCard({
        verses: cardVerses,
        reference: includeReference,
        link: includeLink ? link : null,
        brand: reader_share_card_brand(undefined, msg),
        footer: reader_share_card_footer(undefined, msg),
      });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `easyquran-${surah}-${from}-${to}.png`;
      anchor.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
      toast(reader_share_image_saved(undefined, msg));
    } catch {
      toast(reader_share_image_failed(undefined, msg));
    } finally {
      busy = false;
    }
  }

</script>

<Dialog.Root bind:open>
  <Dialog.Content>
    <div class="flex flex-col gap-1.5">
      <Dialog.Title>{reader_share_dialog_title(undefined, msg)}</Dialog.Title>
      <Dialog.Description>{reader_share_dialog_description(undefined, msg)}</Dialog.Description>
    </div>

    <div class="grid grid-cols-2 gap-3">
      <label class="flex flex-col gap-1.5 text-caption text-foreground-secondary">
        {reader_share_from(undefined, msg)}
        <Input
          type="number"
          min={1}
          max={ayahCount}
          value={from}
          aria-invalid={problem !== null}
          oninput={(event: Event) => (from = readInt(event))}
        />
      </label>
      <label class="flex flex-col gap-1.5 text-caption text-foreground-secondary">
        {reader_share_to(undefined, msg)}
        <Input
          type="number"
          min={1}
          max={ayahCount}
          value={to}
          aria-invalid={problem !== null}
          oninput={(event: Event) => (to = readInt(event))}
        />
      </label>
    </div>
    {#if problem !== null}
      <p role="alert" class="text-caption text-destructive">{problemMessage(problem)}</p>
    {:else if verses === null}
      <p role="status" class="text-caption text-muted-foreground">
        {reader_share_loading(undefined, msg)}
      </p>
    {/if}

    <div class="flex flex-col gap-2">
      <span class="text-caption text-foreground-secondary">
        {reader_share_format(undefined, msg)}
      </span>
      <div class="grid grid-cols-2 gap-2">
        <Button
          type="button"
          size="sm"
          variant={format === "text" ? "primary" : "secondary"}
          aria-pressed={format === "text"}
          onclick={() => (format = "text")}
        >
          {reader_share_format_text(undefined, msg)}
        </Button>
        <Button
          type="button"
          size="sm"
          variant={format === "image" ? "primary" : "secondary"}
          aria-pressed={format === "image"}
          onclick={() => (format = "image")}
        >
          {reader_share_format_image(undefined, msg)}
        </Button>
      </div>
    </div>

    <div class="flex flex-col gap-2.5">
      <label class="flex items-center gap-2.5 text-body-s text-foreground">
        <Checkbox bind:checked={includeReference} />
        {reader_share_include_reference(undefined, msg)}
      </label>
      <label class="flex items-center gap-2.5 text-body-s text-foreground">
        <Checkbox bind:checked={includeLink} />
        {reader_share_include_link(undefined, msg)}
      </label>
    </div>

    <div class="flex flex-wrap justify-end gap-2">
      <Button type="button" variant="ghost" size="sm" disabled={problem !== null} onclick={onCopyLink}>
        {reader_share_copy_link(undefined, msg)}
      </Button>
      {#if format === "text"}
        <Button type="button" size="sm" disabled={verses === null} onclick={onCopyText}>
          {reader_share_copy(undefined, msg)}
        </Button>
      {:else}
        <Button type="button" size="sm" disabled={verses === null || busy} onclick={onDownload}>
          {reader_share_download(undefined, msg)}
        </Button>
      {/if}
    </div>
  </Dialog.Content>
</Dialog.Root>
