/* ════════════════════════════════════════════════════════════════════════
   render.ts — HTML → markdown / plain-text conversion for the text variants.

   The .md, .txt, and llms-full.txt endpoints each fetch the page's OWN
   prerendered HTML and run it through htmlToMarkdown. The rendered page is
   the single source of truth: no content is duplicated or hand-maintained
   here, so the text variants can never drift from what visitors see. Only
   the <main> body is converted (nav/footer/chrome are excluded), and svgs /
   scripts / canvases are stripped before conversion.

   llms.txt is a small static index built from PAGE_META (it is metadata,
   not page body, so no fetch is needed).
   ════════════════════════════════════════════════════════════════════════ */

import TurndownService from "turndown";
import { NAV_PAGES, PAGE_META, SITE } from "$lib/config/site";

const turndown = new TurndownService({
  headingStyle: "atx",
  bulletListMarker: "-",
  codeBlockStyle: "fenced",
  emDelimiter: "*",
  hr: "---",
});
// Non-content chrome that can live inside <main>: icons, scripts, canvases.
// (svg isn't in HTMLElementTagNameMap, so it's stripped via a filter rule.)
turndown.remove(["script", "style", "canvas", "noscript", "template"]);
turndown.addRule("stripSvg", {
  filter: (node) => node.nodeName.toLowerCase() === "svg",
  replacement: () => "",
});
// Drop anything explicitly marked aria-hidden (decorative dots, duplicated
// marquee halves) so the LLM-facing text never repeats content.
turndown.addRule("stripAriaHidden", {
  filter: (node) =>
    typeof (node as HTMLElement).getAttribute === "function" &&
    (node as HTMLElement).getAttribute("aria-hidden") === "true",
  replacement: () => "",
});
// Flatten split/emphasis-mangled headings into one clean line:
// "Read<br>the *Quran.*" -> "# Read the Quran."
turndown.addRule("cleanHeading", {
  filter: ["h1", "h2", "h3", "h4", "h5", "h6"],
  replacement: (content, node) => {
    const level = Number(node.nodeName.slice(1));
    const text = (content || "").replace(/\s+/g, " ").replace(/\*/g, "").trim();
    return `\n\n${"#".repeat(level)} ${text}\n\n`;
  },
});

/** Convert a full HTML document to clean markdown, using only its <main>. */
export function htmlToMarkdown(html: string): string {
  const main = html.match(/<main[^>]*>[\s\S]*?<\/main>/i);
  const body = main ? main[0] : html;
  const md = turndown
    .turndown(body)
    // strip UI chrome that has no markdown value
    .replace(/^›\s+.*$/gm, "") // section eyebrows duplicate the headings
    .replace(/ *copy$/gm, "") // copy-button label
    .replace(/ *copied ✓$/gm, "") // copied state
    .replace(/\n{3,}/g, "\n\n")
    .replace(/[ \t]+\n/g, "\n")
    .trim();
  return md + "\n";
}

/** Absolute URL of the .md variant for a page href (home -> /index.md). */
function mdVariantUrl(href: string): string {
  const path = href === "/" ? "/index.md" : `${href}.md`;
  return `${SITE.url}${path}`;
}

/**
 * The llms.txt index: site title, a one-line description, the site URL, a
 * bullet list of the pages each linking to its .md variant, and a final
 * pointer to /llms-full.txt. Built from metadata only (no page fetch).
 */
export function renderLlmsIndex(): string {
  const pageLines = NAV_PAGES.map(
    (p) => `- [${p.label}](${mdVariantUrl(p.href)}): ${PAGE_META[p.id].description}`,
  );
  return (
    [
      `# ${SITE.domain}`,
      "",
      `> ${PAGE_META.home.description}`,
      "",
      SITE.url,
      "",
      ...pageLines,
      "",
      "## Optional",
      "",
      `- [Full content of every page](${SITE.url}/llms-full.txt): concatenated markdown of every page`,
    ].join("\n") + "\n"
  );
}

/**
 * Strip a markdown string to readable plain text: drop leading # markers,
 * list bullets, * and _ emphasis, standalone horizontal rules, and reduce
 * [text](url) links to their text. Extra blank lines are collapsed.
 */
export function mdToPlain(md: string): string {
  const lines = md.split("\n");
  const out: string[] = [];

  for (let line of lines) {
    // Standalone horizontal rules: --- / *** / ___
    if (/^\s*([-*_])\1{2,}\s*$/.test(line)) continue;

    // Headings: drop leading # markers (and trailing # closes).
    line = line.replace(/^(#{1,6})\s+(.*?)\s*#*\s*$/, "$2");

    // List bullets: unordered (-, *, +) and ordered (N.).
    line = line.replace(/^(\s*)(?:[-*+]|\d+\.)\s+/, "$1");

    // Links: [text](url) -> text.
    line = line.replace(/\[([^\]]+)\]\([^)]*\)/g, "$1");

    // Emphasis: bold then italic, ** and __ before single chars.
    line = line.replace(/\*\*([^*]+)\*\*/g, "$1");
    line = line.replace(/__([^_]+)__/g, "$1");
    line = line.replace(/(^|[^*])\*([^*\n]+)\*(?!\*)/g, "$1$2");
    line = line.replace(/(^|[^_\w])_([^_\n]+)_(?![\w])/g, "$1$2");

    out.push(line);
  }

  return (
    out
      .join("\n")
      .replace(/\n{3,}/g, "\n\n")
      .trim() + "\n"
  );
}

/** Route slug ↔ page path. `index` is the home page. */
export const pagePath = (slug: string): string => (slug === "index" ? "/" : `/${slug}`);

/** Prerender entries for the [slug].md / [slug].txt endpoints, derived from
 *  NAV_PAGES so adding a page to the config is enough. */
export const textVariantEntries = (): { slug: string }[] =>
  NAV_PAGES.map((p) => ({ slug: p.href === "/" ? "index" : p.href.replace(/^\//, "") }));
