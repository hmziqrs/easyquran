import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { pathToFileURL } from "node:url";
import { chromium, webkit } from "playwright";
import { installFirebaseAuditFixture } from "./firebase-audit-fixture.mjs";
import { loadCorpus, root } from "./deep-browser-shared.mjs";

const inputs = process.env.INDOPAK_DEEP_OUTPUT ?? path.join(root, ".cache/indopak-deep");
const output = process.env.INDOPAK_INTERACTION_OUTPUT ?? path.join(inputs, "interactions");
const base = process.env.INDOPAK_READER_BASE ?? "http://localhost:5392";
const engine = process.env.INDOPAK_READER_ENGINE ?? "chromium";
const corpus = await loadCorpus(inputs);
const translationUrl = pathToFileURL(
  path.join(root, "db/quran/translations/sqlite/en.sahih.sqlite"),
);
translationUrl.search = "?mode=ro&immutable=1";
const database = new DatabaseSync(translationUrl, { readOnly: true });
const translationOriginals = Object.fromEntries(
  database
    .prepare("SELECT sura, aya, text FROM quran_text")
    .all()
    .map((row) => [`${row.sura}:${row.aya}`, row.text]),
);
database.close();
const implementation = { chromium, webkit }[engine];
assert.ok(implementation);
const browser = await implementation.launch();
const report = {
  engine,
  version: browser.version(),
  status: "incomplete",
  conditions: {
    viewport: [390, 844],
    service_workers: "blocked",
    firebase_installations: "local installation fixture",
    firebase_web_config: "packaged public configuration fixture",
    analytics_script: "empty local fixture",
  },
  results: [],
  picker: [],
  failed_requests: [],
  console_errors: [],
  errors: [],
};
await mkdir(output, { recursive: true });
let page;
try {
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    serviceWorkers: "block",
  });
  report.firebase_fixture_requests = await installFirebaseAuditFixture(context);
  context.on("requestfailed", (request) =>
    report.failed_requests.push({ url: request.url(), error: request.failure()?.errorText }),
  );
  await context.addInitScript(() => {
    if (!localStorage.getItem("easyquran.reader"))
      localStorage.setItem(
        "easyquran.reader",
        JSON.stringify({ v: 4, arabicScript: "indopak", mode: "verse", fontSize: 33 }),
      );
  });
  page = await context.newPage();
  page.on("console", (message) => {
    if (message.type() === "error") report.console_errors.push(message.text());
  });
  page.setDefaultTimeout(120_000);
  page.setDefaultNavigationTimeout(120_000);
  page.on("pageerror", (error) => report.errors.push({ url: page.url(), message: error.message }));
  async function ready() {
    await page.waitForFunction(() => {
      const verse = document.querySelector("[data-indopak-ayah]");
      return !!verse && getComputedStyle(verse).visibility === "visible";
    });
    await page.evaluate(() => document.fonts.ready);
    await page.waitForLoadState("networkidle");
  }
  for (const test of [
    { query: "الحمد", source: "arabic", label: "Arabic", translation: false },
    { query: "mercy", source: "en.sahih", label: "Saheeh International", translation: true },
  ]) {
    let response;
    if (!test.translation)
      response = page.waitForResponse((item) => {
        const url = new URL(item.url());
        return (
          url.pathname.endsWith("/search") &&
          url.searchParams.get("q") === test.query &&
          item.headers()["content-type"]?.includes("application/json")
        );
      });
    const url = new URL("/search", base);
    url.searchParams.set("q", test.query);
    if (test.translation) url.searchParams.set("t", test.source);
    const [, api] = await Promise.all([page.goto(url.href), response]);
    if (test.translation) {
      await page.waitForFunction(
        (query) => document.querySelector("#search-query")?.value === query,
        test.query,
      );
      await page.getByRole("button", { name: /^Translations/u }).click();
      const row = page.locator("li").filter({ has: page.locator("#search-t-en\\.sahih") });
      await row.waitFor();
      report.picker.push({
        source: test.source,
        text: await row.innerText(),
        buttons: await row.getByRole("button").allTextContents(),
      });
      await page.screenshot({ path: path.join(output, `${engine}-translation-picker.png`) });
      console.log(`${engine}: translation picker ${JSON.stringify(report.picker.at(-1))}`);
      await row.getByRole("button", { name: "Download", exact: true }).click();
      if (process.env.INDOPAK_SEARCH_WORKER_DEBUG === "1") {
        const workerState = await page.evaluate(async () => {
          const { quranWorker } = await import("/src/lib/quran/worker-client.ts");
          const { quran } = await import("/src/lib/stores/quran.svelte.ts");
          return { ready: quranWorker.ready, status: quran.status, error: quran.error };
        });
        console.log(`${engine}: worker ${JSON.stringify(workerState)}`);
        report.worker_state = workerState;
      }
      await row.getByText(/^Downloaded ·/u).waitFor();
      await page.getByRole("button", { name: /^Translations/u }).click();
    }
    let source;
    if (api) {
      assert.equal(api.status(), 200);
      source = (await api.json()).data.results;
    }
    const section = page.locator(`section[aria-label="${test.label}"]`);
    await section.locator("a").first().waitFor();
    assert.equal(new URL(page.url()).searchParams.get("q"), test.query);
    const snippets = await section.locator("a").evaluateAll((links) =>
      links.map((link) => ({
        key: link.textContent.match(/\b\d{1,3}:\d{1,3}\b/u)?.[0],
        href: link.getAttribute("href"),
        text: link.nextElementSibling?.textContent,
      })),
    );
    assert.ok(snippets.length > 0);
    for (const snippet of snippets) {
      let expected;
      if (test.translation) expected = translationOriginals[snippet.key];
      else {
        const hit = source.find((item) => item.ayah?.key === snippet.key);
        assert.ok(hit, `API search hit ${snippet.key}`);
        expected = hit.ayah.text;
      }
      assert.equal(snippet.text, expected, `Search snippet ${snippet.key}`);
      if (test.translation) assert.ok(snippet.href.includes("/t/en/sahih"));
    }
    await page.waitForLoadState("networkidle");
    await section.locator("a").first().click();
    await ready();
    assert.equal(page.url().includes("/t/en/sahih"), test.translation);
    console.log(
      `${engine}: search navigation ${JSON.stringify(
        await page.evaluate(() => ({
          url: location.href,
          reader: JSON.parse(localStorage.getItem("easyquran.reader")),
          keys: [...document.querySelectorAll("[data-verse-key]")].map(
            (node) => node.dataset.verseKey,
          ),
        })),
      )}`,
    );
    const selected = page.locator(`[data-verse-key="${snippets[0].key}"] [data-indopak-ayah]`);
    await selected.waitFor();
    const readerText = await selected.evaluate((verse) => {
      const clone = verse.cloneNode(true);
      clone.querySelector("[data-indopak-ornament]")?.remove();
      return clone.textContent;
    });
    assert.equal(readerText, corpus.originals[snippets[0].key]);
    report.results.push({
      action: "search-snippets/navigation",
      source: test.source,
      exact_snippets: snippets.length,
      keys: snippets.map((item) => item.key),
      reader_source: "saved IndoPak preference",
      translation_downloaded_through_picker: test.translation,
    });
  }
  await page.goto(`${base}/al-fatihah?mode=verse`);
  await ready();
  const target = page.locator('[data-verse-key="1:7"]');
  await target.scrollIntoViewIfNeeded();
  await target.getByRole("button", { name: "Bookmark this verse", exact: true }).click();
  await page.goto(`${base}/bookmarks`);
  const saved = page.getByRole("link", { name: /Al-Fatihah.*7/u });
  await saved.waitFor();
  await saved.click();
  await ready();
  await target.scrollIntoViewIfNeeded();
  const text = await target.locator("[data-indopak-ayah]").evaluate((verse) => {
    const clone = verse.cloneNode(true);
    clone.querySelector("[data-indopak-ornament]").remove();
    return clone.textContent;
  });
  assert.equal(text, corpus.originals["1:7"]);
  report.results.push({ action: "bookmark-list/navigation", key: "1:7", source_exact: true });
  await page.screenshot({ path: path.join(output, `${engine}-bookmark-navigation.png`) });
  assert.deepEqual(report.errors, []);
  assert.deepEqual(report.console_errors, []);
  report.status = "passed_with_recorded_scope";
} catch (error) {
  if (page) {
    report.failed_url = page.url();
    await page
      .screenshot({ path: path.join(output, `${engine}-failed-state.png`) })
      .catch(() => {});
    report.failed_picker = await page
      .locator("li")
      .filter({ has: page.locator("#search-t-en\\.sahih") })
      .allTextContents()
      .catch(() => []);
  }
  report.error = String(error.stack ?? error);
  console.error(report.error);
  process.exitCode = 1;
} finally {
  await browser.close();
  await writeFile(
    path.join(output, `${engine}-search-bookmark-report.json`),
    JSON.stringify(report, null, 2) + "\n",
  );
}
