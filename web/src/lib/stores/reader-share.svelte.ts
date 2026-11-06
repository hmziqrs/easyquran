import { browser } from "$app/env";
import { page } from "$app/state";

import { peekQuranData } from "#lib/data/quran-data-client.js";
import type { CatalogEntry } from "#lib/data/quran-types.js";
import {
  parseKey,
  routeContextFromParams,
  surahAyahPathFor,
  surahPathFor,
  type VerseKey,
} from "#lib/data/quran.js";
import { stripTajweedMarkup } from "#lib/quran/view/tajweed.js";

export type ShareResult = "shared" | "copied" | "link-copied" | "cancelled" | "failed";

export type ShareVerse = { key: VerseKey; text: string };

function surahEntry(num: number): Pick<CatalogEntry, "slug" | "num"> {
  // Quran data is loaded before any reader UI renders; the fallback only
  // shields tests and the crash path — the slug shape mirrors the catalogue.
  return peekQuranData()?.surahByNum(num) ?? { slug: `surah-${num}`, num };
}

function verseRef(key: VerseKey): string {
  const { num, n } = parseKey(key);
  const name = peekQuranData()?.surahByNum(num)?.name ?? `Surah ${num}`;
  return `${name} ${num}:${n}`;
}

/** Absolute deep link (FLOW-05): the reader URL for one ayah, in the open translation context. */
function verseDeepLink(key: VerseKey): string {
  const { num, n } = parseKey(key);
  const path = surahAyahPathFor(routeContextFromParams(page.params), surahEntry(num), n);
  return `${page.url.origin}${path}`;
}

/** Canonical range permalink (R11, §11.5 settled): `…/{slug}?ayah=2:255-2:286`. */
function rangeDeepLink(from: VerseKey, to: VerseKey): string {
  const a = parseKey(from);
  const b = parseKey(to);
  const path = surahPathFor(routeContextFromParams(page.params), surahEntry(a.num));
  return `${page.url.origin}${path}?ayah=${a.num}:${a.n}-${b.num}:${b.n}`;
}

function rangeTitle(from: VerseKey, to: VerseKey): string {
  const a = parseKey(from);
  const b = parseKey(to);
  if (a.num !== b.num) return `${verseRef(from)} – ${verseRef(to)}`;
  const name = peekQuranData()?.surahByNum(a.num)?.name ?? `Surah ${a.num}`;
  return `${name} ${a.num}:${a.n}–${b.n}`;
}

function verseShareText(key: VerseKey, text: string, link: string): string {
  // Clipboard/share are plain-text views: tajweed markup renders as colors in
  // the reader, but must not leak `[h:1[ٱ]` tokens into copied text.
  return `${stripTajweedMarkup(text)}\n${verseRef(key)}\n${link}`;
}

/** What a range share carries. Both default on; the dialog can switch either off. */
export interface ShareOptions {
  readonly includeReference: boolean;
  readonly includeLink: boolean;
}

export const DEFAULT_SHARE_OPTIONS: ShareOptions = { includeReference: true, includeLink: true };

/** Plain-text range body: verse text, optional ref per verse, optional trailing link. */
export function rangeShareText(
  verses: readonly ShareVerse[],
  link: string,
  options: ShareOptions = DEFAULT_SHARE_OPTIONS,
): string {
  const body = verses.map((v) => {
    const text = stripTajweedMarkup(v.text);
    return options.includeReference ? `${text}\n${verseRef(v.key)}` : text;
  });
  const parts = [body.join("\n\n")];
  if (options.includeLink) parts.push(link);
  return parts.join("\n\n");
}

async function writeClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

export function createReaderShare() {
  return {
    async copyVerse(key: VerseKey, text: string): Promise<boolean> {
      if (!browser) return false;
      return writeClipboard(verseShareText(key, text, verseDeepLink(key)));
    },
    async shareVerse(key: VerseKey, verse: string): Promise<ShareResult> {
      if (!browser) return "failed";
      const link = verseDeepLink(key);
      const ref = verseRef(key);
      const text = verseShareText(key, verse, link);
      try {
        if (navigator.share) {
          await navigator.share({ title: ref, text, url: link });
          return "shared";
        }
        return (await writeClipboard(text)) ? "copied" : "failed";
      } catch (cause) {
        // The user dismissed the share sheet — no feedback, nothing copied.
        if (cause instanceof DOMException && cause.name === "AbortError") return "cancelled";
        // Couldn't share — the link still travels: copy it and say so (FLOW-05).
        return (await writeClipboard(link)) ? "link-copied" : "failed";
      }
    },
    /** Canonical `…/{slug}?ayah=S:N-S:M` permalink for a range. */
    rangeLink(from: VerseKey, to: VerseKey): string {
      return rangeDeepLink(from, to);
    },
    /** Range variant: every verse's text (+ ref), joined, with the canonical `?ayah=` URL. */
    async shareRange(
      from: VerseKey,
      to: VerseKey,
      verses: readonly ShareVerse[],
      options: ShareOptions = DEFAULT_SHARE_OPTIONS,
    ): Promise<ShareResult> {
      if (!browser) return "failed";
      const link = rangeDeepLink(from, to);
      const title = rangeTitle(from, to);
      const text = rangeShareText(verses, link, options);
      try {
        if (navigator.share) {
          await navigator.share({ title, text, url: link });
          return "shared";
        }
        return (await writeClipboard(text)) ? "copied" : "failed";
      } catch (cause) {
        if (cause instanceof DOMException && cause.name === "AbortError") return "cancelled";
        return (await writeClipboard(link)) ? "link-copied" : "failed";
      }
    },
    /** Clipboard-only range copy: the dialog's fast path, no share sheet. */
    async copyRange(
      from: VerseKey,
      to: VerseKey,
      verses: readonly ShareVerse[],
      options: ShareOptions = DEFAULT_SHARE_OPTIONS,
    ): Promise<boolean> {
      if (!browser) return false;
      return writeClipboard(rangeShareText(verses, rangeDeepLink(from, to), options));
    },
    async copyLink(link: string): Promise<boolean> {
      if (!browser) return false;
      return writeClipboard(link);
    },
  };
}
