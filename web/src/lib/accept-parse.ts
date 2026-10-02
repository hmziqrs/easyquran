import { MARKETING_ROUTES } from "./config/site-structure";

export interface AcceptEntry {
  readonly type: string;
  readonly q: number;
  readonly specificity: number;
}

export type MdNegotiation =
  | { kind: "passthrough" }
  | { kind: "markdown"; mdPath: string }
  | { kind: "not-acceptable"; accept: string };

const NEGOTIABLE_TYPES = ["text/html", "text/markdown"] as const;

const MD_SIBLING_PATHS: ReadonlyMap<string, string> = new Map(
  Object.values(MARKETING_ROUTES)
    .filter((href) => href === "/" || !href.slice(1).includes("/"))
    .map((href) => [href, href === "/" ? "/index.md" : `${href}.md`] as const),
);

const SURAH_SEGMENT = "[a-z][a-z0-9]*(?:-[a-z0-9]+)*";
const CONTENT_LANGUAGE_SEGMENT = "[a-z][a-z0-9]*(?:-[a-z0-9]+)*";
// Keep in lockstep with the TRANSLATOR_SEGMENT copies in src/lib/i18n/reader.ts, src/hooks.ts,
// and src/lib/server/reader-route.ts (baked ids include underscore translator segments,
// e.g. quranenc.en.hilali_khan).
const TRANSLATOR_SEGMENT = "[a-z0-9]+(?:[._-][a-z0-9]+)*";
const NUMBER = "[1-9][0-9]*";
const RANGE_SEGMENT = "(?:page|juz|hizb|rub)";
// Scheme A: reader content paths are prefix-less; only `ar` requests carry a
// locale prefix. The same four families serve both the html→md sibling lookup
// (optionally /ar-prefixed) and the raw .md request gate (isReaderMdPath).
const READER_MD_PATH_FAMILIES = [
  `/${SURAH_SEGMENT}`,
  `/${RANGE_SEGMENT}/${NUMBER}`,
  `/${SURAH_SEGMENT}/t/${CONTENT_LANGUAGE_SEGMENT}/${TRANSLATOR_SEGMENT}`,
  `/t/${CONTENT_LANGUAGE_SEGMENT}/${TRANSLATOR_SEGMENT}/${RANGE_SEGMENT}/${NUMBER}`,
];
const READER_MD_SIBLING_PATTERNS: readonly RegExp[] = READER_MD_PATH_FAMILIES.map(
  (family) => new RegExp(`^(?:/ar)?${family}$`, "u"),
);
const READER_MD_REQUEST_PATTERNS: readonly RegExp[] = READER_MD_PATH_FAMILIES.map(
  (family) => new RegExp(`^(?:/ar)?${family}\\.md$`, "u"),
);

/** True when the pathname is a prerendered reader .md artifact under scheme A
 * (`/al-baqarah.md`, `/ar/al-baqarah.md`, `/t/en/sahih/page/7.md`, …). Shared
 * with web/server.ts so the pattern list has exactly one copy. */
export function isReaderMdPath(pathname: string): boolean {
  return READER_MD_REQUEST_PATTERNS.some((pattern) => pattern.test(pathname));
}

function readerMdSibling(pathname: string): string | null {
  const hasPrefix = pathname.startsWith("/ar/");
  const bare = hasPrefix ? pathname.slice(3) : pathname;
  // The bare locale spellings are the marketing home, not a reader route —
  // without this guard the broad surah family would claim "/ar" itself.
  if (bare === "/juz" || bare === "/ar" || bare === "/en" || bare === "" || bare === "/") {
    return null;
  }
  // Marketing pages own their .md twins (MD_SIBLING_PATHS); the broad
  // single-segment reader family must never claim them, including the
  // /ar-prefixed spellings that have no published marketing twin.
  if (MD_SIBLING_PATHS.has(bare)) return null;
  return READER_MD_SIBLING_PATTERNS.some((pattern) => pattern.test(pathname))
    ? `${pathname}.md`
    : null;
}

function specificityOf(type: string): number {
  if (type === "*/*") return 0;
  if (type.endsWith("/*")) return 1;
  return 2;
}

export function parseAccept(header: string): AcceptEntry[] {
  const entries: AcceptEntry[] = [];
  for (const part of header.split(",")) {
    const segments = part.split(";");
    const type = (segments[0] ?? "").trim().toLowerCase();
    if (!type.includes("/")) continue;
    let q = 1;
    for (const param of segments.slice(1)) {
      const eq = param.indexOf("=");
      if (eq === -1) continue;
      if (param.slice(0, eq).trim().toLowerCase() !== "q") continue;
      const parsed = Number(param.slice(eq + 1));
      q = Number.isFinite(parsed) ? Math.min(1, Math.max(0, parsed)) : 1;
      break;
    }
    entries.push({ type, q, specificity: specificityOf(type) });
  }
  return entries;
}

interface AcceptMatch {
  readonly entry: AcceptEntry;
  readonly position: number;
}

function bestMatch(entries: AcceptEntry[], candidate: string): AcceptMatch | null {
  const candidateType = candidate.toLowerCase();
  const major = candidateType.split("/")[0] ?? "";
  let best: AcceptMatch | null = null;
  for (let position = 0; position < entries.length; position += 1) {
    const entry = entries[position]!;
    const matches =
      entry.type === candidateType ||
      entry.type === "*/*" ||
      (entry.type.endsWith("/*") && entry.type.slice(0, -2) === major);
    if (!matches) continue;
    if (best === null || entry.specificity > best.entry.specificity) {
      best = { entry, position };
    }
  }
  return best;
}

export function preferredType(header: string | null, produces: readonly string[]): string | null {
  if (produces.length === 0) return null;
  if (header === null) return produces[0]!;
  const entries = parseAccept(header);
  if (entries.length === 0) return produces[0]!;
  let chosen: string | null = null;
  let chosenQ = 0;
  let chosenPosition = -1;
  for (const candidate of produces) {
    const match = bestMatch(entries, candidate);
    if (match === null || match.entry.q <= 0) continue;
    if (match.entry.q > chosenQ || (match.entry.q === chosenQ && match.position < chosenPosition)) {
      chosen = candidate;
      chosenQ = match.entry.q;
      chosenPosition = match.position;
    }
  }
  return chosen;
}

export function varyWithAccept(existing: string | null | undefined): string {
  if (existing === null || existing === undefined || existing === "") return "Accept";
  const hasAccept = existing.split(",").some((token) => token.trim().toLowerCase() === "accept");
  return hasAccept ? existing : `${existing}, Accept`;
}

export function appendVaryAccept(headers: Headers): void {
  headers.set("Vary", varyWithAccept(headers.get("vary")));
}

export function mdSiblingPathFor(pathname: string): string | null {
  return MD_SIBLING_PATHS.get(pathname) ?? readerMdSibling(pathname);
}

export function negotiateMarkdownPath(pathname: string, accept: string | null): MdNegotiation {
  const mdPath = mdSiblingPathFor(pathname);
  if (mdPath === null) return { kind: "passthrough" };
  const chosen = preferredType(accept, NEGOTIABLE_TYPES);
  if (accept !== null && chosen === null) return { kind: "not-acceptable", accept };
  if (chosen === "text/markdown") return { kind: "markdown", mdPath };
  return { kind: "passthrough" };
}

export function notAcceptableBody(accept: string): string {
  return `406 Not Acceptable. Available representations: text/html, text/markdown. Requested Accept: ${accept}\n`;
}
