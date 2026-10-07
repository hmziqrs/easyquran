import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { _android } from "playwright";
import { loadCorpus, root } from "./deep-browser-shared.mjs";

const inputs = process.env.INDOPAK_DEEP_OUTPUT ?? path.join(root, ".cache/indopak-deep");
const output = process.env.INDOPAK_PERFORMANCE_OUTPUT ?? path.join(inputs, "performance");
const corpus = await loadCorpus(inputs);
const adb = process.env.INDOPAK_ADB ?? "/Users/hmziq/Library/Android/sdk/platform-tools/adb";
const [device] = await _android.devices({ omitDriverInstall: true });
assert.ok(device, "Android emulator unavailable");
function shell(...args) {
  return execFileSync(adb, ["-s", device.serial(), "shell", ...args], { encoding: "utf8" }).trim();
}
const report = {
  status: "incomplete",
  recorded_at: new Date().toISOString(),
  platform: "Android emulator; not physical mid-range hardware",
  device: {
    serial: device.serial(),
    model: shell("getprop", "ro.product.model"),
    android: shell("getprop", "ro.build.version.release"),
    sdk: shell("getprop", "ro.build.version.sdk"),
  },
  conditions: {
    cpu_throttle: 4,
    service_workers: "blocked",
    firebase_installations: "blocked",
    viewport: "actual emulator",
    scrolling: "programmatic",
    trials: 3,
  },
  runs: [],
};
const variants = [
  {
    id: "v3",
    commit: "104f049",
    app_commit: process.env.INDOPAK_PERFORMANCE_APP_COMMIT ?? "cabdcca",
    base: process.env.INDOPAK_PERFORMANCE_V3_BASE ?? "http://127.0.0.1:5394",
    font: "indopak-reader-compat-v3.woff2",
  },
  {
    id: "v4",
    commit: process.env.INDOPAK_PERFORMANCE_V4_COMMIT ?? "cabdcca",
    base: process.env.INDOPAK_READER_BASE ?? "http://127.0.0.1:5392",
    font: "indopak-reader-compat-v4.woff2",
  },
];
await mkdir(output, { recursive: true });
async function save() {
  await writeFile(
    path.join(output, "android-performance-report.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
}
function installObservers() {
  window.indopakPerformance = { long_tasks: [], shifts: [], errors: [] };
  new PerformanceObserver((list) => {
    for (const entry of list.getEntries())
      window.indopakPerformance.long_tasks.push({
        start: entry.startTime,
        duration: entry.duration,
      });
  }).observe({ type: "longtask", buffered: true });
  new PerformanceObserver((list) => {
    for (const entry of list.getEntries())
      window.indopakPerformance.shifts.push({
        value: entry.value,
        recent_input: entry.hadRecentInput,
        start: entry.startTime,
      });
  }).observe({ type: "layout-shift", buffered: true });
  window.addEventListener("error", (event) => window.indopakPerformance.errors.push(event.message));
  window.addEventListener("unhandledrejection", (event) =>
    window.indopakPerformance.errors.push(String(event.reason)),
  );
  localStorage.setItem(
    "easyquran.reader",
    JSON.stringify({ v: 4, arabicScript: "indopak", mode: "reading", fontSize: 33 }),
  );
}
function cumulativeLayoutShift(shifts) {
  let maximum = 0;
  let total = 0;
  let first = 0;
  let last = 0;
  for (const shift of shifts) {
    if (shift.recent_input) continue;
    if (shift.start - last > 1000 || shift.start - first > 5000) {
      total = 0;
      first = shift.start;
    }
    total += shift.value;
    maximum = Math.max(maximum, total);
    last = shift.start;
  }
  return maximum;
}
let context;
try {
  for (const variant of variants) {
    console.log(`${variant.id}: launching browser`);
    context = await device.launchBrowser({ viewport: null, serviceWorkers: "block" });
    report.chrome = shell("dumpsys", "package", "com.android.chrome").match(
      /versionName=(\S+)/u,
    )?.[1];
    await context.route("https://firebaseinstallations.googleapis.com/**", (route) =>
      route.abort(),
    );
    await context.addInitScript(installObservers);
    const page = await context.newPage();
    page.setDefaultTimeout(120_000);
    const cdp = await context.newCDPSession(page);
    console.log(`${variant.id}: CDP connected`);
    await cdp.send("Performance.enable");
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
    for (let trial = 0; trial < 3; trial += 1)
      for (const surah of [
        { number: 2, slug: "al-baqarah", count: 286 },
        { number: 26, slug: "ash-shuara", count: 227 },
      ]) {
        const started = Date.now();
        console.log(`${variant.id}: loading ${surah.slug}, trial ${trial}`);
        const response = await page.goto(`${variant.base}/${surah.slug}?mode=reading`, {
          waitUntil: "domcontentloaded",
          timeout: 120_000,
        });
        assert.equal(response.status(), 200);
        const before = Object.fromEntries(
          (await cdp.send("Performance.getMetrics")).metrics.map((metric) => [
            metric.name,
            metric.value,
          ]),
        );
        await page.waitForFunction(() => {
          const verse = document.querySelector("[data-indopak-ayah]");
          return (
            !!verse &&
            getComputedStyle(verse).visibility === "visible" &&
            document.fonts.check('33px "IndoPak Reader Compat"')
          );
        });
        await page.evaluate(() => document.fonts.ready);
        console.log(`${variant.id}: font ready`);
        const rewindDeadline = Date.now() + 15_000;
        while (!(await page.locator(`[data-verse-key="${surah.number}:1"]`).count())) {
          assert.ok(
            Date.now() < rewindDeadline,
            `Cannot reach first verse ${variant.id} ${surah.number}`,
          );
          await page.evaluate(() => window.scrollTo(0, 0));
          await page.waitForTimeout(250);
        }
        const readyMs = Date.now() - started;
        const seen = new Set();
        const errors = [];
        let maxSpans = 0;
        let steps = 0;
        let previousCount = 0;
        let progressedAt = Date.now();
        while (seen.size < surah.count) {
          const state = await page.evaluate(() => ({
            rows: [...document.querySelectorAll("[data-verse-key]:has([data-indopak-ayah])")].map(
              (row) => {
                const clone = row.querySelector("[data-indopak-ayah]").cloneNode(true);
                clone.querySelector("[data-indopak-ornament]").remove();
                return { key: row.dataset.verseKey, text: clone.textContent };
              },
            ),
            spans: document.querySelectorAll("[data-indopak-ayah] span").length,
          }));
          maxSpans = Math.max(maxSpans, state.spans);
          for (const row of state.rows) {
            assert.equal(
              row.text,
              corpus.originals[row.key],
              `Performance text ${variant.id} ${row.key}`,
            );
            seen.add(row.key);
          }
          if (seen.size === surah.count) break;
          if (seen.size !== previousCount) progressedAt = Date.now();
          previousCount = seen.size;
          if (Date.now() - progressedAt >= 60_000)
            report.stall = {
              variant: variant.id,
              surah: surah.number,
              missing: Array.from(
                { length: surah.count },
                (_, index) => `${surah.number}:${index + 1}`,
              ).filter((key) => !seen.has(key)),
            };
          assert.ok(
            Date.now() - progressedAt < 60_000,
            `Stalled ${variant.id} ${surah.number}: ${seen.size}/${surah.count}`,
          );
          await page.evaluate(() => window.scrollBy(0, innerHeight * 0.6));
          await page.waitForTimeout(80);
          steps += 1;
          if (steps % 25 === 0)
            console.log(`${variant.id}: ${surah.number}, seen ${seen.size}/${surah.count}`);
          assert.ok(steps < 6000, `Stalled ${variant.id} ${surah.number}`);
        }
        await page.waitForTimeout(500);
        const metrics = await cdp.send("Performance.getMetrics");
        const observed = await page.evaluate(() => ({
          ...window.indopakPerformance,
          viewport: [innerWidth, innerHeight],
          dpr: devicePixelRatio,
          resources: performance
            .getEntriesByType("resource")
            .filter((entry) => entry.name.includes("indopak-reader-compat"))
            .map((entry) => ({
              url: entry.name,
              transfer_size: entry.transferSize,
              encoded_size: entry.encodedBodySize,
              duration: entry.duration,
            })),
        }));
        errors.push(...observed.errors);
        const values = Object.fromEntries(
          metrics.metrics.map((metric) => [metric.name, metric.value]),
        );
        const deltas = Object.fromEntries(
          ["ScriptDuration", "LayoutDuration", "RecalcStyleDuration", "TaskDuration"].map(
            (name) => [name, values[name] - before[name]],
          ),
        );
        assert.ok(
          Object.values(deltas).every((value) => value >= 0),
          "CDP counter reset inside measured document",
        );
        report.runs.push({
          variant,
          trial,
          surah: surah.number,
          verses_seen: seen.size,
          ready_ms: readyMs,
          total_ms: Date.now() - started,
          steps,
          maximum_mounted_spans: maxSpans,
          ...observed,
          cdp_metrics: values,
          cdp_duration_deltas: deltas,
          cdp_duration_window: "same document, DOMContentLoaded through final scroll",
          cls: cumulativeLayoutShift(observed.shifts),
          errors,
        });
        await save();
        console.log(
          `${variant.id}: trial ${trial} surah ${surah.number}, ${seen.size} verses, ${observed.long_tasks.length} long tasks`,
        );
      }
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: 1 });
    await context.close();
    context = undefined;
  }
  report.status = "measured";
} catch (error) {
  report.error = String(error.stack ?? error);
  console.error(report.error);
  process.exitCode = 1;
} finally {
  await save();
  if (context) await context.close();
  await device.close();
  await save();
}
