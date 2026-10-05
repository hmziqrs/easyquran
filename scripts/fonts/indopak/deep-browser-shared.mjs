// Checks shared by the Playwright (deep-browser-check.mjs) and native Safari
// (safari-native-check.mjs) runners. Functions marked "browser side" are serialized
// into the page, so they must stay self-contained: no imports, no outer bindings.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath } from "node:url";

export const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
export const SIZES = [22, 24, 33, 48, 56];
export const WIDTHS = [320, 640, 960];
export const DESKTOP_VIEWPORT = { width: 1100, height: 900 };
export const PHONE_VIEWPORT = { width: 390, height: 844 };
export const SCREENSHOT_KEYS = ["17:7", "1:7", "2:101", "16:6", "73:17", "51:54", "26:51", "43:15"];
const EXTRA_DOM_KEYS = ["12:1", "16:6", "73:17", "51:54", "79:27", "26:51", "43:15", "4:142"];

export async function loadCorpus(output) {
  const occurrences = JSON.parse(await readFile(path.join(output, "occurrences.json"), "utf8"));
  const codes = [...new Set(occurrences.map((item) => item.code))].sort((a, b) =>
    a.localeCompare(b),
  );
  const database = new DatabaseSync(path.join(root, "db/quran/arabic/quran-indopak.sqlite"), {
    readOnly: true,
  });
  const rows = database.prepare("SELECT sura, aya, text FROM quran_text").all();
  database.close();
  const originals = Object.fromEntries(rows.map((row) => [`${row.sura}:${row.aya}`, row.text]));
  return { occurrences, codes, originals };
}

export function diagnosticKinds(codes) {
  return ["Actual", "Base", ...codes, ...codes.map((code) => `Except ${code}`)];
}

export async function diagnosticFont(output, kind) {
  const data = await readFile(
    path.join(output, `audit-${kind.toLowerCase().replaceAll(" ", "-")}.woff2`),
  );
  return { name: `IndoPak Audit ${kind}`, base64: data.toString("base64") };
}

export function domKinds(code) {
  return ["Original", "Actual", code, `Except ${code}`];
}

export function domImageName(engine, key, kind) {
  return `${engine}-dom-${key.replace(":", "-")}-${kind.replaceAll(" ", "-")}.png`;
}

export function auditStyle(name, key) {
  const selector = `[data-specimen="${key}"]`;
  const ornamentVisibility = /^[A-F0-9]{4}$/.test(name) ? "hidden" : "visible";
  const family = name === "Original" ? "IndoPak Reader Compat" : `IndoPak Audit ${name}`;
  return `[data-audit-hide], [data-audit-hide] * {visibility:hidden !important} [data-specimen] {display:none !important} ${selector} {display:block !important;background:white !important;border-color:transparent !important} ${selector} h3, ${selector} .context {visibility:hidden} ${selector} .verse-text.indopak, ${selector} .indopak-end-sign {font-family: "${family}" !important;color:black !important} ${selector} [data-indopak-ornament] {visibility:${ornamentVisibility}} ${selector} .run {background:white !important}`;
}

export function captureTargets({ occurrences, specimenKeys, inlineKeys, painted, targetKeys }) {
  const specimens = new Set(specimenKeys);
  const inline = new Set(inlineKeys);
  const collisions = new Set(
    painted.filter((item) => item.overlap >= 3).map((item) => `${item.key}:${item.code}`),
  );
  const targets = new Map();
  for (const occurrence of occurrences) {
    if (!specimens.has(occurrence.key)) continue;
    const signature = `${occurrence.key}:${occurrence.code}`;
    if (
      ["E003", "E004", "E021"].includes(occurrence.code) ||
      collisions.has(signature) ||
      EXTRA_DOM_KEYS.includes(occurrence.key) ||
      inline.has(occurrence.key)
    ) {
      if (targetKeys && !targetKeys.includes(occurrence.key)) continue;
      targets.set(signature, occurrence);
    }
  }
  return targets;
}

export function assertSpecimens(specimens, { occurrences, originals }, flowOnly) {
  let count = 0;
  for (const specimen of specimens) {
    assert.equal(specimen.text, originals[specimen.key], `Changed text ${specimen.key}`);
    count += specimen.privateCount;
    assert.ok(
      specimen.ornament_width_em > 0 && specimen.ornament_width_em <= 1.05,
      `Escaped ornament ${specimen.key}`,
    );
    assert.equal(specimen.final_lines, 1, `Split final word ${specimen.key}`);
    assert.ok(/\p{L}/u.test(specimen.final_text), `Missing Arabic word ${specimen.key}`);
    assert.ok(
      specimen.final_width <= specimen.container_width + 1,
      `Final word overflow ${specimen.key}`,
    );
    assert.ok(specimen.family.includes("IndoPak Reader Compat"));
  }
  const expectedKeys = new Set(specimens.map((item) => item.key));
  const expectedCount = occurrences.filter((item) => expectedKeys.has(item.key)).length;
  assert.equal(count, flowOnly ? expectedCount : occurrences.length);
  if (flowOnly)
    assert.equal(
      expectedKeys.size,
      new Set(occurrences.filter((item) => item.code === "E021").map((item) => item.key)).size,
    );
  return {
    specimens: specimens.length,
    private_occurrences: count,
    end_clusters: specimens.filter((item) => item.ending_text).length,
  };
}

export function paintFailures(painted) {
  return painted.filter((item) => item.painted === 0 || item.edge > 0 || item.outsideMismatch > 0);
}

export function overlapCandidates(painted) {
  return painted
    .filter((item) => item.overlap >= 8)
    .map((item) => ({
      key: item.key,
      index: item.index,
      code: item.code,
      overlap: item.overlap,
      fraction: item.overlap_fraction,
    }));
}

// Browser side.
export function controlsReady() {
  return !document.querySelector("select").disabled;
}

// Browser side.
export function specimenKeySets() {
  const keys = (selector) =>
    [...document.querySelectorAll(selector)].map((element) =>
      element.getAttribute("data-specimen"),
    );
  return {
    specimenKeys: keys("[data-specimen]"),
    inlineKeys: keys("[data-specimen]:has(.indopak-body-sign)"),
  };
}

// Browser side.
export function inspectSpecimens() {
  return [...document.querySelectorAll("[data-specimen]")].map((element) => {
    const key = element.getAttribute("data-specimen");
    const verse = element.querySelector(`[data-verse-key="${key}"] [data-indopak-ayah]`);
    const text = verse.cloneNode(true);
    text.querySelector("[data-indopak-ornament]").remove();
    // Counts private code points; grapheme segmentation would merge marks into letters.
    // oxlint-disable-next-line typescript/no-misused-spread
    const privateCount = [...text.textContent].filter((character) =>
      /\p{Co}/u.test(character),
    ).length;
    const finalWord = verse.querySelector(".indopak-final-word");
    const finalBox = finalWord.getBoundingClientRect();
    const container = element.querySelector(".run").getBoundingClientRect();
    const ending = verse.querySelector(".indopak-end-sign");
    const ornament = verse.querySelector(".ayah-ornament");
    const ornamentBox = ornament.getBoundingClientRect();
    const style = getComputedStyle(ornament);
    return {
      key: element.getAttribute("data-specimen"),
      text: text.textContent,
      privateCount,
      final_lines: new Set(
        [...finalWord.getClientRects()].map((rectangle) => Math.round(rectangle.top)),
      ).size,
      final_text: finalWord.textContent,
      final_width: finalBox.width,
      container_width: container.width,
      ornament_width_em: ornamentBox.width / Number.parseFloat(style.fontSize),
      ending_text: ending?.textContent ?? "",
      family: getComputedStyle(verse.parentElement).fontFamily,
    };
  });
}

// Browser side. Horizontal overflow of the page and of each specimen run; recorded for review.
export function layoutOverflow() {
  const page = document.documentElement;
  return {
    page_overflow: page.scrollWidth > page.clientWidth + 1,
    run_overflow: [...document.querySelectorAll("[data-specimen]")]
      .filter((element) => {
        const run = element.querySelector(".run");
        return run.scrollWidth > run.clientWidth + 1;
      })
      .map((element) => element.getAttribute("data-specimen")),
  };
}

// Browser side.
export function hideFixedOverlays() {
  for (const candidate of document.querySelectorAll("body *")) {
    if (["fixed", "sticky"].includes(getComputedStyle(candidate).position)) {
      candidate.setAttribute("data-audit-hide", "");
    }
  }
}

// Browser side. Empty css removes the audit stylesheet.
export function setAuditStyle(css) {
  let element = document.querySelector("style[data-audit-style]");
  if (!css) {
    element?.remove();
    return;
  }
  if (!element) {
    element = document.createElement("style");
    element.setAttribute("data-audit-style", "");
    document.head.append(element);
  }
  element.textContent = css;
}

// Browser side.
export async function addFont({ name, base64 }) {
  const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0));
  const face = new FontFace(name, bytes);
  document.fonts.add(await face.load());
}

// Browser side. Canvas runs isolate each private glyph's ink in its neighboring words.
export function paintOccurrences(items) {
  const size = 96;
  const canvases = new Map();
  function pixels(text, family) {
    let canvas = canvases.get(family);
    if (!canvas) {
      canvas = document.createElement("canvas");
      canvases.set(family, canvas);
    }
    const context = canvas.getContext("2d");
    context.font = `${size}px "IndoPak Reader Compat"`;
    canvas.width = Math.ceil(context.measureText(text).width) + size * 4;
    canvas.height = size * 5;
    context.font = `${size}px "${family}"`;
    context.direction = "rtl";
    context.textAlign = "right";
    context.fillText(text, canvas.width - size * 2, size * 3);
    return {
      width: canvas.width,
      height: canvas.height,
      data: context.getImageData(0, 0, canvas.width, canvas.height).data,
    };
  }
  const negative = pixels("بِسْمِ اللّٰهِ", "IndoPak Audit E021");
  if ([...negative.data].some((value, index) => index % 4 === 3 && value > 96)) {
    throw new Error("Diagnostic font failed transparent-ink control");
  }
  const cache = new Map();
  return items.map((item) => {
    const signature = `${item.code}:${item.excerpt}`;
    let result = cache.get(signature);
    if (!result) {
      const ink = pixels(item.excerpt, `IndoPak Audit ${item.code}`);
      const ordinary = pixels(item.excerpt, `IndoPak Audit Except ${item.code}`);
      const original = pixels(item.excerpt, "IndoPak Audit Actual");
      let painted = 0;
      let overlap = 0;
      let edge = 0;
      let minX = ink.width;
      let maxX = 0;
      let minY = ink.height;
      let maxY = 0;
      for (let y = 0; y < ink.height; y += 1) {
        for (let x = 0; x < ink.width; x += 1) {
          const index = (y * ink.width + x) * 4 + 3;
          if (ink.data[index] <= 96) continue;
          painted += 1;
          if (ordinary.data[index] > 96) overlap += 1;
          if (x === 0 || y === 0 || x === ink.width - 1 || y === ink.height - 1) edge += 1;
          minX = Math.min(minX, x);
          maxX = Math.max(maxX, x);
          minY = Math.min(minY, y);
          maxY = Math.max(maxY, y);
        }
      }
      let outsideMismatch = 0;
      for (let y = 0; y < ink.height; y += 1) {
        for (let x = 0; x < ink.width; x += 1) {
          if (x >= minX - 2 && x <= maxX + 2 && y >= minY - 2 && y <= maxY + 2) continue;
          const index = (y * ink.width + x) * 4 + 3;
          if (Math.abs(original.data[index] - ordinary.data[index]) > 96) outsideMismatch += 1;
        }
      }
      result = {
        painted,
        overlap,
        overlap_fraction: overlap / Math.max(1, painted),
        edge,
        outsideMismatch,
        bounds: [minX, minY, maxX, maxY],
      };
      cache.set(signature, result);
    }
    return { key: item.key, index: item.index, code: item.code, ...result };
  });
}
