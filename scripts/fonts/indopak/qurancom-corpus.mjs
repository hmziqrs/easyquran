import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";

import { root } from "./deep-browser-shared.mjs";

const run = promisify(execFile);
const output =
  process.env.INDOPAK_REFERENCE_OUTPUT ?? path.join(root, ".cache/indopak-v4/reference");
const queue = Array.from({ length: 114 }, (_, index) => index + 1);
const chapters = [];
const failures = [];
const requests = [];
await mkdir(output, { recursive: true });

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
        const url = new URL(`https://api.quran.com/api/v4/verses/by_chapter/${chapter}`);
        url.search = new URLSearchParams({
          words: "true",
          per_page: "300",
          fields: "text_indopak",
          word_fields: "text_indopak",
          mushaf: "3",
        });
        const { stdout } = await run(
          "curl",
          ["--fail", "--silent", "--show-error", "--max-time", "45", "--retry", "2", url.href],
          { maxBuffer: 32 * 1024 * 1024 },
        );
        data = JSON.parse(stdout);
        requests.push({ chapter, url: url.href, observed_at: new Date().toISOString() });
        await writeFile(filename, JSON.stringify(data) + "\n");
      }
      assert.equal(data.pagination.next_page, null, `Incomplete chapter ${chapter}`);
      assert.equal(data.verses.length, data.pagination.total_records);
      for (const verse of data.verses) {
        assert.ok(verse.verse_key.startsWith(`${chapter}:`));
        assert.ok(verse.words.length);
        for (const word of verse.words) assert.equal(typeof word.text_indopak, "string");
      }
      chapters.push(
        ...data.verses.map((verse) => ({
          key: verse.verse_key,
          legacy: verse.text_indopak,
          words: verse.words.map((word) => ({
            location: `${verse.verse_key}:${word.position}`,
            position: word.position,
            legacy: word.text_indopak,
            display: word.text,
            type: word.char_type_name,
          })),
        })),
      );
      if (chapters.length % 500 < data.verses.length)
        console.log(`Reference verses: ${chapters.length}/6236`);
    } catch (error) {
      failures.push({ chapter, error: String(error) });
    }
  }
}

await Promise.all([worker(), worker()]);
chapters.sort((a, b) => a.key.localeCompare(b.key, "en", { numeric: true }));
const report = {
  observed_at: new Date().toISOString(),
  source: "Quran.com public v4 API",
  mushaf: 3,
  endpoint: "https://api.quran.com/api/v4/verses/by_chapter",
  concurrent_requests_maximum: 2,
  verses: chapters.length,
  chapters: 114 - failures.length,
  failures,
  requests,
  served_text_pipeline:
    "API mushaf=3 word.text; current website word.text requires separate observation",
  reference_assets_ignored: true,
};
await writeFile(path.join(output, "verses.json"), JSON.stringify(chapters) + "\n");
await writeFile(path.join(output, "api-report.json"), JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify({ verses: report.verses, chapters: report.chapters, failures }));
assert.equal(failures.length, 0);
assert.equal(chapters.length, 6236);
