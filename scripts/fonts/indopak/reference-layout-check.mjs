import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { chromium, webkit } from "playwright";
import { controlsReady, hideFixedOverlays, loadCorpus, root } from "./deep-browser-shared.mjs";

const inputs = process.env.INDOPAK_DEEP_OUTPUT ?? path.join(root, ".cache/indopak-deep");
const output = path.resolve(process.env.INDOPAK_REVIEW_OUTPUT ?? path.join(inputs, "review"));
const base = process.env.INDOPAK_SPECIMEN_BASE ?? "http://localhost:5391";
const engine = process.env.INDOPAK_REVIEW_ENGINE ?? "chromium";
const baselineCommit = "104f049";
const corpus = await loadCorpus(inputs);
const targeted = [
  "2:101",
  "1:7",
  "17:7",
  "6:165",
  "79:27",
  "89:27",
  "83:31",
  "4:142",
  "2:286",
  "18:110",
  "71:23",
  "91:1",
  "104:4",
  "16:6",
  "73:17",
  "51:54",
  "26:51",
  "43:15",
  "35:11",
  "5:7",
  "12:108",
  "2:10",
  "7:137",
  "12:21",
  "56:23",
  "19:17",
  "91:14",
  "97:5",
];
const reviewKeys = new Set([...corpus.captureKeys, ...targeted]);
for (const code of [0x0614, 0x0615, ...Array.from({ length: 7 }, (_, i) => 0x06d6 + i)]) {
  const key = Object.keys(corpus.originals).find((item) =>
    corpus.originals[item].includes(String.fromCodePoint(code)),
  );
  if (key) reviewKeys.add(key);
}
let seed = 20261006;
function random() {
  seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
  return seed / 2 ** 32;
}
function sample(values, count) {
  const pool = [...values];
  const result = [];
  while (result.length < count) result.push(pool.splice(Math.floor(random() * pool.length), 1)[0]);
  return result;
}
const allKeys = Object.keys(corpus.originals);
const stratified = [];
for (let surah = 1; surah <= 114; surah += 1)
  stratified.push(
    ...sample(
      allKeys.filter((key) => key.startsWith(`${surah}:`)),
      2,
    ),
  );
stratified.push(
  ...sample(
    allKeys.filter((key) => !stratified.includes(key)),
    72,
  ),
);
assert.equal(new Set(stratified).size, 300);
const flowKeys = [
  ...new Set(corpus.occurrences.filter((item) => item.code === "E021").map((item) => item.key)),
];
const lineKeys = new Set([...stratified, ...flowKeys, ...targeted]);
const selected = process.env.INDOPAK_REVIEW_KEYS?.split(",");
const keys = [...new Set([...lineKeys, ...reviewKeys])]
  .filter((key) => !selected || selected.includes(key))
  .sort((a, b) => a.localeCompare(b, "en", { numeric: true }));
await mkdir(output, { recursive: true });

async function prepareBaseline() {
  const directory = path.join(output, "baseline");
  await mkdir(directory, { recursive: true });
  function historical(file) {
    return execFileSync("git", ["show", `${baselineCommit}:${file}`], {
      cwd: root,
      encoding: "utf8",
    });
  }
  const rendererPath = "web/src/routes/(application)/_reader/IndoPakAyah.svelte";
  const ornamentPath = "web/src/routes/(application)/_reader/AyahOrnament.svelte";
  const parserPath = "web/src/lib/quran/view/indopak.ts";
  const renderer = historical(rendererPath)
    .replace('"$lib/quran/view/indopak"', '"./indopak-v3.ts"')
    .replaceAll('"IndoPak Reader Compat"', '"IndoPak Review v3"');
  const ornament = historical(ornamentPath).replace(
    '"./position-label"',
    JSON.stringify(path.join(root, "web/src/routes/(application)/_reader/position-label.ts")),
  );
  await writeFile(path.join(directory, "IndoPakAyah.svelte"), renderer);
  await writeFile(path.join(directory, "AyahOrnament.svelte"), ornament);
  await writeFile(path.join(directory, "indopak-v3.ts"), historical(parserPath));
  await writeFile(
    path.join(directory, "entry.mjs"),
    `import {mount} from "svelte";
import Verse from "./IndoPakAyah.svelte";
export function render(target, props) { return mount(Verse, {target, props}); }
`,
  );
  return {
    commit: baselineCommit,
    renderer_path: rendererPath,
    ornament_path: ornamentPath,
    parser_path: parserPath,
    adaptations: ["import paths", "font-family alias"],
    entry: `/@fs${path.join(directory, "entry.mjs")}`,
  };
}

function inspectReference(key) {
  const element = document.querySelector(`[data-testid="verse-arabic-${key}"]`);
  const data = JSON.parse(document.querySelector("#__NEXT_DATA__").textContent);
  const verse = data.props.pageProps.versesResponse.verses.find((item) => item.verseKey === key);
  const sourceWords = new Map(verse.words.map((word) => [word.location, word]));
  let offset = 0;
  const words = [...element.querySelectorAll("[data-word-location]")].map((word) => {
    const text = word.querySelector("[class*=TextWord_word]");
    const source = sourceWords.get(word.dataset.wordLocation);
    const rect = word.getBoundingClientRect();
    const value = {
      location: word.dataset.wordLocation,
      text: text.textContent,
      legacy: source.textIndopak,
      type: source.charTypeName,
      offset,
      top: rect.top,
      left: rect.left,
      width: rect.width,
      family: getComputedStyle(text).fontFamily,
    };
    if (source.charTypeName === "word") offset += source.textIndopak.match(/\p{Lo}/gu)?.length ?? 0;
    return value;
  });
  const lineStarts = [];
  let previous;
  for (const word of words) {
    if (previous === undefined || Math.abs(word.top - previous) > 2) {
      lineStarts.push({
        offset: word.offset,
        text: word.text,
        type: word.type,
        location: word.location,
      });
      previous = word.top;
    }
  }
  const marker = words.find((word) => word.type !== "word");
  const lastWord = words.findLast((word) => word.type === "word");
  return {
    key,
    build_id: data.buildId,
    preferences: data.props.pageProps.__REDUX_STATE__.quranReaderStyles,
    width: element.getBoundingClientRect().width,
    size: Number.parseFloat(getComputedStyle(element).fontSize),
    dpr: devicePixelRatio,
    font_urls: performance
      .getEntriesByType("resource")
      .map((item) => item.name)
      .filter((url) => url.includes("indopak") && url.endsWith(".woff2")),
    loaded_fonts: [...document.fonts]
      .filter((font) => font.family.includes("IndoPak"))
      .map((font) => ({ family: font.family, status: font.status })),
    words,
    letters: verse.words
      .filter((word) => word.charTypeName === "word")
      .map((word) => word.textIndopak)
      .join("")
      .replace(/[^\p{Lo}]/gu, ""),
    line_starts: lineStarts,
    marker_orphaned: !!marker && Math.abs(marker.top - lastWord.top) > 2,
  };
}

function inspectLocal({ key, width, size }) {
  const sampleNode = document.querySelector(`[data-specimen="${key}"]`);
  sampleNode.style.width = `${width + 34}px`;
  sampleNode.style.maxWidth = `${width + 34}px`;
  sampleNode.style.setProperty("--reader-arabic-size", `${size}px`);
  const verse = sampleNode.querySelector("[data-indopak-ayah]");
  const clone = verse.cloneNode(true);
  clone.querySelector("[data-indopak-ornament]").remove();
  let offset = 0;
  const words = [...verse.querySelectorAll(".indopak-word, .indopak-final-word")].map((word) => {
    const value = word.cloneNode(true);
    value.querySelector(".indopak-ending")?.remove();
    const text = value.textContent;
    const walker = document.createTreeWalker(word, NodeFilter.SHOW_TEXT);
    let node;
    let top;
    while ((node = walker.nextNode())) {
      const match = /\p{Lo}/u.exec(node.textContent);
      if (!match) continue;
      const range = document.createRange();
      range.setStart(node, match.index);
      range.setEnd(node, match.index + match[0].length);
      top = range.getBoundingClientRect().top;
      break;
    }
    const result = { text, offset, top };
    offset += text.match(/\p{Lo}/gu)?.length ?? 0;
    return result;
  });
  const starts = [];
  let previous;
  for (const word of words) {
    if (previous === undefined || Math.abs(word.top - previous) > 2) {
      starts.push({ text: word.text, offset: word.offset });
      previous = word.top;
    }
  }
  const final = verse.querySelector(".indopak-final-word");
  const ornament = verse.querySelector("[data-indopak-ornament]");
  return {
    text: clone.textContent,
    letters: clone.textContent.replace(/[^\p{Lo}]/gu, ""),
    width: sampleNode.querySelector(".run").getBoundingClientRect().width,
    size: Number.parseFloat(getComputedStyle(verse).fontSize),
    line_starts: starts,
    words,
    final_word_with_marker: final.contains(ornament) && final.getClientRects().length === 1,
  };
}

const baseline = await prepareBaseline();
const report = {
  engine,
  status: "incomplete",
  recorded_at: new Date().toISOString(),
  baseline,
  line_anchor_source:
    "Quran.com word.textIndopak letter offsets; geometry from actual served word DOM",
  reference_sizing: "26px comparison size; adaptive native size recorded before override",
  sample: {
    seed: 20261006,
    stratified_random: stratified,
    flow: flowKeys,
    targeted,
    review: [...reviewKeys],
    selected: selected ?? null,
  },
  rows: [],
  captures: [],
  failures: [],
};
const implementation = { chromium, webkit }[engine];
assert.ok(implementation, `Unsupported review engine ${engine}`);
const browser = await implementation.launch();
report.version = browser.version();
const completed = new Set();
if (process.env.INDOPAK_REVIEW_RESUME === "1") {
  const previous = JSON.parse(
    await readFile(path.join(output, `${engine}-layout-report.json`), "utf8"),
  );
  assert.deepEqual(previous.sample, report.sample);
  assert.equal(previous.version, report.version);
  assert.equal(previous.baseline.commit, baselineCommit);
  await writeFile(
    path.join(output, `${engine}-layout-prior-attempt.json`),
    JSON.stringify(previous, null, 2) + "\n",
  );
  for (const key of keys) {
    if (previous.rows.filter((row) => row.key === key).length !== 3) continue;
    if (
      engine === "chromium" &&
      reviewKeys.has(key) &&
      !previous.captures.some((capture) => capture.key === key)
    )
      continue;
    completed.add(key);
  }
  report.rows = previous.rows.filter((row) => completed.has(row.key));
  report.captures = previous.captures.filter((capture) => completed.has(capture.key));
  report.prior_attempt_failures = previous.failures;
}
const context = await browser.newContext({
  viewport: { width: 390, height: 844 },
  deviceScaleFactor: 2,
  colorScheme: "light",
  serviceWorkers: "block",
});
const reference = await context.newPage();
const local = await context.newPage();
async function save() {
  await writeFile(
    path.join(output, `${engine}-layout-report.json`),
    JSON.stringify(report, null, 2) + "\n",
  );
}
try {
  await local.goto(`${base}/design/indopak?keys=${keys.join(",")}`);
  await local.waitForFunction(controlsReady);
  await local.evaluate(() => document.fonts.ready);
  await local.evaluate(hideFixedOverlays);
  await local.addStyleTag({
    content: `[data-audit-hide], [data-audit-hide] * {visibility:hidden !important} [data-specimen] .run {text-align:right} [data-review-v3] {font:400 26px/2.15 "IndoPak Review v3";direction:rtl;white-space:normal;padding-block:20px;color:var(--quran-foreground)} `,
  });
  for (const key of keys) {
    if (completed.has(key)) continue;
    try {
      const response = await reference.goto(`https://quran.com/${key.replace(":", "/")}`, {
        waitUntil: "domcontentloaded",
        timeout: 120_000,
      });
      assert.equal(response.status(), 200);
      const element = reference.locator(`[data-testid="verse-arabic-${key}"]`);
      await element.waitFor();
      await reference.evaluate(() => document.fonts.ready);
      for (const [index, viewport] of [
        { width: 390, height: 844 },
        { width: 320, height: 640 },
        { width: 1280, height: 900 },
      ].entries()) {
        await reference.setViewportSize(viewport);
        await reference.evaluate(
          () =>
            new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))),
        );
        const nativeSize = await element.evaluate((target) =>
          Number.parseFloat(getComputedStyle(target).fontSize),
        );
        await element.evaluate((target) => {
          target.style.transition = "none";
          target.style.fontSize = "26px";
          target.style.setProperty("--ayah-page-hero-quran-font-size", "26px");
        });
        const observed = await reference.evaluate(inspectReference, key);
        observed.native_size = nativeSize;
        observed.size_override = Math.abs(nativeSize - 26) > 0.001;
        assert.equal(observed.preferences.quranFont, "text_indopak");
        assert.equal(observed.size, 26);
        assert.ok(observed.words.every((word) => word.family.includes("IndoPak")));
        assert.ok(observed.loaded_fonts.some((font) => font.status === "loaded"));
        await local.setViewportSize(viewport);
        const ours = await local.evaluate(inspectLocal, {
          key,
          width: observed.width,
          size: observed.size,
        });
        assert.equal(ours.text, corpus.originals[key]);
        assert.ok(
          Math.abs(ours.width - observed.width) < 0.1,
          `${key}: widths ${ours.width}/${observed.width}`,
        );
        assert.equal(ours.size, observed.size);
        const comparable = ours.letters === observed.letters;
        const offsets = ours.line_starts.map((word) => word.offset);
        const referenceOffsets = observed.line_starts.map((word) => word.offset);
        report.rows.push({
          key,
          viewport,
          reference: observed,
          ours,
          comparable,
          identical_breaks:
            comparable && JSON.stringify(offsets) === JSON.stringify(referenceOffsets),
          line_count_difference: ours.line_starts.length - observed.line_starts.length,
          investigation:
            Math.abs(ours.line_starts.length - observed.line_starts.length) >= 2 ||
            (comparable && offsets.at(-1) !== referenceOffsets.at(-1) && !observed.marker_orphaned),
        });
        if (index !== 0 || !reviewKeys.has(key) || engine !== "chromium") continue;
        const filename = key.replace(":", "-");
        const referenceFile = `${filename}-qurancom.png`;
        const v4File = `${filename}-v4.png`;
        const v3File = `${filename}-v3.png`;
        await element.screenshot({ path: path.join(output, referenceFile) });
        const run = local.locator(`[data-specimen="${key}"] .run`);
        await run.screenshot({ path: path.join(output, v4File) });
        await local.evaluate(
          async ({ key: verseKey, text, entry }) => {
            const { render } = await import(entry);
            const article = document.querySelector(`[data-specimen="${verseKey}"]`);
            const target = document.createElement("div");
            target.dataset.reviewV3 = verseKey;
            target.dir = "rtl";
            target.lang = "ar";
            article.append(target);
            render(target, { text, n: Number(verseKey.split(":")[1]), vKey: verseKey });
            await document.fonts.load('26px "IndoPak Review v3"');
          },
          { key, text: corpus.originals[key], entry: baseline.entry },
        );
        await local
          .locator(`[data-review-v3="${key}"]`)
          .screenshot({ path: path.join(output, v3File) });
        report.captures.push({
          key,
          width: observed.width,
          size: observed.size,
          dpr: 2,
          reference: referenceFile,
          v4: v4File,
          v3: v3File,
          build_id: observed.build_id,
          font_urls: observed.font_urls,
        });
      }
      await save();
      console.log(`${engine}: reference layout ${key} (${report.rows.length / 3}/${keys.length})`);
    } catch (error) {
      report.failures.push({ key, error: String(error.stack ?? error) });
      await save();
      throw error;
    }
  }
  const comparableRows = report.rows.filter((row) => row.comparable);
  report.summary = {
    keys: keys.length,
    states: report.rows.length,
    comparable_states: comparableRows.length,
    identical_breaks_percent:
      (100 * comparableRows.filter((row) => row.identical_breaks).length) / comparableRows.length,
    mean_line_count_difference:
      report.rows.reduce((total, row) => total + row.line_count_difference, 0) / report.rows.length,
    investigations: report.rows
      .filter((row) => row.investigation)
      .map(({ key, viewport, line_count_difference, comparable }) => ({
        key,
        viewport,
        line_count_difference,
        comparable,
      })),
  };
  report.status = "captured_for_review";
} finally {
  await save();
  await browser.close();
}
