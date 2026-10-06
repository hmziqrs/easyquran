import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

import { _android } from "playwright";

import {
  addFont,
  assertSpecimens,
  controlsReady,
  diagnosticFont,
  inspectSpecimens,
  layoutOverflow,
  loadCorpus,
  root,
} from "./deep-browser-shared.mjs";

const output = process.env.INDOPAK_DEEP_OUTPUT ?? path.join(root, ".cache/indopak-deep");
const destination = process.env.INDOPAK_MOBILE_OUTPUT ?? path.join(output, "android");
const base = process.env.INDOPAK_SPECIMEN_BASE ?? "http://127.0.0.1:5391";
const adb = process.env.INDOPAK_ADB ?? "/Users/hmziq/Library/Android/sdk/platform-tools/adb";
const corpus = await loadCorpus(output);
const critical = [
  ...new Set([
    ...corpus.captureKeys,
    "33:38",
    "12:21",
    "18:110",
    "56:23",
    "19:17",
    "91:14",
    "97:5",
  ]),
];
const report = {
  status: "failed",
  platform: "Android emulator",
  matrices: [],
  scrolls: [],
  errors: [],
};
await mkdir(destination, { recursive: true });
const [device] = await _android.devices({ omitDriverInstall: true });
assert.ok(device, "No Android emulator connected");
let context;
function shell(...args) {
  return execFileSync(adb, ["-s", device.serial(), "shell", ...args], { encoding: "utf8" }).trim();
}
try {
  report.device = {
    serial: device.serial(),
    model: shell("getprop", "ro.product.model"),
    android: shell("getprop", "ro.build.version.release"),
    sdk: shell("getprop", "ro.build.version.sdk"),
    chrome: shell("dumpsys", "package", "com.android.chrome").match(/versionName=(\S+)/u)?.[1],
  };
  context = await device.launchBrowser({ viewport: null, serviceWorkers: "block" });
  context.setDefaultNavigationTimeout(120_000);
  context.setDefaultTimeout(60_000);
  const page = await context.newPage();
  page.on("pageerror", (error) => report.errors.push(error.message));
  shell("settings", "put", "system", "accelerometer_rotation", "0");
  for (const orientation of ["portrait", "landscape"]) {
    shell("settings", "put", "system", "user_rotation", orientation === "portrait" ? "0" : "1");
    await page.waitForFunction(
      (expected) => innerWidth > innerHeight === expected,
      orientation === "landscape",
    );
    for (const mode of ["reading", "verse"]) {
      const response = await page.goto(
        `${base}/design/indopak?audit=all&mode=${mode}&keys=${critical.join(",")}`,
        { waitUntil: "domcontentloaded" },
      );
      assert.equal(response.status(), 200);
      await page.waitForFunction(controlsReady);
      await page.evaluate(() => document.fonts.ready);
      for (const kind of ["Ring", "Digits"])
        await page.evaluate(addFont, await diagnosticFont(output, kind));
      for (const size of [22, 33, 56]) {
        await page.getByLabel("Font size").selectOption(String(size));
        await page.getByLabel("Run width").selectOption("320");
        const specimens = await page.evaluate(inspectSpecimens, {
          requireInk: true,
          fontReference: corpus.fontReference,
        });
        const summary = assertSpecimens(specimens, corpus, false);
        const overflow = await page.evaluate(layoutOverflow);
        assert.equal(overflow.page_overflow, false);
        assert.ok(overflow.run_overflow.every((key) => ["12:21", "18:110", "56:23"].includes(key)));
        const geometry = await page.evaluate(() => ({
          viewport: [innerWidth, innerHeight],
          dpr: devicePixelRatio,
          scale: visualViewport.scale,
          font: getComputedStyle(document.querySelector("[data-indopak-ayah]")).fontSize,
        }));
        report.matrices.push({ orientation, mode, size, ...summary, ...overflow, ...geometry });
        for (const key of ["33:38", "17:7", "2:286", "51:54", "19:17", "91:14", "56:23"]) {
          await page.locator(`[data-specimen="${key}"]`).screenshot({
            path: path.join(
              destination,
              `${orientation}-${mode}-${size}-${key.replace(":", "-")}.png`,
            ),
          });
        }
      }
      await page.getByLabel("Font size").selectOption("33");
      const keys = await page
        .locator("[data-specimen]")
        .evaluateAll((nodes) => nodes.map((node) => node.dataset.specimen));
      for (const key of keys)
        await page.locator(`[data-specimen="${key}"]`).scrollIntoViewIfNeeded();
      report.scrolls.push({
        orientation,
        mode,
        unique_verses: keys.length,
        private_occurrences: report.matrices.at(-1).private_occurrences,
      });
      console.log(`android: ${orientation} ${mode} matrix and ${keys.length}-verse scroll passed`);
    }
  }
  shell("settings", "put", "system", "user_rotation", "0");
  await page.goto(`${base}/design/indopak?keys=17:7`);
  await page.waitForFunction(controlsReady);
  const original = await page
    .locator('[data-specimen="17:7"] [data-indopak-ayah]')
    .evaluate((node) => {
      const text = node.cloneNode(true);
      text.querySelector("[data-indopak-ornament]").remove();
      return text.textContent;
    });
  assert.equal(original, corpus.originals["17:7"]);
  const client = await context.newCDPSession(page);
  await page.locator('[data-specimen="17:7"]').scrollIntoViewIfNeeded();
  await client.send("Input.synthesizePinchGesture", {
    x: 200,
    y: 300,
    scaleFactor: 1.5,
    gestureSourceType: "touch",
  });
  const zoom = await page.evaluate(() => ({
    scale: visualViewport.scale,
    viewport: [innerWidth, innerHeight],
  }));
  assert.ok(zoom.scale > 1, `Pinch zoom did not apply: ${JSON.stringify(zoom)}`);
  await page.screenshot({ path: path.join(destination, "pinch-zoom.png") });
  report.zoom = zoom;
  await page.goto(`${base}/design/indopak?keys=33:38`);
  await page.goBack();
  assert.ok(page.url().includes("keys=17:7"));
  await page.goForward();
  assert.ok(page.url().includes("keys=33:38"));
  report.history = "passed";
  const font = await context.request.get(`${base}/fonts/indopak-reader-compat-v4.woff2`);
  assert.equal(font.status(), 200);
  const bytes = await font.body();
  assert.deepEqual(
    bytes,
    await readFile(path.join(root, "web/static/fonts/indopak-reader-compat-v4.woff2")),
  );
  report.font = { bytes: bytes.length, identical_to_package: true };
  assert.deepEqual(report.errors, []);
  report.status = "mechanical_checks_passed";
} catch (error) {
  report.error = String(error.stack);
  process.exitCode = 1;
} finally {
  shell("settings", "put", "system", "user_rotation", "0");
  await context?.close();
  await device.close();
  await writeFile(
    path.join(destination, "android-report.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
}
console.log(JSON.stringify({ ...report, matrices: report.matrices.length }));
