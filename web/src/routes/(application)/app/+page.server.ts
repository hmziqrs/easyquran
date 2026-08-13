import { readerPrerenderHrefs } from "$lib/components/i18n/reader-prerender.server";
import { SUPPORTED_UI_LOCALES } from "$lib/i18n/locales";
import type { PublicHref } from "$lib/i18n/public-href";
import { readerHrefFor } from "$lib/i18n/reader";
import { readerEntryPath } from "$lib/i18n/seo";
import { QURAN_DATA } from "$lib/server/quran-data";

export function load(): { readerPrerenderHrefs: PublicHref[] } {
  return {
    readerPrerenderHrefs: readerPrerenderHrefs(
      QURAN_DATA,
      SUPPORTED_UI_LOCALES,
      readerHrefFor,
      readerEntryPath,
    ),
  };
}
