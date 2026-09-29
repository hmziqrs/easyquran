// Screenshot + probe harness used for docs/research/ux-audit (see README.md in this folder).
// usage (dev server running on :5391):  node docs/research/ux-audit/tools/shoot.mjs spec.json
// spec: { base, out, shots: [{ name, url, w, h, mode, palette, dpr, full, prefs, reader, actions, annotate, probe, axe, clip, selector }] }
import { createRequire } from "node:module";
import { readFile, writeFile, mkdir } from "node:fs/promises";
import path from "node:path";

// Puppeteer is resolved from the web workspace so no extra install is needed.
const require = createRequire(new URL("../../../../web/package.json", import.meta.url));
const puppeteer = require("puppeteer");

const spec = JSON.parse(await readFile(process.argv[2], "utf8"));
const base = spec.base ?? "http://localhost:5391";
const out = spec.out;
await mkdir(out, { recursive: true });
// axe-core is only needed for `"axe": true` shots; loaded from a local copy, else from cdnjs.
async function loadAxe() {
  try {
    return await readFile(new URL("./axe.min.js", import.meta.url), "utf8");
  } catch {
    const res = await fetch("https://cdnjs.cloudflare.com/ajax/libs/axe-core/4.10.2/axe.min.js");
    return res.text();
  }
}
const axeSrc = spec.shots.some((x) => x.axe) ? await loadAxe() : "";

const browser = await puppeteer.launch({
  headless: true,
  args: ["--force-color-profile=srgb", "--disable-lcd-text", "--lang=en-US"],
});

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function settle(page) {
  await page.evaluate(() => document.fonts.ready);
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
}

async function runActions(page, actions = []) {
  for (const a of actions) {
    try {
      if (a.click) {
        await page.waitForSelector(a.click, { timeout: a.timeout ?? 8000 });
        await page.click(a.click);
      } else if (a.clickText) {
        const ok = await page.evaluate((t, tag) => {
          const els = [...document.querySelectorAll(tag ?? "button, a, [role=button], [role=tab], [role=option], label, summary")];
          const el = els.find((e) => (e.textContent ?? "").trim().replace(/\s+/g, " ").includes(t) || e.getAttribute("aria-label") === t);
          if (el) { el.scrollIntoView({ block: "center" }); el.click(); return true; }
          return false;
        }, a.clickText, a.tag);
        if (!ok) console.warn("  clickText miss:", a.clickText);
      } else if (a.type) {
        await page.waitForSelector(a.type[0], { timeout: 8000 });
        await page.type(a.type[0], a.type[1], { delay: 20 });
      } else if (a.press) {
        await page.keyboard.press(a.press);
      } else if (a.down) {
        await page.keyboard.down(a.down);
      } else if (a.up) {
        await page.keyboard.up(a.up);
      } else if (a.hover) {
        await page.hover(a.hover);
      } else if (a.wait) {
        await sleep(a.wait);
      } else if (a.waitFor) {
        await page.waitForSelector(a.waitFor, { timeout: a.timeout ?? 15000 });
      } else if (a.eval) {
        await page.evaluate(a.eval);
      } else if (a.scroll !== undefined) {
        await page.evaluate((y) => window.scrollTo(0, y), a.scroll);
      } else if (a.offline !== undefined) {
        await page.setOfflineMode(a.offline);
      } else if (a.reload) {
        await page.reload({ waitUntil: "domcontentloaded", timeout: 60000 }).catch(() => {});
      } else if (a.goto) {
        await page.goto(base + a.goto, { waitUntil: "networkidle2", timeout: 60000 });
      }
    } catch (e) {
      console.warn("  action failed", JSON.stringify(a), String(e).slice(0, 160));
    }
    await sleep(a.after ?? 250);
  }
}

async function annotate(page, notes = []) {
  if (!notes.length) return;
  await page.evaluate((notes) => {
    const layer = document.createElement("div");
    layer.id = "__audit_layer";
    layer.style.cssText = "position:absolute;left:0;top:0;width:0;height:0;z-index:2147483647;pointer-events:none;";
    document.body.appendChild(layer);
    const byText = (t, tag) => {
      const cands = [...document.querySelectorAll(tag ?? "*")].filter((e) => {
        const r = e.getBoundingClientRect();
        return r.width > 0 && (e.textContent ?? "").replace(/\s+/g, " ").includes(t);
      });
      cands.sort((a, b) => (a.textContent.length - b.textContent.length) || (a.getBoundingClientRect().width * a.getBoundingClientRect().height - b.getBoundingClientRect().width * b.getBoundingClientRect().height));
      return cands;
    };
    notes.forEach((n, i) => {
      let els;
      if (n.rect) els = [{ getBoundingClientRect: () => ({ left: n.rect[0] - scrollX, top: n.rect[1] - scrollY, width: n.rect[2], height: n.rect[3], right: n.rect[0] + n.rect[2] - scrollX }) }];
      else if (n.text) els = byText(n.text, n.tag);
      else els = [...document.querySelectorAll(n.sel)];
      const pick = n.all ? els : [els[n.index ?? 0]].filter(Boolean);
      if (!pick.length) console.warn("annotate miss " + (n.sel ?? n.text ?? "rect"));
      pick.forEach((el, j) => {
        const r = el.getBoundingClientRect();
        const pad = n.pad ?? 3;
        const box = document.createElement("div");
        box.style.cssText = `position:absolute;left:${r.left + scrollX - pad}px;top:${r.top + scrollY - pad}px;width:${r.width + pad * 2}px;height:${r.height + pad * 2}px;border:2.5px solid #ff2d55;border-radius:6px;box-shadow:0 0 0 1px #fff;`;
        layer.appendChild(box);
        if (j === 0 && n.label !== "") {
          const tag = document.createElement("div");
          tag.textContent = n.label ?? String(i + 1);
          const side = n.labelSide ?? "top";
          let left = r.left + scrollX - pad;
          let top = r.top + scrollY - pad - 22;
          if (side === "right") { left = r.right + scrollX + pad + 2; top = r.top + scrollY - pad; }
          if (side.startsWith("below")) { top = r.top + r.height + scrollY + pad + 3; }
          tag.style.cssText = `position:absolute;left:0px;top:${Math.max(0, top)}px;background:#ff2d55;color:#fff;font:700 12px/20px system-ui,sans-serif;padding:0 7px;border-radius:10px;white-space:nowrap;box-shadow:0 0 0 1px #fff;direction:ltr;`;
          layer.appendChild(tag);
          const w = tag.offsetWidth;
          if (side === "left") { left = r.left + scrollX - pad - w - 4; tag.style.top = `${r.top + scrollY - pad}px`; }
          if (side === "below-end" ) left = r.right + scrollX + pad - w;
          const maxLeft = document.documentElement.clientWidth + scrollX - w - 4;
          tag.style.left = `${Math.max(scrollX + 2, Math.min(left, maxLeft))}px`;
        }
      });
    });
  }, notes);
}

async function compose(s) {
  const page = await browser.newPage();
  await page.setViewport({ width: s.w ?? 1600, height: 400, deviceScaleFactor: 1 });
  const parts = [];
  for (const it of s.compose) {
    const buf = await readFile(it.src);
    const ext = it.src.split(".").pop();
    parts.push(`<figure><figcaption>${it.caption ?? ""}</figcaption><img src="data:image/${ext};base64,${buf.toString("base64")}" style="${it.style ?? ""}"></figure>`);
  }
  await page.setContent(`<html><body style="margin:0;padding:20px;background:#e9e9ee;font:600 15px system-ui,sans-serif;color:#222"><div style="display:flex;gap:20px;align-items:flex-start;flex-direction:${s.stack ? "column" : "row"};flex-wrap:${s.wrap ? "wrap" : "nowrap"}">${parts.join("")}</div><style>figure{margin:0;flex:1 1 0;align-self:stretch;min-width:0;background:#fff;border:1px solid #ccc;border-radius:10px;padding:10px}figcaption{margin:0 0 8px;font:700 15px system-ui}img{width:100%;height:auto;display:block;border:1px solid #ddd;border-radius:6px}</style></body></html>`, { waitUntil: "load" });
  const file = path.join(out, `${s.name}.webp`);
  await mkdir(path.dirname(file), { recursive: true });
  await page.screenshot({ path: file, type: "webp", quality: 82, fullPage: true });
  await page.close();
  console.log(`composed ${s.name}`);
}

for (const s of spec.shots) {
  if (s.compose) { await compose(s); continue; }
  const ctx = await browser.createBrowserContext();
  const page = await ctx.newPage();
  page.on("pageerror", (e) => console.warn(`  [${s.name}] pageerror`, String(e).slice(0, 200)));
  if (s.console) page.on("console", (m) => console.log(`  [${s.name}] console.${m.type()}:`, m.text().slice(0, 300)));
  const dpr = s.dpr ?? (s.w && s.w < 600 ? 2 : 1);
  await page.setViewport({ width: s.w ?? 1440, height: s.h ?? 900, deviceScaleFactor: dpr, isMobile: (s.w ?? 1440) < 600, hasTouch: (s.w ?? 1440) < 600 });
  await page.emulateMediaFeatures([{ name: "prefers-color-scheme", value: s.mode ?? "light" }, { name: "prefers-reduced-motion", value: s.reducedMotion ?? "no-preference" }]);
  const prefs = { palette: s.palette ?? "sacred", mode: s.mode ?? "light", appearance: s.mode ?? "light", ...(s.prefs ?? {}) };
  const reader = s.reader ?? null;
  const extraLS = s.ls ?? {};
  await page.evaluateOnNewDocument((prefs, reader, extraLS, fresh) => {
    try {
      if (!fresh) localStorage.setItem("easyquran.prefs", JSON.stringify(prefs));
      if (reader) localStorage.setItem("easyquran.reader", JSON.stringify(reader));
      for (const [k, v] of Object.entries(extraLS)) localStorage.setItem(k, typeof v === "string" ? v : JSON.stringify(v));
    } catch {}
  }, prefs, reader, extraLS, !!s.fresh);
  try {
    await page.goto(base + s.url, { waitUntil: "networkidle2", timeout: 90000 });
  } catch (e) {
    console.warn(`  [${s.name}] goto`, String(e).slice(0, 160));
  }
  await settle(page);
  await sleep(s.delay ?? 600);
  await runActions(page, s.actions);
  await settle(page);
  if (s.probe) {
    const res = await page.evaluate(s.probe);
    const txt = typeof res === "string" ? res : JSON.stringify(res, null, 1);
    console.log(`=== probe ${s.name}\n${txt}`);
  }
  if (s.axe) {
    await page.evaluate(axeSrc + "\n;window.axe = axe;");
    const res = await page.evaluate(async () => {
      // eslint-disable-next-line no-undef
      const r = await axe.run(document, { resultTypes: ["violations"], runOnly: { type: "tag", values: ["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa", "best-practice"] } });
      return r.violations.map((v) => ({ id: v.id, impact: v.impact, help: v.help, n: v.nodes.length, nodes: v.nodes.slice(0, 6).map((n) => ({ t: n.target.join(" "), s: (n.failureSummary ?? "").split("\n").slice(1, 3).join(" | ").slice(0, 220), html: n.html.slice(0, 160) })) }));
    });
    await writeFile(path.join(out, `${s.name}.axe.json`), JSON.stringify(res, null, 1));
    console.log(`=== axe ${s.name}: ${res.map((v) => `${v.id}(${v.impact},${v.n})`).join(", ") || "clean"}`);
  }
  if (!s.noShot) {
    await annotate(page, s.annotate);
    const file = path.join(out, `${s.name}.${s.type ?? "webp"}`);
    await mkdir(path.dirname(file), { recursive: true });
    const opts = { path: file, type: s.type ?? "webp", fullPage: !!s.full };
    if ((s.type ?? "webp") !== "png") opts.quality = s.quality ?? 82;
    if (s.selector) {
      const el = await page.$(s.selector);
      if (el) await el.screenshot(opts);
      else console.warn(`  [${s.name}] selector miss ${s.selector}`);
    } else {
      if (s.clip) opts.clip = s.clip;
      await page.screenshot(opts);
    }
    console.log(`shot ${s.name}`);
  }
  await ctx.close();
}
await browser.close();
