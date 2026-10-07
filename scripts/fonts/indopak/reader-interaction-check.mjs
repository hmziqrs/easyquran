import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { pathToFileURL } from "node:url";
import { chromium, webkit } from "playwright";
import { installFirebaseAuditFixture } from "./firebase-audit-fixture.mjs";
import { assertSpecimens, inspectSpecimens, loadCorpus, root } from "./deep-browser-shared.mjs";

const inputs = process.env.INDOPAK_DEEP_OUTPUT ?? path.join(root, ".cache/indopak-deep");
const output = process.env.INDOPAK_INTERACTION_OUTPUT ?? path.join(inputs, "interactions");
const base = process.env.INDOPAK_READER_BASE ?? "http://localhost:5392";
const engine = process.env.INDOPAK_READER_ENGINE ?? "chromium";
const actionsOnly = process.env.INDOPAK_INTERACTIONS_ONLY === "1";
const configFixture = process.env.INDOPAK_INTERACTION_CONFIG_FIXTURE === "1";
const corpus = await loadCorpus(inputs);
const translationUrl = pathToFileURL(
  path.join(root, "db/quran/translations/sqlite/en.sahih.sqlite"),
);
translationUrl.search = "?mode=ro&immutable=1";
const translationDatabase = new DatabaseSync(translationUrl, { readOnly: true });
const translationOriginals = Object.fromEntries(
  translationDatabase
    .prepare("SELECT sura, aya, text FROM quran_text")
    .all()
    .map((row) => [`${row.sura}:${row.aya}`, row.text]),
);
translationDatabase.close();
const implementation = { chromium, webkit }[engine];
assert.ok(implementation, `Unsupported interaction engine ${engine}`);
const browser = await implementation.launch();
const report = {
  engine,
  version: browser.version(),
  status: "incomplete",
  conditions: {
    viewport: [390, 844],
    service_workers: "blocked",
    firebase_installations: configFixture ? "local installation fixture" : "blocked",
    firebase_web_config: configFixture
      ? "packaged public configuration fixture"
      : "unmodified network",
    analytics_script: configFixture ? "empty local fixture" : "unmodified network",
    scroll: "programmatic",
    route_matrix: actionsOnly ? "skipped" : "48 states",
  },
  matrices: [],
  interactions: [],
  errors: [],
  console_errors: [],
  network_failures: [],
  firebase_fixture_requests: [],
};
await mkdir(output, { recursive: true });
async function save() {
  await writeFile(
    path.join(output, `${engine}-interaction-report.json`),
    JSON.stringify(report, null, 2) + "\n",
  );
}
async function contextFor(mode, size, theme) {
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    serviceWorkers: "block",
  });
  if (configFixture) {
    const events = await installFirebaseAuditFixture(context);
    report.firebase_fixture_requests.push(events);
  } else
    await context.route("https://firebaseinstallations.googleapis.com/**", (route) =>
      route.abort(),
    );
  await context.addInitScript(
    ({ mode, size, theme }) => {
      if (!localStorage.getItem("easyquran.reader"))
        localStorage.setItem(
          "easyquran.reader",
          JSON.stringify({ v: 4, arabicScript: "indopak", mode, fontSize: size }),
        );
      if (!localStorage.getItem("easyquran.prefs"))
        localStorage.setItem(
          "easyquran.prefs",
          JSON.stringify({ v: 1, mode: theme, palette: "sacred" }),
        );
    },
    { mode, size, theme },
  );
  const page = await context.newPage();
  page.setDefaultTimeout(120_000);
  page.on("pageerror", (error) =>
    report.errors.push({ url: page.url(), error: error.message, stack: error.stack }),
  );
  page.on("console", (message) => {
    if (message.type() === "error")
      report.console_errors.push({ url: page.url(), message: message.text() });
  });
  page.on("requestfailed", (request) =>
    report.network_failures.push({ url: request.url(), failure: request.failure()?.errorText }),
  );
  return { context, page };
}
async function ready(page) {
  await page.waitForFunction(() => {
    const verse = document.querySelector("[data-indopak-ayah]");
    return !!verse && getComputedStyle(verse).visibility === "visible";
  });
  await page.evaluate(() => document.fonts.ready);
  if (configFixture) await page.waitForLoadState("networkidle");
}
async function inspect(page) {
  const specimens = await page.evaluate(inspectSpecimens, {
    reader: true,
    requireInk: true,
    fontReference: corpus.fontReference,
  });
  assert.ok(specimens.length > 0);
  assertSpecimens(specimens, corpus, false, true, true);
  for (const specimen of specimens)
    assert.deepEqual(specimen.clipping, [], `Clipping ${specimen.key}`);
  return specimens;
}
async function inspectTranslation(page) {
  const rows = await page.locator("[data-verse-key]").evaluateAll((nodes) =>
    nodes.flatMap((row) => {
      const text = row.querySelector(
        ".verse-text--translation, .translation-text, .verse-extra:not(.verse-extra--error)",
      );
      if (!text) return [];
      const clone = text.cloneNode(true);
      const credit = !!clone.querySelector(".verse-extra-credit");
      clone.querySelector(".ayah-marker")?.remove();
      clone.querySelector(".verse-extra-credit")?.remove();
      return [{ key: row.dataset.verseKey, text: clone.textContent, credit }];
    }),
  );
  assert.ok(rows.length > 0);
  for (const row of rows) {
    const separator = row.credit ? " " : "";
    assert.equal(
      row.text,
      translationOriginals[row.key] + separator,
      `Translation text ${row.key}`,
    );
  }
  return rows;
}
try {
  const routes = actionsOnly
    ? []
    : [
        "/al-fatihah",
        "/page/1",
        "/juz/30",
        "/al-fatihah/t/en/sahih",
        "/t/en/sahih/page/1",
        "/t/en/sahih/juz/30",
      ];
  for (const mode of ["reading", "verse"])
    for (const size of [22, 56])
      for (const theme of ["light", "dark"]) {
        const { context, page } = await contextFor(mode, size, theme);
        for (const route of routes) {
          const response = await page.goto(`${base}${route}?mode=${mode}`, {
            waitUntil: "domcontentloaded",
          });
          assert.equal(response.status(), 200);
          const translated = route.includes("/t/");
          if (route.startsWith("/al-fatihah"))
            await page.getByRole("button", { name: /Ayah-by-Ayah|Ayahs/ }).waitFor();
          if (translated)
            await page
              .locator(
                ".verse-text--translation,.translation-text,.verse-extra:not(.verse-extra--error)",
              )
              .first()
              .waitFor();
          else await ready(page);
          assert.equal(await page.evaluate(() => document.documentElement.dataset.mode), theme);
          const seen = new Set();
          for (const edge of ["first", "last"]) {
            if (edge === "last") {
              await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
              await page.waitForTimeout(250);
            }
            if (translated) for (const row of await inspectTranslation(page)) seen.add(row.key);
            const arabicCount = await page.locator("[data-indopak-ayah]").count();
            if (arabicCount) {
              await ready(page);
              for (const specimen of await inspect(page)) seen.add(specimen.key);
              const actualSize = await page
                .locator("[data-indopak-ayah]")
                .first()
                .evaluate((node) => Number.parseFloat(getComputedStyle(node).fontSize));
              assert.equal(actualSize, size);
            } else assert.ok(translated, "Arabic route must use saved IndoPak script");
            await page.screenshot({
              path: path.join(
                output,
                `${engine}-${mode}-${size}-${theme}-${route.replaceAll("/", "_")}-${edge}.png`,
              ),
            });
          }
          const links = await page
            .locator('a[aria-label^="Next"],a[aria-label^="Previous"]')
            .evaluateAll((nodes) => nodes.map((node) => node.getAttribute("href")));
          if (route.includes("/t/"))
            assert.ok(
              links.every(
                (href) => href.includes("/t/en/sahih") || href.startsWith("/t/en/sahih/"),
              ),
              `Translation context ${links}`,
            );
          report.matrices.push({
            route,
            mode,
            size,
            theme,
            visited_edges: ["first", "last"],
            inspected_keys: [...seen],
            arabic_companion_present: (await page.locator("[data-indopak-ayah]").count()) > 0,
            navigation_links: links,
          });
          await save();
          console.log(`${engine}: ${mode} ${size} ${theme} ${route}`);
        }
        await context.close();
      }
  const { context, page } = await contextFor("verse", 33, "light");
  await page.goto(`${base}/al-fatihah`);
  await ready(page);
  await page.goto(`${base}/settings#reading`);
  await page.getByRole("button", { name: "Uthmani", exact: true }).click();
  await page.goBack();
  await page.waitForFunction(
    () =>
      document.querySelector(".verse-text[lang=ar]") &&
      !document.querySelector("[data-indopak-ayah]"),
  );
  if (configFixture) await page.waitForLoadState("networkidle");
  await page.goto(`${base}/settings#reading`);
  await page.getByRole("button", { name: "IndoPak", exact: true }).click();
  await page.getByRole("button", { name: "Reading", exact: true }).click();
  await page.goBack();
  await ready(page);
  await page.reload();
  await ready(page);
  const persisted = await page.evaluate(() => JSON.parse(localStorage.getItem("easyquran.reader")));
  assert.equal(persisted.arabicScript, "indopak");
  assert.equal(persisted.mode, "verse");
  await page.goForward();
  await page.getByRole("button", { name: "IndoPak", exact: true }).waitFor();
  assert.equal(
    await page.getByRole("button", { name: "IndoPak", exact: true }).getAttribute("aria-pressed"),
    "true",
  );
  await page.goBack();
  await ready(page);
  await page.getByRole("button", { name: "Reading", exact: true }).click();
  await page.reload();
  await ready(page);
  assert.equal(
    await page.evaluate(() => JSON.parse(localStorage.getItem("easyquran.reader")).mode),
    "reading",
  );
  report.interactions.push({
    action: "script/settings/back/forward/mode-URL/reload",
    passed: true,
    persisted,
  });
  await page.getByRole("button", { name: /Ayah-by-Ayah|Ayahs/ }).click();
  const target = page.locator('[data-verse-key="1:7"]');
  await target.scrollIntoViewIfNeeded();
  await ready(page);
  const selected = await target.locator("[data-indopak-ayah]").evaluate((verse) => {
    const walker = document.createTreeWalker(verse, NodeFilter.SHOW_TEXT);
    const nodes = [];
    let node;
    while ((node = walker.nextNode()))
      if (!node.parentElement.closest("[data-indopak-ornament]") && node.textContent.length)
        nodes.push(node);
    const range = document.createRange();
    range.setStart(nodes[0], 0);
    range.setEnd(nodes.at(-1), nodes.at(-1).textContent.length);
    const selection = getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
    const event = new ClipboardEvent("copy", {
      bubbles: true,
      cancelable: true,
      clipboardData: new DataTransfer(),
    });
    verse.dispatchEvent(event);
    return {
      dom_source: range.toString(),
      rendered_selection: selection.toString(),
      copied: event.clipboardData.getData("text/plain"),
      handled: event.defaultPrevented,
    };
  });
  assert.equal(selected.dom_source, corpus.originals["1:7"]);
  assert.equal(selected.copied, corpus.originals["1:7"]);
  assert.equal(selected.handled, true);
  if (engine === "chromium") {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.keyboard.press("Meta+c");
    const clipboard = await page.evaluate(() => navigator.clipboard.readText());
    assert.equal(clipboard, corpus.originals["1:7"]);
    report.interactions.push({
      action: "OS-clipboard-selected-verse",
      key: "1:7",
      source_exact: true,
    });
  }
  await page.screenshot({ path: path.join(output, `${engine}-selection-1-7.png`) });
  const query = corpus.originals["1:7"].match(/^\S+\s+\S+/u)[0];
  const crossBoxFind = await page.evaluate((text) => {
    getSelection().removeAllRanges();
    return window.find(text, false, false, true);
  }, query);
  report.interactions.push({
    action: "selection/window.find",
    key: "1:7",
    selected,
    query,
    find_api: "window.find",
    cross_box_find: crossBoxFind,
    native_find_in_page: "requires separate browser UI verification",
    source_copy_passed: true,
  });
  await target.getByRole("button", { name: "Bookmark this verse", exact: true }).click();
  await target.getByRole("button", { name: "Remove bookmark", exact: true }).waitFor();
  await page.reload();
  await ready(page);
  await target.scrollIntoViewIfNeeded();
  await target.getByRole("button", { name: "Remove bookmark", exact: true }).waitFor();
  report.interactions.push({ action: "anonymous-bookmark/reload", key: "1:7", passed: true });
  if (engine === "chromium") {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    await target.getByRole("button", { name: "Copy ayah", exact: true }).click();
    const clipboard = await page.evaluate(() => navigator.clipboard.readText());
    assert.equal(clipboard.split("\n")[0], corpus.originals["1:7"]);
    report.interactions.push({
      action: "OS-clipboard-copy",
      key: "1:7",
      clipboard,
      source_line_exact: true,
      reference_line_expected: true,
    });
  } else
    report.interactions.push({
      action: "OS-clipboard-copy",
      status: "untested",
      reason: "Playwright WebKit does not grant clipboard-read permission",
    });
  await target.evaluate((row) => row.scrollIntoView({ block: "center" }));
  await page.waitForTimeout(350);
  await target.locator(".ayah-ornament").hover();
  const tooltip = page.locator("[data-tooltip-content]:visible");
  await tooltip.waitFor();
  report.interactions.push({
    action: "ring-tooltip",
    key: "1:7",
    text: await tooltip.innerText(),
    lang: await target.locator(".ayah-ornament").getAttribute("lang"),
  });
  assert.equal(report.interactions.at(-1).lang, "ur");
  assert.ok(report.interactions.at(-1).text.includes("1:7"));
  await context.close();
  assert.deepEqual(report.errors, []);
  if (configFixture) assert.deepEqual(report.console_errors, []);
  report.status = "passed_with_recorded_scope";
} catch (error) {
  report.error = String(error.stack ?? error);
  process.exitCode = 1;
} finally {
  await browser.close();
  await save();
}
