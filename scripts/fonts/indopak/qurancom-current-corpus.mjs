import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

import { chromium } from "playwright";

import { root } from "./deep-browser-shared.mjs";

const output =
  process.env.INDOPAK_REFERENCE_OUTPUT ?? path.join(root, ".cache/indopak-v4/reference-current");
const queue = Array.from({ length: 114 }, (_, index) => index + 1);
const chapters = [];
const failures = [];
const requests = [];
await mkdir(output, { recursive: true });
const browser = await chromium.launch();
const page = await browser.newPage({ serviceWorkers: "block" });
try {
  await page.goto("https://quran.com/17/7");
  const initial = JSON.parse(await page.locator("#__NEXT_DATA__").textContent());
  const styles = initial.props.pageProps.__REDUX_STATE__.quranReaderStyles;
  assert.equal(styles.quranFont, "text_indopak");
  await page.getByText("لِیَسُوْٓءٗا", { exact: true }).waitFor();
  await page.evaluate(() => document.fonts.ready);
  const fonts = await page.evaluate(() =>
    performance
      .getEntriesByType("resource")
      .map((entry) => entry.name)
      .filter((url) => url.includes("indopak") && url.endsWith(".woff2")),
  );
  assert.ok(fonts.length);
  async function worker() {
    for (;;) {
      const chapter = queue.shift();
      if (!chapter) return;
      const filename = path.join(output, `chapter-${chapter}.json`);
      try {
        let data;
        try {
          data = JSON.parse(await readFile(filename, "utf8"));
        } catch {
          const url = new URL(
            `https://quran.com/api/proxy/content/api/qdc/verses/by_chapter/${chapter}`,
          );
          url.search = new URLSearchParams({
            words: "true",
            per_page: "300",
            fields: "text_indopak",
            word_fields: "verse_key,verse_id,page_number,location,text_indopak",
            word_translation_language: "ur",
            mushaf: "6",
          });
          const result = await page.evaluate(async (pathname) => {
            const response = await fetch(pathname);
            if (!response.ok) throw new Error(`Reference HTTP ${response.status}`);
            return response.json();
          }, url.pathname + url.search);
          data = result;
          requests.push({ chapter, url: url.href, observed_at: new Date().toISOString() });
          await writeFile(filename, JSON.stringify(data) + "\n");
        }
        assert.equal(data.pagination.next_page, null, `Incomplete current chapter ${chapter}`);
        assert.equal(data.verses.length, data.pagination.total_records);
        for (const verse of data.verses) {
          assert.ok(verse.verse_key.startsWith(`${chapter}:`));
          for (const word of verse.words) {
            assert.equal(typeof word.text_indopak, "string");
            assert.equal(typeof word.text, "string");
          }
        }
        chapters.push(
          ...data.verses.map((verse) => ({
            key: verse.verse_key,
            legacy: verse.text_indopak,
            words: verse.words.map((word) => ({
              location: word.location ?? `${verse.verse_key}:${word.position}`,
              position: word.position,
              legacy: word.text_indopak,
              display: word.text,
              type: word.char_type_name,
              css: word.css_class ?? "",
            })),
          })),
        );
        console.log(
          `Current reference chapters: ${114 - queue.length}/114; verses ${chapters.length}`,
        );
      } catch (error) {
        failures.push({ chapter, error: String(error.stack) });
      }
    }
  }
  await Promise.all([worker(), worker()]);
  chapters.sort((a, b) => a.key.localeCompare(b.key, "en", { numeric: true }));
  const report = {
    source: "Observed Quran.com website content proxy",
    observed_at: new Date().toISOString(),
    build_id: initial.buildId,
    font_urls: fonts,
    styles,
    mushaf: 6,
    concurrent_requests_maximum: 2,
    verses: chapters.length,
    chapters: 114 - failures.length,
    failures,
    requests,
    served_text_pipeline:
      "Current website word.text; proxy and mushaf observed from live chapter navigation",
    reference_assets_ignored: true,
  };
  await writeFile(path.join(output, "verses.json"), JSON.stringify(chapters) + "\n");
  await writeFile(path.join(output, "api-report.json"), JSON.stringify(report, null, 2) + "\n");
  console.log(JSON.stringify({ verses: report.verses, chapters: report.chapters, failures }));
  assert.equal(failures.length, 0);
  assert.equal(chapters.length, 6236);
} finally {
  await browser.close();
}
