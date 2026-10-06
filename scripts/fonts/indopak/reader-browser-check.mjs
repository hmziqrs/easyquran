import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";

import { chromium, webkit } from "playwright";

import { assertSpecimens, inspectSpecimens, loadCorpus, root } from "./deep-browser-shared.mjs";
import { Session, freePort } from "./safari-native-check.mjs";

const output = process.env.INDOPAK_DEEP_OUTPUT ?? path.join(root, ".cache/indopak-deep");
const destination = process.env.INDOPAK_READER_OUTPUT ?? output;
const base = process.env.INDOPAK_READER_BASE ?? "http://127.0.0.1:5392";
const engine = process.env.INDOPAK_READER_ENGINE ?? "chromium";
const blockTelemetry = process.env.INDOPAK_READER_BLOCK_TELEMETRY === "1";
const modes = (process.env.INDOPAK_READER_MODES ?? "reading,verse").split(",");
const size = Number(process.env.INDOPAK_READER_SIZE ?? 33);
const corpus = await loadCorpus(output);
const catalog = JSON.parse(
  await readFile(path.join(root, "web/static/quran-meta/quran-data.json"), "utf8"),
);
const selected = process.env.INDOPAK_READER_SURAHS?.split(",").map(Number);
const surahs = catalog[1]
  .map((row, index) => ({ number: index + 1, slug: row[0], count: row[6] }))
  .filter((surah) => !selected || selected.includes(surah.number));
const records = [];
const errors = [];
const windowErrors = [];
let browser;
let page;
let session;
let driver;
let version;
await mkdir(destination, { recursive: true });

function installReaderErrors() {
  function record(message) {
    const key = "indopak-reader-errors";
    const values = JSON.parse(sessionStorage.getItem(key) ?? "[]");
    values.push({ message, url: location.href });
    sessionStorage.setItem(key, JSON.stringify(values));
  }
  window.addEventListener("error", (event) => record(event.message));
  window.addEventListener("unhandledrejection", (event) => record(String(event.reason)));
}

async function evaluate(fn, arg) {
  if (session) return session.executeAsync(fn, arg);
  return page.evaluate(fn, arg);
}

async function navigate(url) {
  if (session) {
    await session.navigate(url);
    await session.execute(installReaderErrors);
    return;
  }
  const response = await page.goto(url, { waitUntil: "domcontentloaded", timeout: 120_000 });
  assert.equal(response.status(), 200);
}

async function screenshot(filename) {
  if (!session) return page.screenshot({ path: path.join(destination, filename) });
  const png = await session.command("GET", "/screenshot");
  await writeFile(path.join(destination, filename), Buffer.from(png, "base64"));
}

function ready() {
  const verse = document.querySelector("[data-indopak-ayah]");
  return (
    !!verse &&
    getComputedStyle(verse).visibility === "visible" &&
    document.fonts.check('33px "IndoPak Reader Compat"')
  );
}

async function waitFor(fn, arg, timeout = 120_000) {
  if (!session) {
    await page.waitForFunction(fn, arg, { timeout });
    return;
  }
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await evaluate(fn, arg)) return;
    await delay(100);
  }
  throw new Error(`Timeout waiting for ${fn.name}`);
}

function scrollStep() {
  const before = window.scrollY;
  window.scrollBy(0, window.innerHeight * 0.6);
  if (Math.abs(window.scrollY - before) < 1)
    window.dispatchEvent(new WheelEvent("wheel", { deltaY: window.innerHeight }));
  return new Promise((resolve) =>
    setTimeout(
      () =>
        resolve({
          y: window.scrollY,
          height: document.documentElement.scrollHeight,
          viewport: [innerWidth, innerHeight],
          keys: Array.from(
            document.querySelectorAll("[data-verse-key]:has([data-indopak-ayah])"),
            (element) => element.dataset.verseKey,
          ),
        }),
      50,
    ),
  );
}

try {
  if (engine === "safari") {
    assert.equal(blockTelemetry, false, "Native Safari cannot intercept telemetry requests");
    const port = await freePort();
    driver = spawn("/usr/bin/safaridriver", ["--port", String(port)], { stdio: "ignore" });
    session = new Session(`http://127.0.0.1:${port}`);
    for (let attempt = 0; attempt < 100; attempt += 1) {
      try {
        await session.raw("GET", "/status");
        break;
      } catch {
        await delay(100);
      }
    }
    version = await session.start();
    await session.setViewport({ width: 390, height: 844 });
    await navigate(`${base}/al-fatihah`);
  } else {
    const implementation = { chromium, webkit }[engine];
    assert.ok(implementation, `Unsupported reader engine ${engine}`);
    browser = await implementation.launch();
    version = browser.version();
    page = await browser.newPage({
      viewport: { width: 390, height: 844 },
      serviceWorkers: "block",
    });
    if (blockTelemetry)
      await page.route("https://firebaseinstallations.googleapis.com/**", (route) => route.abort());
    await page.addInitScript(installReaderErrors);
    await page.addInitScript(
      ({ mode, size }) => {
        if (localStorage.getItem("easyquran.reader")) return;
        localStorage.setItem(
          "easyquran.reader",
          JSON.stringify({ v: 4, arabicScript: "indopak", mode, fontSize: size }),
        );
      },
      { mode: modes[0], size },
    );
    page.on("pageerror", (error) =>
      errors.push({ message: error.message, url: page.url(), stack: error.stack }),
    );
    await navigate(`${base}/al-fatihah`);
    await waitFor(ready);
  }
  for (const mode of modes) {
    await evaluate(
      ({ mode, size }) => {
        localStorage.setItem(
          "easyquran.reader",
          JSON.stringify({ v: 4, arabicScript: "indopak", mode, fontSize: size }),
        );
      },
      { mode, size },
    );
    for (const surah of surahs) {
      await navigate(`${base}/${surah.slug}?mode=${mode}`);
      await waitFor(ready);
      await evaluate(() => document.fonts.ready);
      const seen = new Set();
      const privateSeen = new Set();
      const endRowsSeen = new Set();
      let previous;
      let stalledAt = Date.now();
      let steps = 0;
      let maximumMounted = 0;
      const first = `${engine}-reader-${mode}-${surah.number}-first.png`;
      const last = `${engine}-reader-${mode}-${surah.number}-last.png`;
      await screenshot(first);
      while (seen.size < surah.count) {
        const specimens = await evaluate(inspectSpecimens, {
          reader: true,
          requireInk: true,
          fontReference: corpus.fontReference,
        });
        maximumMounted = Math.max(maximumMounted, specimens.length);
        assertSpecimens(specimens, corpus, false, true, true);
        for (const specimen of specimens) {
          assert.ok(specimen.key.startsWith(`${surah.number}:`));
          assert.deepEqual(specimen.clipping, [], `Clipped reader ${specimen.key}`);
          seen.add(specimen.key);
          if (specimen.privateCount) privateSeen.add(specimen.key);
          if (specimen.end_rows.length) endRowsSeen.add(specimen.key);
        }
        if (seen.size === surah.count) break;
        const state = await evaluate(scrollStep);
        const progress = `${seen.size}|${state.y}|${state.height}|${state.keys.join(",")}`;
        if (progress !== previous) stalledAt = Date.now();
        assert.ok(
          Date.now() - stalledAt < 60_000,
          `Stalled ${surah.number} ${mode}: ${seen.size}/${surah.count}`,
        );
        previous = progress;
        steps += 1;
        assert.ok(steps < 8000, `Excessive scroll steps ${surah.number}`);
      }
      for (let n = 1; n <= surah.count; n += 1) assert.ok(seen.has(`${surah.number}:${n}`));
      await evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
      await screenshot(last);
      windowErrors.push(
        ...(await evaluate(() => {
          const key = "indopak-reader-errors";
          const values = JSON.parse(sessionStorage.getItem(key) ?? "[]");
          sessionStorage.removeItem(key);
          return values;
        })),
      );
      records.push({
        ...surah,
        mode,
        size,
        verses: seen.size,
        private_verses: privateSeen.size,
        end_stack_verses: endRowsSeen.size,
        maximum_mounted: maximumMounted,
        scroll_steps: steps,
        clipping_failures: 0,
        exact_text_failures: 0,
        screenshots: [first, last],
      });
      await writeFile(
        path.join(destination, `${engine}-reader-progress.json`),
        JSON.stringify(records, null, 2) + "\n",
      );
      console.log(`${engine}: reader ${mode} ${surah.number}/114, ${seen.size} verses`);
    }
  }
  assert.deepEqual(errors, []);
  assert.deepEqual(windowErrors, []);
  await writeFile(
    path.join(destination, `${engine}-reader-report.json`),
    JSON.stringify(
      {
        engine,
        version,
        status: "passed",
        matrix_size: size,
        modes,
        surahs_per_mode: surahs.length,
        verses_per_mode: surahs.reduce((total, surah) => total + surah.count, 0),
        database_id: "quran-indopak",
        database_open_mode: "ro&immutable=1",
        programmatic_scroll: true,
        errors,
        window_errors: windowErrors,
        firebase_installations_blocked: blockTelemetry,
        results: records,
      },
      null,
      2,
    ) + "\n",
  );
} catch (error) {
  await writeFile(
    path.join(destination, `${engine}-reader-report.json`),
    JSON.stringify(
      {
        engine,
        version,
        status: "failed",
        error: String(error.stack),
        errors,
        window_errors: windowErrors,
        firebase_installations_blocked: blockTelemetry,
        results: records,
      },
      null,
      2,
    ) + "\n",
  );
  throw error;
} finally {
  await browser?.close();
  await session?.stop();
  driver?.kill();
}
