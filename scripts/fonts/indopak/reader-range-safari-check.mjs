import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { assertSpecimens, inspectSpecimens, loadCorpus, root } from "./deep-browser-shared.mjs";
import { freePort, Session } from "./safari-native-check.mjs";

const inputs = process.env.INDOPAK_DEEP_OUTPUT ?? path.join(root, ".cache/indopak-deep");
const output = process.env.INDOPAK_INTERACTION_OUTPUT ?? path.join(inputs, "interactions");
const base = process.env.INDOPAK_READER_BASE ?? "http://localhost:5392";
const corpus = await loadCorpus(inputs);
const report = {
  status: "incomplete",
  platform: "native macOS Safari",
  conditions: {
    viewport: [390, 844],
    scroll: "programmatic",
    telemetry: "unmodified",
    error_window: "after_font_readiness",
  },
  matrices: [],
  errors: [],
};
await mkdir(output, { recursive: true });
const port = await freePort();
const driver = spawn("/usr/bin/safaridriver", ["-p", String(port)], { stdio: "ignore" });
const session = new Session(`http://127.0.0.1:${port}`);
async function save() {
  await writeFile(
    path.join(output, "safari-range-report.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
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
  assert.equal((await session.setViewport({ width: 390, height: 844 })).exact, true);
  await session.navigate(`${base}/al-fatihah`);
  for (const mode of ["reading", "verse"])
    for (const size of [22, 56])
      for (const theme of ["light", "dark"]) {
        await session.execute(
          ({ mode, size, theme }) => {
            localStorage.setItem(
              "easyquran.reader",
              JSON.stringify({ v: 4, arabicScript: "indopak", mode, fontSize: size }),
            );
            localStorage.setItem(
              "easyquran.prefs",
              JSON.stringify({ v: 1, mode: theme, palette: "sacred" }),
            );
          },
          { mode, size, theme },
        );
        for (const route of ["/al-fatihah", "/page/1", "/juz/30"]) {
          report.active_case = { route, mode, size, theme };
          await session.navigate(`${base}${route}?mode=${mode}`);
          await session.waitFor(() => {
            const verse = document.querySelector("[data-indopak-ayah]");
            return !!verse && getComputedStyle(verse).visibility === "visible";
          });
          await session.executeAsync(() => document.fonts.ready.then(() => true));
          await session.execute(() => {
            window.indopakRangeErrors = [];
            window.addEventListener("error", (event) =>
              window.indopakRangeErrors.push(event.message),
            );
            window.addEventListener("unhandledrejection", (event) =>
              window.indopakRangeErrors.push(String(event.reason)),
            );
          });
          assert.equal(await session.execute(() => document.documentElement.dataset.mode), theme);
          assert.equal(
            await session.execute(() =>
              Number.parseFloat(
                getComputedStyle(document.querySelector("[data-indopak-ayah]")).fontSize,
              ),
            ),
            size,
          );
          const specimens = await session.execute(inspectSpecimens, {
            reader: true,
            requireInk: true,
            fontReference: corpus.fontReference,
          });
          const summary = assertSpecimens(specimens, corpus, false, true, true);
          for (const specimen of specimens)
            assert.deepEqual(specimen.clipping, [], `Clipped ${specimen.key}`);
          const filename = `safari-${mode}-${size}-${theme}-${route.replaceAll("/", "_")}.png`;
          const screenshot = await session.command("GET", "/screenshot");
          await writeFile(path.join(output, filename), Buffer.from(screenshot, "base64"));
          report.errors.push(...(await session.execute(() => window.indopakRangeErrors)));
          report.matrices.push({
            ...report.active_case,
            ...summary,
            keys: specimens.map((item) => item.key),
            screenshot: filename,
          });
          await save();
          console.log(`safari: ${mode} ${size} ${theme} ${route}`);
        }
      }
  assert.deepEqual(report.errors, []);
  delete report.active_case;
  report.status = "passed";
} catch (error) {
  report.error = String(error.stack ?? error);
  process.exitCode = 1;
} finally {
  await session.stop();
  driver.kill();
  await save();
}
