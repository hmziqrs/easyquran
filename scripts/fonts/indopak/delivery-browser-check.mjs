import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { chromium } from "playwright";
import { loadCorpus, root } from "./deep-browser-shared.mjs";

const inputs = process.env.INDOPAK_DEEP_OUTPUT ?? path.join(root, ".cache/indopak-deep");
const output = process.env.INDOPAK_DELIVERY_OUTPUT ?? path.join(inputs, "delivery");
const base = process.env.INDOPAK_READER_BASE ?? "http://127.0.0.1:5392";
const corpus = await loadCorpus(inputs);
const browser = await chromium.launch();
const results = [];
await mkdir(output, { recursive: true });

function snapshot() {
  return Array.from(
    document.querySelectorAll("[data-verse-key]:has([data-indopak-ayah])"),
    (row) => {
      const verse = row.querySelector("[data-indopak-ayah]");
      const text = verse.cloneNode(true);
      text.querySelector("[data-indopak-ornament]").remove();
      return {
        key: row.dataset.verseKey,
        text: text.textContent,
        visibility: getComputedStyle(verse).visibility,
        aria_hidden: verse.getAttribute("aria-hidden"),
      };
    },
  );
}

try {
  for (const scenario of ["slow", "woff2-failed", "both-failed-reload"]) {
    const context = await browser.newContext({
      serviceWorkers: "block",
      viewport: { width: 390, height: 844 },
    });
    await context.addInitScript(() =>
      localStorage.setItem(
        "easyquran.reader",
        JSON.stringify({ v: 4, arabicScript: "indopak", mode: "reading", fontSize: 33 }),
      ),
    );
    const page = await context.newPage();
    const errors = [];
    const requests = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("request", (request) => {
      if (request.url().includes("indopak-reader-compat-v4.")) requests.push(request.url());
    });
    let release;
    const held = new Promise((resolve) => {
      release = resolve;
    });
    if (scenario === "slow")
      await page.route("**/indopak-reader-compat-v4.woff2", async (route) => {
        await held;
        await route.continue();
      });
    if (scenario === "woff2-failed")
      await page.route("**/indopak-reader-compat-v4.woff2", (route) => route.abort());
    if (scenario === "both-failed-reload")
      await page.route("**/indopak-reader-compat-v4.*", (route) => route.abort());
    await page.goto(`${base}/al-fatihah`, { waitUntil: "domcontentloaded" });
    await page.waitForSelector("[data-indopak-ayah]", { state: "attached" });
    if (scenario === "slow") {
      await page.waitForFunction(() =>
        [...document.querySelectorAll("[data-indopak-ayah]")].every(
          (verse) => getComputedStyle(verse).visibility === "hidden",
        ),
      );
      await delay(3500);
      const initial = await page.evaluate(snapshot);
      assert.ok(initial.length > 0);
      assert.ok(
        initial.every((verse) => verse.visibility === "hidden" && verse.aria_hidden === "true"),
      );
      const client = await context.newCDPSession(page);
      const capture = await client.send("Page.captureScreenshot", { format: "png" });
      await writeFile(path.join(output, "slow-initial.png"), Buffer.from(capture.data, "base64"));
      await client.detach();
      results.push({ scenario: "slow-initial", held_ms: 3500, verses: initial });
      release();
    }
    if (scenario === "both-failed-reload") {
      await page.getByRole("button", { name: "Reload page", exact: true }).waitFor();
      const failed = await page.evaluate(snapshot);
      assert.ok(failed.length > 0);
      assert.ok(
        failed.every((verse) => verse.visibility === "hidden" && verse.aria_hidden === "true"),
      );
      await page.screenshot({ path: path.join(output, "both-failed.png") });
      results.push({ scenario: "both-failed", verses: failed });
      await page.unrouteAll();
      await page.getByRole("button", { name: "Reload page", exact: true }).click();
    }
    await page.waitForFunction(() =>
      [...document.querySelectorAll("[data-indopak-ayah]")].some(
        (verse) => getComputedStyle(verse).visibility === "visible",
      ),
    );
    const settled = await page.evaluate(snapshot);
    assert.ok(
      settled.every((verse) => verse.visibility === "visible" && verse.aria_hidden !== "true"),
    );
    for (const verse of settled) assert.equal(verse.text, corpus.originals[verse.key]);
    assert.deepEqual(errors, []);
    if (scenario === "woff2-failed") assert.ok(requests.some((url) => url.endsWith(".ttf")));
    const font = await page.request.get(`${base}/fonts/indopak-reader-compat-v4.woff2`);
    assert.equal(font.status(), 200);
    assert.ok(font.headers()["cache-control"].includes("immutable"));
    const packaged = await readFile(
      path.join(root, "web/static/fonts/indopak-reader-compat-v4.woff2"),
    );
    assert.ok((await font.body()).equals(packaged));
    await page.screenshot({ path: path.join(output, `${scenario}-settled.png`) });
    results.push({
      scenario,
      status: "passed",
      requests,
      errors,
      settled,
      font_bytes: packaged.length,
      font_byte_equality: true,
      cache_control: font.headers()["cache-control"],
    });
    console.log(`${scenario}: delivery passed`);
    await context.close();
  }
  await writeFile(
    path.join(output, "report.json"),
    JSON.stringify({ status: "passed", results }, null, 2) + "\n",
  );
} catch (error) {
  await writeFile(
    path.join(output, "report.json"),
    JSON.stringify({ status: "failed", error: String(error.stack ?? error), results }, null, 2) +
      "\n",
  );
  throw error;
} finally {
  await browser.close();
}
