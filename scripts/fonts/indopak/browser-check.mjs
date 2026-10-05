import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath } from "node:url";

import { chromium, firefox, webkit } from "playwright";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const base = process.env.INDOPAK_SPECIMEN_BASE ?? "http://localhost:5391";
const output = process.env.INDOPAK_SPECIMEN_OUTPUT ?? path.join(root, ".cache/indopak-browser");
const database = new DatabaseSync(path.join(root, "db/quran/arabic/quran-indopak.sqlite"), {
  readOnly: true,
});
const rows = database.prepare("SELECT sura, aya, text FROM quran_text").all();
database.close();
const originals = new Map(rows.map((row) => [`${row.sura}:${row.aya}`, String(row.text)]));
const manifest = JSON.parse(
  await readFile(path.join(root, "scripts/fonts/indopak/mapping.json"), "utf8"),
);
const inventory = JSON.parse(
  await readFile(path.join(root, "scripts/fonts/indopak/inventory.json"), "utf8"),
);
const font = await readFile(path.join(root, "web/static/fonts/indopak-reader-compat-v2.woff2"));
await mkdir(output, { recursive: true });
const reports = [];

async function checkOrnaments(page, engine) {
  const runs = await page.locator(".ayah-ornament").evaluateAll((elements) =>
    elements.map((element) => ({
      key: element.getAttribute("data-verse-anchor"),
      width: element.getBoundingClientRect().width,
      fontSize: Number.parseFloat(getComputedStyle(element).fontSize),
      text: element.textContent,
    })),
  );
  assert.ok(runs.length > 0);
  for (const run of runs) {
    assert.ok(
      run.width > 0 && run.width <= run.fontSize * 1.05,
      `${engine}: ${run.key} digits escaped medallion (${run.width / run.fontSize}em)`,
    );
  }
  return runs;
}

async function checkPrivateMarks(page, engine) {
  const painted = await page.evaluate(() => {
    const size = 120;
    const width = size * 12;
    const height = size * 3;
    function pixels(text, direction) {
      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = height;
      const context = canvas.getContext("2d");
      context.font = `${size}px "IndoPak Reader Compat"`;
      context.direction = direction;
      context.textAlign = "right";
      context.fillText(text, width - size, size * 2);
      return context.getImageData(0, 0, width, height).data;
    }
    const original = pixels("لِيَسُـوْۤء\uE004ا", "rtl");
    const diagnostic = pixels("لِيَسُـوْۤء\u0657ا", "rtl");
    let minY = height;
    let maxY = 0;
    for (let y = 0; y < height; y += 1) {
      for (let x = 0; x < width; x += 1) {
        const alpha = (y * width + x) * 4 + 3;
        if (Math.abs(original[alpha] - diagnostic[alpha]) <= 64) continue;
        minY = Math.min(y, minY);
        maxY = Math.max(y, maxY);
      }
    }
    const stack = pixels("\uE021\u06D9", "ltr");
    const columns = [];
    for (let x = 0; x < width; x += 1) {
      for (let y = 0; y < height; y += 1) {
        if (stack[(y * width + x) * 4 + 3] <= 32) continue;
        columns.push(x);
        break;
      }
    }
    const runs = [];
    for (const column of columns) {
      const last = runs.at(-1);
      if (!last || column > last[1] + 1) runs.push([column, column]);
      else last[1] = column;
    }
    return {
      inverted_damma_difference_height_em: (maxY - minY) / size,
      optional_pause_ink_runs: runs,
    };
  });
  assert.ok(
    painted.inverted_damma_difference_height_em < 0.35,
    `${engine}: inverted damma drifted away from its Unicode reference`,
  );
  assert.equal(
    painted.optional_pause_ink_runs.length,
    2,
    `${engine}: optional ayah and pause collide`,
  );
  return painted;
}

async function checkEndSigns(page, engine) {
  const signs = await page.locator(".indopak-end-sign").evaluateAll((elements) => {
    const context = document.createElement("canvas").getContext("2d");
    function ink(element) {
      const style = getComputedStyle(element);
      context.font = `${style.fontSize} ${style.fontFamily}`;
      const metrics = context.measureText(element.textContent);
      const box = element.getBoundingClientRect();
      const baseline =
        Number.parseFloat(style.lineHeight) / 2 +
        (metrics.fontBoundingBoxAscent - metrics.fontBoundingBoxDescent) / 2;
      const top = box.top + baseline - metrics.actualBoundingBoxAscent;
      return { top, size: Number.parseFloat(style.fontSize) };
    }
    return elements.map((element) => {
      const ornament = element.parentElement.querySelector(".ayah-ornament");
      const frame = ink(ornament);
      const bottom = Math.max(
        ...[...element.children].map((child) => {
          const mark = ink(child);
          return mark.top + mark.size * 0.35;
        }),
      );
      const finalWord = element.closest(".indopak-final-word");
      return {
        key: ornament.getAttribute("data-verse-anchor"),
        gap: frame.top - bottom,
        lines: finalWord.getClientRects().length,
      };
    });
  });
  for (const sign of signs) {
    assert.ok(sign.gap > 0, `${engine}: ${sign.key} end sign collides with its ornament`);
    assert.equal(sign.lines, 1, `${engine}: ${sign.key} orphaned ayah ornament`);
  }
  assert.ok(signs.length > 0);
  return signs.length;
}

for (const [name, engine] of Object.entries({ chromium, firefox, webkit })) {
  let browser;
  try {
    browser = await engine.launch({ headless: true, timeout: 30000 });
    const page = await browser.newPage({ viewport: { width: 1100, height: 900 } });
    const pageErrors = [];
    page.on("pageerror", (failure) => pageErrors.push(failure.message));
    const response = await page.goto(`${base}/design/indopak`);
    assert.equal(response.status(), 200);
    await page
      .getByRole("heading", { name: "IndoPak font compatibility specimen", exact: true })
      .waitFor();
    await page.waitForFunction(() => !document.querySelector("select").disabled);
    await page.getByLabel("Font size").selectOption("33");
    await page.evaluate(async () => {
      await document.fonts.load('33px "IndoPak Reader Compat"', "\uE003\uE004\uE022");
      await document.fonts.load('33px "Ayah Ornament"', "\u06DD١٠١");
      await document.fonts.ready;
    });
    const packaged = await page.request.get(`${base}/fonts/indopak-reader-compat-v2.woff2`);
    assert.equal(packaged.status(), 200);
    assert.ok((await packaged.body()).equals(font));
    const loaded = await page.evaluate(() =>
      [...document.fonts].some(
        (face) =>
          face.family.replaceAll('"', "") === "IndoPak Reader Compat" && face.status === "loaded",
      ),
    );
    assert.ok(loaded);
    const ornaments = await checkOrnaments(page, name);
    for (const digitCount of [1, 2, 3]) {
      assert.ok(ornaments.some((run) => run.text.length === digitCount + 1));
    }
    const privateMarks = await checkPrivateMarks(page, name);
    const endSigns = await checkEndSigns(page, name);
    const specimens = await page.locator("[data-specimen]").evaluateAll((elements) =>
      elements.map((element) => {
        const original = element.querySelector("[data-indopak-ayah]").cloneNode(true);
        original.querySelector("[data-indopak-ornament]").remove();
        return {
          key: element.getAttribute("data-specimen"),
          text: original.textContent,
          family: getComputedStyle(element.querySelector(".verse-text")).fontFamily,
        };
      }),
    );
    for (const specimen of specimens) {
      assert.equal(
        specimen.text,
        originals.get(specimen.key),
        `${name}: original string ${specimen.key}`,
      );
      assert.ok(specimen.family.includes("IndoPak Reader Compat"));
    }
    for (const item of inventory.codepoints) {
      const character = String.fromCodePoint(Number.parseInt(item.codepoint.slice(2), 16));
      assert.ok(specimens.some((specimen) => specimen.text.includes(character)));
    }
    const contextCount = manifest.entries.reduce(
      (total, entry) => total + entry.contexts.length,
      0,
    );
    for (const size of [24, 33, 48]) {
      await page.getByLabel("Font size").selectOption(String(size));
      await page.waitForFunction(
        (expected) =>
          getComputedStyle(document.querySelector("[data-specimen] .verse-text")).fontSize ===
          `${expected}px`,
        size,
      );
      for (const width of [320, 640, 960]) {
        await page.getByLabel("Run width").selectOption(String(width));
        await page.waitForFunction(
          (expected) =>
            getComputedStyle(document.querySelector("[data-specimen]")).maxWidth ===
            `${expected}px`,
          width,
        );
        const empty = await page
          .locator("[data-specimen] .verse-text")
          .evaluateAll(
            (elements) =>
              elements.filter((element) => element.getBoundingClientRect().width <= 0).length,
          );
        assert.equal(empty, 0);
        await checkOrnaments(page, name);
        await checkEndSigns(page, name);
      }
    }
    await page.getByLabel("Font size").selectOption("48");
    await page.getByLabel("Run width").selectOption("960");
    const originalShot = await page.locator('[data-diagnostic="original"]').screenshot();
    const diagnosticShot = await page.locator('[data-diagnostic="unicode"]').screenshot();
    await writeFile(path.join(output, `${name}-17-7-original.png`), originalShot);
    await writeFile(path.join(output, `${name}-17-7-unicode-diagnostic.png`), diagnosticShot);
    for (const entry of manifest.entries) {
      const key = entry.representative_verse_keys[0];
      await page
        .locator(`[data-specimen="${key}"]`)
        .screenshot({ path: path.join(output, `${name}-${entry.codepoint.slice(2)}.png`) });
    }
    await page
      .locator('[data-control="uthmani:1:7"]')
      .screenshot({ path: path.join(output, `${name}-uthmani-control.png`) });
    await page.getByLabel("Font size").selectOption("33");
    await page.getByLabel("Run width").selectOption("640");
    await page
      .locator('[data-specimen="2:101"]')
      .screenshot({ path: path.join(output, `${name}-2-101-ornament.png`) });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.getByLabel("Font size").selectOption("33");
    await page.getByLabel("Run width").selectOption("320");
    await page
      .locator('[data-specimen="17:7"]')
      .screenshot({ path: path.join(output, `${name}-phone-17-7.png`) });
    await checkOrnaments(page, name);
    await checkEndSigns(page, name);
    const fallbackPage = await browser.newPage({ viewport: { width: 390, height: 844 } });
    await fallbackPage.route("**/fonts/ayah-ornament.woff2", (route) => route.abort());
    await fallbackPage.goto(`${base}/design/indopak`);
    await fallbackPage.evaluate(async () => {
      await document.fonts.load('33px "Scheherazade New"', "\u06DD١٠١");
      await document.fonts.ready;
    });
    await checkOrnaments(fallbackPage, `${name} fallback`);
    await fallbackPage.close();
    assert.deepEqual(pageErrors, []);
    reports.push({
      engine: name,
      version: browser.version(),
      specimens: specimens.length,
      context_classes: contextCount,
      status: "checks_passed",
      font_loaded_from_packaged_bytes: true,
      original_strings_preserved: true,
      ornament_runs_checked: ornaments.length,
      ornament_digit_lengths: [1, 2, 3],
      ornament_fallback_checked: true,
      private_mark_paint: privateMarks,
      end_signs_checked: endSigns,
      sizes: [24, 33, 48],
      widths: [320, 640, 960],
      phone_viewport: [390, 844],
      production_approved: manifest.production_approved,
    });
  } catch (failure) {
    reports.push({
      engine: name,
      status: "harness_failed",
      error: failure.message,
      production_approved: false,
    });
  } finally {
    await browser?.close();
  }
}

await writeFile(path.join(output, "report.json"), JSON.stringify(reports, null, 2) + "\n");
console.log(JSON.stringify(reports, null, 2));
if (reports.some((report) => report.status === "harness_failed")) process.exitCode = 1;
