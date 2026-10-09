import type { Reroute } from "@sveltejs/kit/hooks";

import { deLocalizeUrl } from "#lib/paraglide/runtime.js";

const SURAH_SEGMENT = "[a-z][a-z0-9]*(?:-[a-z0-9]+)*";
const CONTENT_LANGUAGE_SEGMENT = "[a-z][a-z0-9]*(?:-[a-z0-9]+)*";
// Keep in lockstep with the TRANSLATOR_SEGMENT copies in src/lib/i18n/reader.ts,
// src/lib/accept-parse.ts, and src/lib/server/reader-route.ts (baked ids include
// underscore translator segments, e.g. quranenc.en.hilali_khan).
const TRANSLATOR_SEGMENT = "[a-z0-9]+(?:[._-][a-z0-9]+)*";
const NUMBER = "[1-9][0-9]*";
const RANGE_SEGMENT = "(?:page|juz|hizb)";
// Scheme A: reader routes live at the site root (no `/app` marker). The
// bounded app pages join the same mechanism (Q1 default): /ar/search,
// /ar/settings, /ar/bookmarks and /ar/yours reroute onto their unprefixed
// twins. Numeric paths are NOT here — digits are owned by the hooks.server
// numeric-alias 308, never a reroute.
const READER_ROUTE_PATTERNS = [
  new RegExp("^/(?:juz|surah|pages)$", "u"),
  new RegExp(`^/${SURAH_SEGMENT}$`, "u"),
  new RegExp(`^/${RANGE_SEGMENT}/${NUMBER}$`, "u"),
  new RegExp(`^/${SURAH_SEGMENT}/t/${CONTENT_LANGUAGE_SEGMENT}/${TRANSLATOR_SEGMENT}$`, "u"),
  new RegExp(
    `^/t/${CONTENT_LANGUAGE_SEGMENT}/${TRANSLATOR_SEGMENT}/${RANGE_SEGMENT}/${NUMBER}$`,
    "u",
  ),
  new RegExp(`^/${SURAH_SEGMENT}\\.md$`, "u"),
  new RegExp(`^/${RANGE_SEGMENT}/${NUMBER}\\.md$`, "u"),
  new RegExp(`^/${SURAH_SEGMENT}/t/${CONTENT_LANGUAGE_SEGMENT}/${TRANSLATOR_SEGMENT}\\.md$`, "u"),
  new RegExp(
    `^/t/${CONTENT_LANGUAGE_SEGMENT}/${TRANSLATOR_SEGMENT}/${RANGE_SEGMENT}/${NUMBER}\\.md$`,
    "u",
  ),
  new RegExp("^/(?:search|settings|bookmarks|yours)$", "u"),
];

function isReaderRoute(pathname: string): boolean {
  return (
    pathname.length <= 256 &&
    !pathname.includes("%") &&
    !pathname.includes("//") &&
    READER_ROUTE_PATTERNS.some((pattern) => pattern.test(pathname))
  );
}

function localizedReaderTuple(rawPathname: string, candidatePathname: string): boolean {
  // Scheme A: only `/ar` prefixes reroute — `/en/**` is owned by the
  // hooks.server 308 (never rerouted to a 200), and an unprefixed path is its
  // own candidate (deLocalizeUrl is identity on it).
  if (!rawPathname.startsWith("/ar/")) return false;
  const candidate = rawPathname.slice(3);
  return candidate === candidatePathname && isReaderRoute(candidate);
}

export const reroute: Reroute = ({ url }) => {
  const candidate = deLocalizeUrl(url);
  if (localizedReaderTuple(url.pathname, candidate.pathname)) return candidate.pathname;
  if ((url.pathname === "/ar" || url.pathname === "/ar/") && candidate.pathname === "/") {
    return "/";
  }
  return url.pathname;
};
