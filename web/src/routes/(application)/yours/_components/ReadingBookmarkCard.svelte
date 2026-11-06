<script lang="ts">
  import type { QuranData } from "#lib/data/quran-data.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Card } from "#lib/components/ui/card/index.js";
  import { surahRouteContext } from "#lib/data/quran.js";
  import { appLocale } from "#lib/i18n/app-locale.js";
  import {
    reader_notes_source_arabic,
    reader_reading_bookmark_detail,
    reader_reading_bookmark_open,
    reader_reading_bookmark_set_on,
    reader_reading_bookmark_title,
  } from "#lib/i18n/m/reader.js";
  import { currentReadingBookmark } from "#lib/preferences/reading-bookmark.svelte.js";
  import { openVerseAt } from "#lib/reader/resume.js";
  import { peekTranslationName } from "#lib/quran/catalogue.js";

  let { quranData }: { quranData: QuranData | undefined } = $props();

  const locale = appLocale();
  const msgOptions = { locale };
  const dateFormat = new Intl.DateTimeFormat(locale, { dateStyle: "medium" });
  const ARABIC_CTX = surahRouteContext("uthmani");

  const place = $derived(currentReadingBookmark());
  const source = $derived(
    place === null
      ? ""
      : (peekTranslationName(place.sourceId ?? undefined) ??
          reader_notes_source_arabic(undefined, msgOptions)),
  );
  const surahName = $derived(
    place === null ? "" : (quranData?.surahByNum(place.surah)?.name ?? `Surah ${place.surah}`),
  );

  function open(): void {
    if (place === null) return;
    void openVerseAt(place.surah, place.ayah, place.sourceId ?? undefined, ARABIC_CTX);
  }
</script>

{#if place !== null}
  <section aria-labelledby="yours-reading-place" class="mt-5">
    <h2 id="yours-reading-place" class="eyebrow">
      {reader_reading_bookmark_title(undefined, msgOptions)}
    </h2>
    <Card class="mt-2 flex flex-row items-center justify-between gap-3 px-4 py-3.5 sm:px-5">
      <div class="min-w-0">
        <p class="text-[14.5px] font-medium text-foreground">
          {reader_reading_bookmark_detail({ surah: surahName, ayah: place.ayah }, msgOptions)}
        </p>
        <p class="mt-0.5 text-caption text-muted">
          {reader_reading_bookmark_set_on(
            { source, date: dateFormat.format(place.setAt) },
            msgOptions,
          )}
        </p>
      </div>
      <Button variant="ghost" size="sm" class="shrink-0" onclick={open}>
        {reader_reading_bookmark_open(undefined, msgOptions)}
      </Button>
    </Card>
  </section>
{/if}
