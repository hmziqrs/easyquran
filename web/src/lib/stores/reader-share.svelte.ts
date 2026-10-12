import { browser } from "$app/env";
import { page } from "$app/state";

import { peekQuranData } from "#lib/data/quran-data-client.js";
import {
  parseKey,
  routeContextFromParams,
  surahAyahPathFor,
  surahPathFor,
  type VerseKey,
} from "#lib/data/quran.js";
import type { CatalogEntry } from "#lib/data/quran-types.js";
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

function rangeShareText(verses: readonly ShareVerse[], link: string): string {
  const body = verses.map((v) => `${stripTajweedMarkup(v.text)}\n${verseRef(v.key)}`).join("\n\n");
  return `${body}\n\n${link}`;
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
    /** Range variant: every verse's text + ref, joined, with the canonical `?ayah=` URL. */
    async shareRange(
      from: VerseKey,
      to: VerseKey,
      verses: readonly ShareVerse[],
    ): Promise<ShareResult> {
      if (!browser) return "failed";
      const link = rangeDeepLink(from, to);
      const title = rangeTitle(from, to);
      const text = rangeShareText(verses, link);
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
  };
}
