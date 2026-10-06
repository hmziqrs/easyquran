import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { chromium } from "playwright";
import { loadCorpus, root } from "./deep-browser-shared.mjs";

const inputs = process.env.INDOPAK_DEEP_OUTPUT ?? path.join(root, ".cache/indopak-deep");
const output = process.env.INDOPAK_DELIVERY_OUTPUT ?? path.join(inputs, "delivery");
const base = process.env.INDOPAK_READER_BASE ?? "http://127.0.0.1:5392";
const corpus = await loadCorpus(inputs);
const browser = await chromium.launch();
const context = await browser.newContext({ viewport: { width: 390, height: 844 } });
const report = { status: "failed", errors: [] };
await mkdir(output, { recursive: true });
await context.addInitScript(() => {
  localStorage.setItem(
    "easyquran.reader",
    JSON.stringify({ v: 4, arabicScript: "indopak", mode: "reading", fontSize: 33 }),
  );
});
const page = await context.newPage();
page.on("pageerror", (error) => report.errors.push(error.message));

function readerReady() {
  const verse = document.querySelector("[data-indopak-ayah]");
  return !!verse && getComputedStyle(verse).visibility === "visible";
}

try {
  await page.goto(`${base}/al-fatihah`, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(readerReady);
  await page.evaluate(() => navigator.serviceWorker.ready);
  await page.reload();
  await page.waitForFunction(() => !!navigator.serviceWorker.controller);
  await page.waitForFunction(readerReady);
  report.service_worker = await page.evaluate(() => ({
    script_url: navigator.serviceWorker.controller.scriptURL,
    state: navigator.serviceWorker.controller.state,
  }));
  report.precache = await page.evaluate(async () => {
    const names = (await caches.keys()).filter((name) => name.startsWith("eq-app-"));
    const result = [];
    for (const name of names) {
      const cache = await caches.open(name);
      const keys = await cache.keys();
      let bytes = 0;
      const fonts = [];
      for (const request of keys) {
        const response = await cache.match(request);
        const size = (await response.arrayBuffer()).byteLength;
        bytes += size;
        if (new URL(request.url).pathname.startsWith("/fonts/"))
          fonts.push({ path: new URL(request.url).pathname, bytes: size });
      }
      result.push({ name, entries: keys.length, bytes, fonts });
    }
    return result;
  });
  assert.ok(
    report.precache.some((cache) =>
      cache.fonts.some((font) => font.path === "/fonts/indopak-reader-compat-v4.woff2"),
    ),
  );
  await context.setOffline(true);
  await page.reload({ waitUntil: "domcontentloaded" });
  await page.waitForFunction(readerReady);
  const verses = await page.evaluate(() =>
    Array.from(document.querySelectorAll("[data-verse-key]:has([data-indopak-ayah])"), (row) => {
      const text = row.querySelector("[data-indopak-ayah]").cloneNode(true);
      text.querySelector("[data-indopak-ornament]").remove();
      return { key: row.dataset.verseKey, text: text.textContent };
    }),
  );
  assert.equal(verses.length, 7);
  for (const verse of verses) assert.equal(verse.text, corpus.originals[verse.key]);
  const font = await page.evaluate(async () => {
    const response = await fetch("/fonts/indopak-reader-compat-v4.woff2");
    const bytes = new Uint8Array(await response.arrayBuffer());
    return {
      status: response.status,
      base64: btoa(Array.from(bytes, (value) => String.fromCharCode(value)).join("")),
    };
  });
  assert.equal(font.status, 200);
  const packaged = await readFile(
    path.join(root, "web/static/fonts/indopak-reader-compat-v4.woff2"),
  );
  assert.ok(Buffer.from(font.base64, "base64").equals(packaged));
  await page.screenshot({ path: path.join(output, "offline-reader.png") });
  assert.deepEqual(report.errors, []);
  Object.assign(report, {
    status: "passed",
    offline: true,
    verses,
    font: { bytes: packaged.length, identical_to_package: true },
    scope: "Precached font and previously visited al-Fatihah reader reload",
  });
} catch (error) {
  report.error = String(error.stack ?? error);
  process.exitCode = 1;
} finally {
  await browser.close();
  await writeFile(path.join(output, "offline-report.json"), JSON.stringify(report, null, 2) + "\n");
}
console.log(
  JSON.stringify({
    status: report.status,
    error: report.error,
    precache: report.precache?.map(({ fonts, ...cache }) => ({
      ...cache,
      font_count: fonts.length,
    })),
  }),
);
