import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const output = process.env.INDOPAK_REFERENCE_OUTPUT ?? "/tmp/easyquran-deep-reference";
const manifest = JSON.parse(
  await readFile(path.join(root, "scripts/fonts/indopak/mapping.json"), "utf8"),
);
const keys = new Set(
  manifest.entries.flatMap((entry) => entry.contexts.map((context) => context.verse_key)),
);
for (const key of [
  "16:6",
  "73:17",
  "79:27",
  "51:54",
  "26:51",
  "43:15",
  "3:187",
  "4:45",
  "28:25",
  "28:48",
  "29:32",
  "19:17",
  "35:2",
  "35:11",
  "6:46",
  "2:101",
])
  keys.add(key);
const queue = [...keys].sort((a, b) => {
  const [aSurah, aAyah] = a.split(":").map(Number);
  const [bSurah, bAyah] = b.split(":").map(Number);
  return aSurah - bSurah || aAyah - bAyah;
});
await mkdir(output, { recursive: true });
const browser = await chromium.launch();
const context = await browser.newContext();
const page = await context.newPage();
const verses = [];
const failures = [];
const builds = new Set();
try {
  await page.goto("https://quran.com/17/7");
  await page.locator("#__NEXT_DATA__").waitFor({ state: "attached" });
  const word = page.getByText("لِیَسُوْٓءٗا", { exact: true });
  await word.waitFor();
  const wordStyle = await word.evaluate((element) => ({
    family: getComputedStyle(element).fontFamily,
    class: element.className,
  }));
  const observed = await page.evaluate(() => ({
    fonts: performance
      .getEntriesByType("resource")
      .map((entry) => entry.name)
      .filter((url) => url.includes("indopak") && url.endsWith(".woff2")),
  }));
  observed.word = wordStyle;
  assert.ok(wordStyle.family.includes("IndoPak"));
  const fontUrl = observed.fonts[0];
  assert.ok(fontUrl, "No live IndoPak font resource observed");
  const fontResponse = await context.request.get(fontUrl);
  assert.equal(fontResponse.status(), 200);
  await writeFile(path.join(output, "qurancom.woff2"), await fontResponse.body());
  async function worker() {
    for (;;) {
      const key = queue.shift();
      if (!key) return;
      try {
        const response = await context.request.get(`https://quran.com/${key.replace(":", "/")}`, {
          timeout: 30000,
        });
        assert.equal(response.status(), 200, `${key}: reference page unavailable`);
        const html = await response.text();
        const match = /<script[^>]*id="__NEXT_DATA__"[^>]*>([\s\S]*?)<\/script>/.exec(html);
        assert.ok(match, `${key}: no page data`);
        const data = JSON.parse(match[1]);
        builds.add(data.buildId);
        const props = data.props.pageProps;
        assert.equal(
          props.__REDUX_STATE__.quranReaderStyles.quranFont,
          "text_indopak",
          `${key}: different reference font preference`,
        );
        const verse = props.versesResponse.verses.find((item) => item.verseKey === key);
        assert.ok(verse, `${key}: missing verse`);
        verses.push({
          key,
          words: verse.words.map((word) => ({
            location: word.location,
            legacy: word.textIndopak,
            display: word.text,
            type: word.charTypeName,
            css: word.cssClass ?? "",
          })),
        });
        if (verses.length % 25 === 0)
          console.log(`Reference pages checked: ${verses.length}/${keys.size}`);
      } catch (error) {
        failures.push({ key, error: error.message });
      }
    }
  }
  await Promise.all([worker(), worker()]);
  verses.sort((a, b) => a.key.localeCompare(b.key, "en", { numeric: true }));
  await writeFile(path.join(output, "verses.json"), JSON.stringify(verses, null, 2) + "\n");
  const report = {
    observed_at: new Date().toISOString(),
    pages_requested: keys.size,
    pages_checked: verses.length,
    failures,
    build_ids: [...builds],
    observed,
    font_url: fontUrl,
    font_bytes: (await fontResponse.body()).length,
    reference_assets_outside_repository: true,
  };
  await writeFile(path.join(output, "live-report.json"), JSON.stringify(report, null, 2) + "\n");
  console.log(JSON.stringify(report, null, 2));
  assert.equal(failures.length, 0);
} finally {
  await browser.close();
}
