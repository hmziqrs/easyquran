import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { controlsReady, loadCorpus, root } from "./deep-browser-shared.mjs";
import { inspectLocal, inspectReference } from "./reference-layout-shared.mjs";
import { freePort, Session } from "./safari-native-check.mjs";

const inputs = process.env.INDOPAK_DEEP_OUTPUT ?? path.join(root, ".cache/indopak-deep");
const output = process.env.INDOPAK_REVIEW_OUTPUT ?? path.join(inputs, "review");
const base = process.env.INDOPAK_SPECIMEN_BASE ?? "http://localhost:5391";
const corpus = await loadCorpus(inputs);
const previous = JSON.parse(
  await readFile(path.join(output, "chromium-layout-report.json"), "utf8"),
);
const selected = process.env.INDOPAK_REVIEW_KEYS?.split(",");
const keys = [
  ...new Set([
    ...previous.sample.stratified_random,
    ...previous.sample.flow,
    ...previous.sample.targeted,
  ]),
]
  .filter((key) => !selected || selected.includes(key))
  .sort((a, b) => a.localeCompare(b, "en", { numeric: true }));
const report = {
  engine: "safari",
  status: "incomplete",
  recorded_at: new Date().toISOString(),
  sample: { ...previous.sample, native_keys: keys, selected: selected ?? null },
  rows: [],
  failures: [],
  unavailable_viewports: [],
};
await mkdir(output, { recursive: true });
const port = await freePort();
const driver = spawn("/usr/bin/safaridriver", ["-p", String(port)], { stdio: "ignore" });
const session = new Session(`http://127.0.0.1:${port}`);
async function save() {
  await writeFile(
    path.join(output, "safari-layout-report.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
}
async function switchTab(handle) {
  await session.command("POST", "/window", { handle });
}
try {
  for (let attempt = 0; ; attempt += 1) {
    try {
      if ((await session.raw("GET", "/status")).ready) break;
    } catch (error) {
      if (attempt >= 100) throw error;
    }
    await delay(100);
  }
  report.version = await session.start();
  const local = await session.command("GET", "/window");
  await session.navigate(`${base}/design/indopak?keys=${keys.join(",")}`);
  await session.waitFor(controlsReady);
  await session.executeAsync(() => document.fonts.ready.then(() => true));
  const remote = (await session.command("POST", "/window/new", { type: "tab" })).handle;
  for (const key of keys) {
    try {
      await switchTab(remote);
      await session.navigate(`https://quran.com/${key.replace(":", "/")}`);
      await session.waitFor(
        (verseKey) => !!document.querySelector(`[data-testid="verse-arabic-${verseKey}"]`),
        [key],
      );
      await session.executeAsync(() => document.fonts.ready.then(() => true));
      const references = [];
      for (const viewport of [
        { width: 390, height: 844 },
        { width: 320, height: 640 },
        { width: 1280, height: 900 },
      ]) {
        const geometry = await session.setViewport(viewport);
        if (
          !geometry.exact &&
          !report.unavailable_viewports.some(
            (item) => JSON.stringify(item.requested) === JSON.stringify(geometry.requested),
          )
        )
          report.unavailable_viewports.push(geometry);
        const nativeSize = await session.execute((verseKey) => {
          const element = document.querySelector(`[data-testid="verse-arabic-${verseKey}"]`);
          const native = Number.parseFloat(getComputedStyle(element).fontSize);
          element.style.transition = "none";
          element.style.fontSize = "26px";
          element.style.setProperty("--ayah-page-hero-quran-font-size", "26px");
          return native;
        }, key);
        const reference = await session.execute(inspectReference, key);
        assert.equal(reference.size, 26);
        assert.equal(reference.preferences.quranFont, "text_indopak");
        assert.ok(reference.loaded_fonts.some((font) => font.status === "loaded"));
        references.push({
          viewport: { width: geometry.actual[0], height: geometry.actual[1] },
          requested_viewport: viewport,
          reference: {
            ...reference,
            native_size: nativeSize,
            size_override: Math.abs(nativeSize - 26) > 0.001,
          },
        });
      }
      await switchTab(local);
      for (const { viewport, requested_viewport, reference } of references) {
        await session.setViewport(viewport);
        const ours = await session.execute(inspectLocal, {
          key,
          width: reference.width,
          size: reference.size,
        });
        assert.equal(ours.text, corpus.originals[key]);
        assert.equal(ours.size, reference.size);
        assert.ok(Math.abs(ours.width - reference.width) < 0.1);
        const comparable = ours.letters === reference.letters;
        const offsets = ours.line_starts.map((word) => word.offset);
        const referenceOffsets = reference.line_starts.map((word) => word.offset);
        report.rows.push({
          key,
          viewport,
          requested_viewport,
          reference,
          ours,
          comparable,
          identical_breaks:
            comparable && JSON.stringify(offsets) === JSON.stringify(referenceOffsets),
          line_count_difference: ours.line_starts.length - reference.line_starts.length,
          investigation: Math.abs(ours.line_starts.length - reference.line_starts.length) >= 2,
        });
      }
      await save();
      console.log(`safari: reference layout ${key} (${report.rows.length / 3}/${keys.length})`);
    } catch (error) {
      report.failures.push({ key, error: String(error.stack ?? error) });
      throw error;
    }
  }
  report.status = "captured_for_review";
  if (report.unavailable_viewports.length) report.status = "captured_for_review_with_viewport_gap";
} catch (error) {
  report.error = String(error.stack ?? error);
  process.exitCode = 1;
} finally {
  await session.stop();
  driver.kill();
  await save();
}
