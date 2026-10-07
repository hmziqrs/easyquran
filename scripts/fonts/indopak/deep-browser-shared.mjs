import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath, pathToFileURL } from "node:url";

export const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
export const SIZES = [22, 24, 33, 48, 56];
export const WIDTHS = [320, 640, 960];
export const DESKTOP_VIEWPORT = { width: 1100, height: 900 };
export const PHONE_VIEWPORT = { width: 390, height: 844 };
export const SCREENSHOT_KEYS = ["17:7", "1:7", "2:101", "16:6", "73:17", "51:54", "26:51", "43:15"];
export const SIZE_ADJUST = "125%";
const ORNAMENT_MAX_EM = 1.15;

export async function loadCorpus(output) {
  const occurrences = JSON.parse(await readFile(path.join(output, "occurrences.json"), "utf8"));
  const codes = [...new Set(occurrences.map((item) => item.code))].sort((a, b) =>
    a.localeCompare(b),
  );
  const url = pathToFileURL(path.join(root, "db/quran/arabic/quran-indopak.sqlite"));
  url.search = "?mode=ro&immutable=1";
  const database = new DatabaseSync(url, { readOnly: true });
  const rows = database.prepare("SELECT sura, aya, text FROM quran_text").all();
  database.close();
  const originals = Object.fromEntries(
    rows.map((row) => {
      assert.equal(typeof row.sura, "number");
      assert.equal(typeof row.aya, "number");
      assert.equal(typeof row.text, "string");
      return [`${String(row.sura)}:${String(row.aya)}`, row.text];
    }),
  );
  const fontReference = JSON.parse(
    await readFile(path.join(output, "font-reference.json"), "utf8"),
  );
  const segmentation = JSON.parse(
    await readFile(path.join(root, "scripts/fonts/indopak/segmentation-report.json"), "utf8"),
  );
  const captureKeys = [
    ...new Set([
      ...segmentation.context_coverage.phase_c_capture_keys,
      ...segmentation.context_coverage.phase_c_review_keys,
    ]),
  ];
  return { occurrences, codes, originals, fontReference, captureKeys, segmentation };
}

export function diagnosticKinds(codes) {
  return ["Actual", "Base", "Ring", "Digits", ...codes, ...codes.map((code) => `Except ${code}`)];
}

export async function diagnosticFont(output, kind) {
  const data = await readFile(
    path.join(output, `audit-${kind.toLowerCase().replaceAll(" ", "-")}.woff2`),
  );
  return {
    name: `IndoPak Audit ${kind}`,
    base64: data.toString("base64"),
    sizeAdjust: SIZE_ADJUST,
  };
}

export function domKinds(code) {
  return ["Original", "Actual", code, `Except ${code}`];
}

export function domImageName(engine, key, kind) {
  return `${engine}-dom-${key.replace(":", "-")}-${kind.replaceAll(" ", "-")}.png`;
}

export function auditStyle(name, key) {
  const selector = `[data-specimen="${key}"]`;
  const privateOnly = /^[A-F0-9]{4}$/.test(name);
  const ornamentVisibility = privateOnly ? "hidden" : "visible";
  const family = name === "Original" ? "IndoPak Reader Compat" : `IndoPak Audit ${name}`;
  const surroundingFamily = name === "Original" ? family : "IndoPak Audit Actual";
  const target = `${selector} [data-verse-key="${key}"]`;
  const hideNeighbors = privateOnly
    ? `${selector} [data-verse-key]:not([data-verse-key="${key}"]) {visibility:hidden !important}`
    : "";
  return `[data-audit-hide], [data-audit-hide] * {visibility:hidden !important} [data-specimen] {display:none !important} ${selector} {display:block !important;background:white !important;border-color:transparent !important} ${selector} h3, ${selector} .context {visibility:hidden} ${selector} .verse-text.indopak, ${selector} .indopak-end-sign {font-family: "${surroundingFamily}" !important;color:black !important} ${target} .verse-text.indopak, ${target} .indopak-end-sign {font-family:"${family}" !important} ${selector} [data-indopak-ornament] {visibility:${ornamentVisibility}} ${selector} .run {background:white !important} ${hideNeighbors}`;
}

export function captureTargets({ occurrences, specimenKeys, targetKeys }) {
  const specimens = new Set(specimenKeys);
  const targets = new Map();
  for (const occurrence of occurrences) {
    if (!specimens.has(occurrence.key)) continue;
    if (targetKeys && !targetKeys.includes(occurrence.key)) continue;
    targets.set(`${occurrence.key}:${occurrence.code}`, occurrence);
  }
  return targets;
}

export function assertSpecimens(
  specimens,
  { occurrences, originals },
  flowOnly,
  requireInk = true,
  partialCorpus = false,
) {
  let count = 0;
  for (const specimen of specimens) {
    assert.equal(specimen.text, originals[specimen.key], `Changed text ${specimen.key}`);
    count += specimen.privateCount;
    assert.ok(
      specimen.ornament_width_em > 0 && specimen.ornament_width_em <= ORNAMENT_MAX_EM,
      `Escaped ornament ${specimen.key}`,
    );
    assert.equal(specimen.final_lines, 1, `Split final word ${specimen.key}`);
    assert.ok(/\p{L}/u.test(specimen.final_text), `Missing Arabic word ${specimen.key}`);
    if (specimen.final_width > specimen.container_width + 1)
      assert.ok(
        ["56:23"].includes(specimen.key),
        `Unpredicted final word overflow ${specimen.key}`,
      );
    assert.deepEqual(specimen.sign_led_boxes, [], `Sign-led word boxes ${specimen.key}`);
    assert.ok(
      /^\u06dd[\u06f0-\u06f9]{1,3}$/u.test(specimen.ornament_text),
      `Unexpected marker encoding ${specimen.key}`,
    );
    if (requireInk) {
      for (const row of specimen.end_rows) {
        assert.ok(
          row.clearance_em >= 0.02,
          `End sign touches ring ${specimen.key}: ${row.clearance_em}`,
        );
        assert.ok(
          Math.abs(row.center_error_em) <= 0.05,
          `End sign off centre ${specimen.key}: ${row.center_error_em}`,
        );
        assert.ok(
          row.previous_line_clearance === null || row.previous_line_clearance >= 0,
          `End sign touches previous line ${specimen.key}`,
        );
      }
    }
    assert.ok(specimen.family.includes("IndoPak Reader Compat"));
  }
  const expectedKeys = new Set(specimens.map((item) => item.key));
  const expectedCount = occurrences.filter((item) => expectedKeys.has(item.key)).length;
  assert.equal(count, flowOnly || partialCorpus ? expectedCount : occurrences.length);
  if (flowOnly)
    assert.equal(
      expectedKeys.size,
      new Set(occurrences.filter((item) => item.code === "E021").map((item) => item.key)).size,
    );
  return {
    specimens: specimens.length,
    private_occurrences: count,
    end_clusters: specimens.filter((item) => item.ending_text).length,
    end_geometry: specimens
      .filter((item) => item.end_rows.length)
      .map((item) => ({ key: item.key, rows: item.end_rows })),
    ring_checks: specimens.filter((item) => item.ring_check).length,
    occurrence_scopes: specimens.flatMap((item) => item.occurrences).length,
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
export function controlsReady() {
  const control = document.querySelector("select");
  return !!control && !control.disabled;
}
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
export function inspectSpecimens(options = {}) {
  const context = document.createElement("canvas").getContext("2d");
  const baselines = new Map();
  const ringChecks = new Map();
  function nodeInk(node) {
    const style = getComputedStyle(node.parentElement);
    const size = Number.parseFloat(style.fontSize);
    if (!size || !node.textContent.trim()) return null;
    context.font = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
    context.direction = style.direction;
    context.textAlign = "left";
    const metrics = context.measureText(node.textContent);
    if (!Number.isFinite(metrics.fontBoundingBoxAscent))
      throw new Error("Missing font baseline metrics");
    const range = document.createRange();
    range.selectNodeContents(node);
    const box = range.getBoundingClientRect();
    const baselineKey = `${context.font}|${box.height}`;
    if (!baselines.has(baselineKey)) {
      const probe = document.createElement("span");
      probe.style.cssText =
        "display:inline-block;width:0;height:0;vertical-align:baseline;padding:0;border:0;margin:0";
      node.before(probe);
      const baseline = probe.getBoundingClientRect().top;
      const after = range.getBoundingClientRect();
      probe.remove();
      if (
        Math.abs(after.top - box.top) > 1 / 64 ||
        Math.abs(after.width - box.width) > 1 / 64 ||
        Math.abs(after.height - box.height) > 1 / 64
      )
        throw new Error("Baseline probe changed layout");
      baselines.set(baselineKey, baseline - box.top);
    }
    const baseline = box.top + baselines.get(baselineKey);
    const characters = Array.from(node.textContent.trim());
    if (
      node.parentElement.closest("[data-indopak-ornament]") &&
      options.fontReference?.ayah_enclosures
    ) {
      const ring = options.fontReference.ayah_enclosures[characters.length - 2];
      if (!ring) throw new Error("Unexpected ayah marker length");
      return {
        baseline,
        left: box.left + ring[0] * size,
        right: box.left + ring[2] * size,
        top: baseline - ring[3] * size,
        bottom: baseline - ring[1] * size,
      };
    }
    const sign =
      characters.length === 1 &&
      options.fontReference?.single_signs?.[
        characters[0].codePointAt(0).toString(16).toUpperCase().padStart(4, "0")
      ];
    if (sign && node.parentElement.closest(".indopak-end-sign,.indopak-body-sign"))
      return {
        baseline,
        left: box.left + sign[0] * size,
        right: box.left + sign[2] * size,
        top: baseline - sign[3] * size,
        bottom: baseline - sign[1] * size,
      };
    return {
      baseline,
      left: box.left - metrics.actualBoundingBoxLeft,
      right: box.left + metrics.actualBoundingBoxRight,
      top: baseline - metrics.actualBoundingBoxAscent,
      bottom: baseline + metrics.actualBoundingBoxDescent,
    };
  }
  function inkFor(element) {
    const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
    const boxes = [];
    let node;
    while ((node = walker.nextNode())) {
      if (node.parentElement.closest(".indopak-space, [data-indopak-ornament]")) continue;
      const box = nodeInk(node);
      if (box) boxes.push(box);
    }
    if (!boxes.length) return null;
    return {
      baseline: Math.max(...boxes.map((box) => box.baseline)),
      left: Math.min(...boxes.map((box) => box.left)),
      right: Math.max(...boxes.map((box) => box.right)),
      top: Math.min(...boxes.map((box) => box.top)),
      bottom: Math.max(...boxes.map((box) => box.bottom)),
    };
  }
  function ringCheck(element) {
    if (!options.canvasRingDiagnostic) return null;
    const style = getComputedStyle(element);
    const size = Number.parseFloat(style.fontSize);
    const cacheKey = `${element.textContent}|${size}|${style.fontWeight}|${style.direction}`;
    if (ringChecks.has(cacheKey)) return ringChecks.get(cacheKey);
    const scale = 4;
    const width = Math.ceil(element.getBoundingClientRect().width * scale) + 8 * scale;
    const height = Math.ceil(size * 5 * scale);
    function pixels(family) {
      const canvas = document.createElement("canvas");
      canvas.lang = element.closest("[lang]")?.lang ?? "ur";
      canvas.width = width;
      canvas.height = height;
      const ctx = canvas.getContext("2d");
      ctx.font = `${style.fontWeight} ${size * scale}px "${family}"`;
      ctx.direction = style.direction;
      ctx.textAlign = "left";
      ctx.fillText(element.textContent, 4 * scale, size * 3 * scale);
      return ctx.getImageData(0, 0, width, height).data;
    }
    const ring = pixels("IndoPak Audit Ring");
    const digits = pixels("IndoPak Audit Digits");
    let left = width;
    let right = 0;
    let top = height;
    let bottom = 0;
    let ringPixels = 0;
    for (let y = 0; y < height; y += 1) {
      for (let x = 0; x < width; x += 1) {
        if (ring[(y * width + x) * 4 + 3] <= 96) continue;
        ringPixels += 1;
        left = Math.min(left, x);
        right = Math.max(right, x + 1);
        top = Math.min(top, y);
        bottom = Math.max(bottom, y + 1);
      }
    }
    const radius = Math.min(right - left, bottom - top) / 2;
    const inner = radius - options.fontReference.ring_stroke_em * size * scale;
    const external = new Uint8Array(width * height);
    const queue = new Uint32Array(width * height);
    let head = 0;
    let tail = 0;
    function visit(x, y) {
      if (x < 0 || y < 0 || x >= width || y >= height) return;
      const index = y * width + x;
      if (external[index] || ring[index * 4 + 3] > 96) return;
      external[index] = 1;
      queue[tail++] = index;
    }
    for (let x = 0; x < width; x += 1) {
      visit(x, 0);
      visit(x, height - 1);
    }
    for (let y = 0; y < height; y += 1) {
      visit(0, y);
      visit(width - 1, y);
    }
    while (head < tail) {
      const index = queue[head++];
      const x = index % width;
      const y = Math.floor(index / width);
      visit(x - 1, y);
      visit(x + 1, y);
      visit(x, y - 1);
      visit(x, y + 1);
    }
    let outside = 0;
    let digitPixels = 0;
    for (let y = 0; y < height; y += 1) {
      for (let x = 0; x < width; x += 1) {
        if (digits[(y * width + x) * 4 + 3] <= 96) continue;
        digitPixels += 1;
        if (external[y * width + x] || ring[(y * width + x) * 4 + 3] > 96) outside += 1;
      }
    }
    const result = {
      ring_pixels: ringPixels,
      digit_pixels: digitPixels,
      outside_pixels: outside,
      sampling_scale: scale,
      inner_radius_em: inner / scale / size,
    };
    ringChecks.set(cacheKey, result);
    return result;
  }
  const selector = options.reader ? "[data-verse-key]" : "[data-specimen]";
  return Array.from(document.querySelectorAll(selector))
    .filter((element) => element.querySelector("[data-indopak-ayah]"))
    .filter(
      (element) =>
        !options.keys ||
        options.keys.includes(element.dataset.specimen ?? element.dataset.verseKey),
    )
    .map((element) => {
      const key = element.dataset.specimen ?? element.dataset.verseKey;
      const verseContainer = options.reader
        ? element
        : element.querySelector(`[data-verse-key="${key}"]`);
      if (!verseContainer) throw new Error(`Missing verse container ${key}`);
      const verse = verseContainer.querySelector("[data-indopak-ayah]");
      const text = verse.cloneNode(true);
      text.querySelector("[data-indopak-ornament]").remove();
      const privateCount = Array.from(text.textContent).filter((character) =>
        /\p{Co}/u.test(character),
      ).length;
      const finalWord = verse.querySelector(".indopak-final-word");
      const finalBox = finalWord.getBoundingClientRect();
      const run = element.querySelector(".run") ?? element.closest(".reading-flow,.verse-row");
      const container = run.getBoundingClientRect();
      const ending = verse.querySelector(".indopak-end-sign");
      const ornament = verse.querySelector(".ayah-ornament");
      const ornamentBox = ornament.getBoundingClientRect();
      const style = getComputedStyle(ornament);
      const size = Number.parseFloat(getComputedStyle(verse).fontSize);
      const wordElements = Array.from(verse.querySelectorAll(".indopak-word"));
      const signLed = wordElements
        .filter((word, index) => {
          if (/^\p{L}/u.test(word.textContent)) return false;
          if (index === 0 && /^[\p{Cf}\s]*\p{L}/u.test(word.textContent)) return false;
          if (index === 0 && /^[\p{Cf}\s]+$/u.test(word.textContent)) return false;
          return true;
        })
        .map((word) => word.textContent);
      const finalInk = inkFor(finalWord);
      const previous = Array.from(run.querySelectorAll(".indopak-word"))
        .map(inkFor)
        .filter((box) => box && box.baseline < finalInk.baseline - size * 0.5);
      const previousBottom = previous.length
        ? Math.max(...previous.map((box) => box.bottom))
        : null;
      const ringNode = Array.from(ornament.childNodes).find(
        (node) => node.nodeType === Node.TEXT_NODE,
      );
      const ringInk = nodeInk(ringNode);
      const endRows = Array.from(verse.querySelectorAll(".indopak-end-sign > span")).map((row) => {
        const box = inkFor(row);
        const alignedPrevious = previous.filter(
          (previousBox) => previousBox.right > box.left && previousBox.left < box.right,
        );
        const alignedBottom = alignedPrevious.length
          ? Math.max(...alignedPrevious.map((previousBox) => previousBox.bottom))
          : null;
        return {
          code: row.textContent.trim().codePointAt(0).toString(16).toUpperCase(),
          ink: box,
          clearance_em: (ringInk.top - box.bottom) / size,
          center_error_em: ((box.left + box.right) / 2 - (ringInk.left + ringInk.right) / 2) / size,
          previous_line_clearance: alignedBottom === null ? null : box.top - alignedBottom,
          previous_line_vertical_clearance:
            previousBottom === null ? null : box.top - previousBottom,
          previous_line_aligned_ink_boxes: alignedPrevious,
        };
      });
      const scopes = [];
      const article = element.getBoundingClientRect();
      const clipping = [];
      if (options.reader) {
        for (
          let ancestor = element;
          ancestor && ancestor !== document.body;
          ancestor = ancestor.parentElement
        ) {
          const ancestorStyle = getComputedStyle(ancestor);
          const clipsX = /^(hidden|clip)$/u.test(ancestorStyle.overflowX);
          const clipsY = /^(hidden|clip)$/u.test(ancestorStyle.overflowY);
          const clipsPaint = /\b(paint|strict|content)\b/u.test(ancestorStyle.contain);
          if (!clipsX && !clipsY && !clipsPaint) continue;
          const box = ancestor.getBoundingClientRect();
          for (const row of endRows) {
            if (
              (clipsY || clipsPaint) &&
              (row.ink.top < box.top - 1 / 64 || row.ink.bottom > box.bottom + 1 / 64)
            )
              clipping.push({
                code: row.code,
                axis: "y",
                ancestor: ancestor.className,
                ink: row.ink,
                clip: box.toJSON(),
              });
            if (
              (clipsX || clipsPaint) &&
              (row.ink.left < box.left - 1 / 64 || row.ink.right > box.right + 1 / 64)
            )
              clipping.push({
                code: row.code,
                axis: "x",
                ancestor: ancestor.className,
                ink: row.ink,
                clip: box.toJSON(),
              });
          }
        }
      }
      const walker = document.createTreeWalker(verse, NodeFilter.SHOW_TEXT);
      let node;
      let offset = 0;
      while ((node = walker.nextNode())) {
        if (node.parentElement.closest("[data-indopak-ornament]")) continue;
        for (let index = 0; index < node.textContent.length; index += 1) {
          const character = node.textContent[index];
          if (!/\p{Co}/u.test(character)) continue;
          const box = nodeInk(node);
          const code = character.codePointAt(0).toString(16).toUpperCase();
          const sign = options.fontReference?.single_signs?.[code];
          if (sign) {
            const glyphRange = document.createRange();
            glyphRange.setStart(node, index);
            glyphRange.setEnd(node, index + 1);
            const glyph = glyphRange.getBoundingClientRect();
            const size = Number.parseFloat(getComputedStyle(node.parentElement).fontSize);
            box.left = Math.min(box.left, glyph.left + sign[0] * size);
            box.right = Math.max(box.right, glyph.left + sign[2] * size);
            box.top = Math.min(box.top, box.baseline - sign[3] * size);
            box.bottom = Math.max(box.bottom, box.baseline - sign[1] * size);
          }
          const range = document.createRange();
          range.selectNodeContents(node);
          const layout = range.getBoundingClientRect();
          const same = Array.from(node.textContent).filter((value) => value === character).length;
          scopes.push({
            key,
            code: character.codePointAt(0).toString(16).toUpperCase(),
            index: offset + index,
            scope: [
              Math.min(box.left, layout.left) - article.left,
              Math.min(box.top, layout.top) - article.top,
              Math.max(box.right, layout.right) - article.left,
              Math.max(box.bottom, layout.bottom) - article.top,
            ],
            same_code_in_scope: same,
          });
        }
        offset += node.textContent.length;
      }
      return {
        key,
        text: text.textContent,
        privateCount,
        final_lines: new Set(
          Array.from(finalWord.getClientRects(), (rectangle) => Math.round(rectangle.top)),
        ).size,
        final_text: finalWord.textContent,
        final_width: finalBox.width,
        container_width: container.width,
        ornament_width_em: ornamentBox.width / Number.parseFloat(style.fontSize),
        ornament_text: ornament.textContent,
        ending_text: ending?.textContent ?? "",
        family: getComputedStyle(verse.parentElement).fontFamily,
        sign_led_boxes: signLed,
        end_rows: endRows,
        ring_check: ringCheck(ornament),
        occurrences: scopes,
        clipping,
      };
    });
}

export function paintOccurrences(items) {
  const keys = new Map(
    Array.from(document.querySelectorAll("[data-specimen]"), (element) => [
      element.dataset.specimen,
      element,
    ]),
  );
  return items
    .filter((item) => keys.has(item.key))
    .map((item) => ({
      key: item.key,
      index: item.index,
      code: item.code,
      scope: "word_box_dom_capture",
    }));
}

export async function fontAdjustmentEvidence(reference) {
  const sampleText = reference.words.map((word) => word.text).join(" ");
  const effectiveSize = 33 * reference.size_adjust;
  await document.fonts.load('33px "IndoPak Reader Compat"', sampleText);
  await document.fonts.load(`${effectiveSize}px "IndoPak Audit Upstream"`, sampleText);
  await document.fonts.load('132px "IndoPak Audit Upstream"', sampleText);
  const samples = [];
  for (const word of reference.words) {
    const element = document.createElement("span");
    element.lang = "ar";
    element.dir = "rtl";
    element.textContent = word.text;
    element.style.cssText =
      'position:absolute;visibility:hidden;display:inline-block;white-space:nowrap;font:400 33px "IndoPak Reader Compat"';
    document.body.append(element);
    const width = element.getBoundingClientRect().width;
    element.style.fontFamily = '"IndoPak Audit Upstream"';
    element.style.fontSize = `${effectiveSize}px`;
    const expected = element.getBoundingClientRect().width;
    element.style.fontSize = "132px";
    const upstream = element.getBoundingClientRect().width / 4;
    element.remove();
    const harfbuzz = word.packaged_advance_em * reference.size_adjust * 33;
    samples.push({
      key: word.key,
      width,
      upstream_33_px: upstream,
      upstream_normalized_adjusted_px: upstream * reference.size_adjust,
      upstream_effective_font_size_css_px: effectiveSize,
      expected,
      harfbuzz_expected: harfbuzz,
      engine_shaping_delta_px: width - harfbuzz,
      error_css_px: width - expected,
      honoured: Math.abs(width - expected) <= 1 / 32,
    });
  }
  return {
    check: document.fonts.check('33px "IndoPak Reader Compat"', sampleText),
    faces: Array.from(document.fonts)
      .filter((face) => face.family.replaceAll('"', "") === "IndoPak Reader Compat")
      .map((face) => ({ status: face.status, size_adjust: face.sizeAdjust })),
    samples,
    advance_tolerance_css_px: 1 / 32,
    reference_measurement: "Same engine, pinned upstream face at identical effective font size",
  };
}
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
export function hideFixedOverlays() {
  for (const candidate of document.querySelectorAll("body *")) {
    if (candidate.closest("[data-ring-grid]")) continue;
    if (["fixed", "sticky"].includes(getComputedStyle(candidate).position)) {
      candidate.setAttribute("data-audit-hide", "");
      candidate.style.setProperty("visibility", "hidden", "important");
    }
  }
}
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
export async function addFont({ name, base64, sizeAdjust }) {
  const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0));
  const face = new FontFace(name, bytes, { sizeAdjust });
  document.fonts.add(await face.load());
}
export function buildRingGrid(options) {
  const {
    size,
    first = 1,
    last = 286,
    columns = 12,
    fixed = false,
  } = typeof options === "number" ? { size: options } : options;
  document.querySelector("[data-ring-grid]")?.remove();
  const prototype = document.querySelector("[data-indopak-ornament] .ayah-ornament");
  const source = getComputedStyle(prototype);
  const grid = document.createElement("div");
  grid.dataset.ringGrid = "";
  grid.style.cssText = `display:grid;grid-template-columns:repeat(${columns},80px);gap:8px;padding:4px;width:max-content;background:white;color:black`;
  if (fixed) {
    grid.style.position = "fixed";
    grid.style.top = "0";
    grid.style.left = "0";
    grid.style.zIndex = "2147483647";
  }
  for (let number = first; number <= last; number += 1) {
    const cell = document.createElement("div");
    cell.dataset.ringNumber = String(number);
    cell.style.cssText =
      "display:flex;align-items:center;justify-content:center;width:80px;height:90px;background:white";
    const span = document.createElement("span");
    span.className = "ayah-ornament";
    span.lang = "ur";
    span.textContent =
      "\u06dd" +
      Array.from(String(number), (digit) => String.fromCodePoint(0x06f0 + Number(digit))).join("");
    span.style.cssText = `font-family:"IndoPak Reader Compat";font-weight:${source.fontWeight};font-size:${size * 0.68}px;font-feature-settings:${source.fontFeatureSettings};font-variant-ligatures:${source.fontVariantLigatures};font-kerning:${source.fontKerning};letter-spacing:${source.letterSpacing};direction:${source.direction};unicode-bidi:isolate;white-space:nowrap;line-height:1;color:black;margin:0`;
    cell.append(span);
    grid.append(cell);
  }
  document.body.append(grid);
  return {
    size,
    first,
    last,
    columns,
    fixed,
    direction: source.direction,
    features: source.fontFeatureSettings,
    language: "ur",
  };
}
export function setRingGridKind(kind) {
  const family = kind === "Original" ? "IndoPak Reader Compat" : `IndoPak Audit ${kind}`;
  for (const span of document.querySelectorAll("[data-ring-grid] .ayah-ornament"))
    span.style.fontFamily = `"${family}"`;
}
export function ringGridGeometry() {
  const grid = document.querySelector("[data-ring-grid]").getBoundingClientRect();
  return {
    width: grid.width,
    height: grid.height,
    scale: devicePixelRatio,
    numbers: Array.from(document.querySelectorAll("[data-ring-number]"), (cell) => {
      const box = cell.getBoundingClientRect();
      return {
        number: Number(cell.dataset.ringNumber),
        box: [
          box.left - grid.left,
          box.top - grid.top,
          box.right - grid.left,
          box.bottom - grid.top,
        ],
        marker_width: cell.firstElementChild.getBoundingClientRect().width,
      };
    }),
  };
}
export function removeRingGrid() {
  document.querySelector("[data-ring-grid]")?.remove();
}
export function assertRingLayout(original, current) {
  assert.equal(current.scale, original.scale);
  assert.equal(current.numbers.length, original.numbers.length);
  const differences = [
    Math.abs(current.width - original.width),
    Math.abs(current.height - original.height),
  ];
  for (let index = 0; index < original.numbers.length; index += 1) {
    const first = original.numbers[index];
    const second = current.numbers[index];
    assert.equal(first.number, second.number);
    differences.push(Math.abs(first.marker_width - second.marker_width));
    differences.push(...first.box.map((value, axis) => Math.abs(value - second.box[axis])));
  }
  const maximum = Math.max(...differences);
  assert.ok(maximum <= 1 / 64, `Ring layout drift exceeds one layout unit: ${maximum}`);
  return maximum;
}
export function paintShapingOccurrences(items) {
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
