import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import type { StageResult } from "./runner.ts";

interface Meta {
  at: string;
  gitSha: string;
  buildId: string;
  cpus: number;
  cpuModel?: string;
  memGb: number;
  platform: string;
  profile: { id: string; repeats: number };
  runtimes: { id: string; version?: string }[];
  note?: string;
}

const web = (r: StageResult) => r.proc?.[`web:${r.runtime}`];

const fmt = (n: number, digits = 1): string =>
  Number.isFinite(n) ? n.toLocaleString("en-US", { maximumFractionDigits: digits }) : "—";

function chart(results: StageResult[], runtimes: string[], group: string): string {
  const rows = results.filter((r) => `${r.suite} · ${r.scenario}` === group);
  if (rows.length === 0) return "";
  const valid = rows.filter((r) => !r.generatorBound);
  if (valid.length === 0) return "";
  const maxRate = Math.max(...valid.map((r) => r.achievedRate));
  const maxRss = Math.max(...valid.map((r) => web(r)?.peakRssMb ?? 0));
  const w = 560;
  const h = 260;
  const pad = 44;
  const x = (rate: number): number => pad + (rate / maxRate) * (w - pad - 12);
  const y = (mb: number): number => h - pad - (mb / maxRss) * (h - pad - 16);
  const colors = ["var(--c1)", "var(--c2)", "var(--c3)"];

  const series = runtimes
    .map((id, i) => {
      const points = rows
        .filter((r) => r.runtime === id && !r.generatorBound)
        .sort((a, b) => a.stage - b.stage)
        .map((r) => `${x(r.achievedRate)},${y(web(r)?.peakRssMb ?? 0)}`);
      if (points.length === 0) return "";
      return `<polyline fill="none" stroke="${colors[i]}" stroke-width="2" points="${points.join(" ")}"/>
        ${points.map((p) => `<circle cx="${p.split(",")[0]}" cy="${p.split(",")[1]}" r="3.5" fill="${colors[i]}"/>`).join("")}`;
    })
    .join("");

  return `<figure><figcaption>${group} — achieved RPS vs peak server RSS</figcaption>
  <svg viewBox="0 0 ${w} ${h}" role="img" aria-label="${group} throughput versus peak resident memory by runtime">
    <line x1="${pad}" y1="${h - pad}" x2="${w - 12}" y2="${h - pad}" stroke="currentColor" opacity=".3"/>
    <line x1="${pad}" y1="16" x2="${pad}" y2="${h - pad}" stroke="currentColor" opacity=".3"/>
    <text x="${pad}" y="${h - 14}" font-size="11" fill="currentColor" opacity=".65">0</text>
    <text x="${w - 60}" y="${h - 14}" font-size="11" fill="currentColor" opacity=".65">${fmt(maxRate, 0)} rps</text>
    <text x="6" y="24" font-size="11" fill="currentColor" opacity=".65">${fmt(maxRss, 0)} MB</text>
    ${series}
  </svg>
  <div class="legend">${runtimes.map((id, i) => `<span><i style="background:${colors[i]}"></i>${id}</span>`).join("")}</div>
  </figure>`;
}

export function buildReport(dir: string): string {
  const meta = JSON.parse(readFileSync(path.join(dir, "meta.json"), "utf8")) as Meta;
  const results = JSON.parse(readFileSync(path.join(dir, "results.json"), "utf8")) as StageResult[];
  const runtimes = [...new Set(results.map((r) => r.runtime))];
  const groups = [...new Set(results.map((r) => `${r.suite} · ${r.scenario}`))];

  const table = (group: string): string => {
    const rows = results
      .filter((r) => `${r.suite} · ${r.scenario}` === group)
      .sort((a, b) => a.stage - b.stage || a.runtime.localeCompare(b.runtime));
    return `<table><thead><tr>
      <th>runtime</th><th>offered</th><th>achieved</th>
      <th>cpu% mean</th><th>cpu% peak</th><th>rss peak</th><th>rss end</th>
      <th>p50</th><th>p99</th><th>ok%</th><th>hit%</th><th>verdict</th>
    </tr></thead><tbody>${rows
      .map(
        (r) => `<tr class="${r.generatorBound ? "void" : ""}">
      <td>${r.runtime}</td>
      <td class="n">${fmt(r.offeredRate, 0)}</td>
      <td class="n">${fmt(r.achievedRate, 0)}</td>
      <td class="n">${fmt(web(r)?.meanCpu ?? Number.NaN, 0)}</td>
      <td class="n">${fmt(web(r)?.peakCpu ?? Number.NaN, 0)}</td>
      <td class="n">${fmt(web(r)?.peakRssMb ?? Number.NaN, 0)} MB</td>
      <td class="n">${fmt(web(r)?.endRssMb ?? Number.NaN, 0)} MB</td>
      <td class="n">${fmt(r.latencyMs.p50)}</td>
      <td class="n">${fmt(r.latencyMs.p99)}</td>
      <td class="n ${r.successRatio < 0.99 ? "bad" : ""}">${fmt(r.successRatio * 100)}</td>
      <td class="n">${r.cacheHitRatio === null ? "—" : fmt(r.cacheHitRatio * 100, 0)}</td>
      <td class="err">${r.generatorBound ? "generator-bound — ranks nothing" : r.errors.length > 0 ? "server errors" : "valid"}</td>
    </tr>`,
      )
      .join("")}</tbody></table>`;
  };

  return `<title>EasyQuran SSR runtime benchmark — ${meta.profile.id}</title>
<style>
  :root { --bg:#fff; --fg:#16181d; --muted:#666; --line:#e3e5ea; --c1:#2563eb; --c2:#d97706; --c3:#059669; --bad:#dc2626; }
  @media (prefers-color-scheme: dark) { :root { --bg:#0f1115; --fg:#e7e9ee; --muted:#9aa0ac; --line:#262a33; --c1:#60a5fa; --c2:#fbbf24; --c3:#34d399; --bad:#f87171; } }
  :root[data-theme="dark"] { --bg:#0f1115; --fg:#e7e9ee; --muted:#9aa0ac; --line:#262a33; --c1:#60a5fa; --c2:#fbbf24; --c3:#34d399; --bad:#f87171; }
  :root[data-theme="light"] { --bg:#fff; --fg:#16181d; --muted:#666; --line:#e3e5ea; --c1:#2563eb; --c2:#d97706; --c3:#059669; --bad:#dc2626; }
  body { background:var(--bg); color:var(--fg); font:15px/1.55 ui-sans-serif,system-ui,-apple-system,sans-serif; margin:0; padding:2.5rem 1.25rem 5rem; }
  main { max-width:60rem; margin:0 auto; }
  h1 { font-size:1.6rem; margin:0 0 .35rem; letter-spacing:-.02em; }
  h2 { font-size:1.15rem; margin:2.5rem 0 .75rem; letter-spacing:-.01em; }
  .sub { color:var(--muted); margin:0 0 1.5rem; font-size:.9rem; }
  .banner { border:1px solid var(--line); border-left:3px solid var(--c2); padding:.7rem .9rem; border-radius:6px; margin:0 0 1.5rem; font-size:.88rem; color:var(--muted); }
  .facts { display:grid; grid-template-columns:repeat(auto-fit,minmax(9rem,1fr)); gap:.75rem; margin:0 0 1rem; }
  .facts div { border:1px solid var(--line); border-radius:6px; padding:.55rem .7rem; }
  .facts dt { color:var(--muted); font-size:.72rem; text-transform:uppercase; letter-spacing:.04em; }
  .facts dd { margin:.15rem 0 0; font-size:.9rem; font-variant-numeric:tabular-nums; }
  .scroll { overflow-x:auto; }
  table { border-collapse:collapse; width:100%; font-size:.86rem; }
  th, td { text-align:left; padding:.4rem .55rem; border-bottom:1px solid var(--line); white-space:nowrap; }
  th { color:var(--muted); font-weight:600; font-size:.75rem; text-transform:uppercase; letter-spacing:.03em; }
  td.n { text-align:right; font-variant-numeric:tabular-nums; }
  td.bad { color:var(--bad); }
  td.err { color:var(--muted); font-size:.78rem; max-width:18rem; overflow:hidden; text-overflow:ellipsis; }
  tr.void td { opacity:.45; text-decoration:line-through; text-decoration-color:var(--line); }
  tr.void td.err { text-decoration:none; color:var(--bad); opacity:.8; }
  figure { margin:1.25rem 0 0; }
  figcaption { color:var(--muted); font-size:.8rem; margin-bottom:.35rem; }
  svg { width:100%; height:auto; border:1px solid var(--line); border-radius:6px; }
  .legend { display:flex; gap:1rem; margin-top:.5rem; font-size:.8rem; color:var(--muted); }
  .legend i { display:inline-block; width:.7rem; height:.7rem; border-radius:2px; margin-right:.35rem; vertical-align:-1px; }
  code { font-family:ui-monospace,SFMono-Regular,monospace; font-size:.85em; }
</style>
<main>
  <h1>EasyQuran SSR runtime benchmark</h1>
  <p class="sub">Same <code>adapter-node</code> build, three runtimes · profile <code>${meta.profile.id}</code> · ${new Date(meta.at).toLocaleString()}</p>
  ${meta.note ? `<p class="banner"><strong>Directional only.</strong> ${meta.note}</p>` : ""}
  <dl class="facts">
    <div><dt>host</dt><dd>${meta.cpus}× ${meta.cpuModel ?? "cpu"} · ${meta.memGb} GB</dd></div>
    <div><dt>platform</dt><dd>${meta.platform}</dd></div>
    <div><dt>git</dt><dd>${meta.gitSha}</dd></div>
    <div><dt>build id</dt><dd>${meta.buildId}</dd></div>
    ${meta.runtimes.map((r) => `<div><dt>${r.id}</dt><dd>${r.version ?? "?"}</dd></div>`).join("")}
  </dl>
  ${groups
    .map(
      (group) => `<h2>${group}</h2>
      <div class="scroll">${table(group)}</div>
      ${chart(results, runtimes, group)}`,
    )
    .join("")}
  <h2>Reading this</h2>
  <p class="sub">Load generator shares the host with both servers, so numbers rank runtimes against
  each other — they are not absolute capacity. <code>hit%</code> comes from <code>/health/quran</code>
  counter deltas, never from response headers. Upstream is a release-build Axum on :8899, started once
  and shared by every runtime.</p>
</main>`;
}

if (process.argv[2]) {
  const dir = path.resolve(process.argv[2]);
  const html = buildReport(dir);
  writeFileSync(path.join(dir, "report.html"), html, "utf8");
  console.log(`[report] ${path.join(dir, "report.html")}`);
}
