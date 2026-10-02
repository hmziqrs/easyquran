import { readerPrerenderHrefs } from "$lib/components/i18n/reader-prerender.server";
import { SUPPORTED_UI_LOCALES } from "$lib/i18n/locales";
import { readerHrefFor } from "$lib/i18n/reader";
import { readerEntryPath } from "$lib/i18n/seo";
import { QURAN_DATA } from "$lib/server/quran-data";

// The /app hub is gone (scheme A); this prerendered index now seeds build-time
// route discovery with the same anchor set ReaderPrerenderLinks used to render
// there (minus the two dead home hrefs).
export function load() {
  return {
    readerPrerenderHrefs: readerPrerenderHrefs(
      QURAN_DATA,
      SUPPORTED_UI_LOCALES,
      readerHrefFor,
      readerEntryPath,
    ),
  };
}
