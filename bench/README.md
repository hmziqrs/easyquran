# EasyQuran SSR runtime benchmark — design

Static, reproducible load benchmark of the production web server — SvelteKit 3 + adapter-bun,
served by `bun ./web/server.ts` — measuring how the translated-page SSR path and its disk-TTL
HTML cache behave under realistic, popularity-skewed traffic.

Status: **design** (a harness skeleton exists under `bench/src/`; no `just bench-*` recipes
yet). Everything here is design; nothing changes app behavior — the harness is read-only
against `web/` and `rust/` except for env vars and a disposable cache dir.

**Amendment (2026-10, SvelteKit 3 migration).** The original design compared node and bun
executing one `adapter-node` build. Production is now adapter-bun: `web/server.ts` is a custom
`Bun.serve` entry over the adapter's `build/adapter-bun.js` hand-off, and the build itself runs
under bun (`pnpm build` → `bun run --bun vp build`; `adapt()` calls `Bun.build`). Node cannot
execute that output, so the cross-runtime A/B is dead. Facts below (URL shapes, adapter names,
build command) are updated; sections that still encode the dead premise (§3 matrix, §8
interleave) are marked historical pending a redesign call — single-runtime characterization or
a bun-version A/B — which this document does not make.

---

## 1. What this answers

1. Requests/sec and latency distribution of the production bun server at a fixed set of
   offered rates.
2. Cost of a translated-page **cache miss** (full SSR + Axum range fetch) vs **cache hit**
   (disk read) — and the rate at which that server falls over.
3. How TTL expiry, LRU budget eviction, and Zipf popularity interact — specifically whether a hot
   head starves the long tail out of the cache.
4. Settled by the SvelteKit 3 migration: bun runs this production server (adapter-bun,
   `bun ./web/server.ts`). The open residue is where that one runtime saturates and how the
   disk-TTL cache behaves on it.

**Non-goals.** Absolute capacity numbers (generator shares the host), CDN/edge behavior, browser
render, Rust API benchmarking (Axum is a fixed dependency here, kept out of saturation on purpose).

---

## 2. Topology

```
┌─ same Mac (16 cores / 128 GB) ───────────────────────────────────────────┐
│                                                                          │
│  vegeta  ──HTTP/1.1 keep-alive──▶  web server (bun; see §3)              │
│  (targets streamed from a                 :3100                          │
│   pre-generated Zipf list)          bun ./web/server.ts                  │
│                                     (adapter-bun hand-off; see §3)       │
│                                          │                               │
│                                          │ INTERNAL_QURAN_API_BASE       │
│                                          ▼                               │
│                                 Axum (cargo release) :8888               │
│                                 started once, pre-warmed, shared         │
│                                          │                               │
│                                          ▼                               │
│                              db/quran/**.sqlite (read-only)       │
│                                                                          │
│  disk cache: bench/.run/cache/<scenario>/  (QURAN_SSR_CACHE_DIR)         │
└──────────────────────────────────────────────────────────────────────────┘
```

**Port discipline.** The repo's `justfile` sets `dotenv-load := true` and root `.env` carries
`PORT=8888` (Axum). The web server reads the same `PORT` var, so bench recipes **must** pass
`PORT=3100` explicitly or the web server collides with Axum. Harness asserts both ports are free
before each run and fails loudly otherwise.

**Axum is a fixed constant, not a variable.** Built once with `cargo build -p ruxlog --release`,
started once for the whole matrix, pre-warmed with the full hot-key set before stage 1, and never
restarted across scenarios. Its own CPU use is sampled and reported so a saturated upstream is
visible rather than silently attributed to the web server.

---

## 3. Runtime matrix

**Historical design, invalidated by the SvelteKit 3 migration.** The matrix compared `node24`
and `bun14` executing one `adapter-node` build. Production is now adapter-bun — `web/server.ts`
is a custom `Bun.serve` entry over the adapter's `build/adapter-bun.js` hand-off — and node
cannot execute that output (`Bun.serve`/`Bun.file` and the adapter's own runtime are bun-only).
`bench/src/config.ts` still lists both slots and must be reworked before any run; the redesign
call — single-runtime characterization of production bun, or a bun-version A/B — is deliberately
not made here.

| slot    | binary                                             | source            |
| ------- | -------------------------------------------------- | ----------------- |
| `bun14` | `bun` ≥ 1.4 (deployed image `oven/bun:1.4.2-slim`) | already installed |

What survives unchanged: the build is produced once by one `pnpm build` (which runs
`bun run --bun vp build` under `PUBLIC_ENV=prod`), its build-id recorded, and never rebuilt
mid-matrix — the disk cache key is namespaced by SvelteKit's build id (`build-${version}` in
`web/src/lib/server/quran-disk-cache.ts`), so a rebuild would silently invalidate every
scenario's cache.

### bun compat shim (retired)

The original design preloaded a bench-only `bench/shims/bun-node-sqlite.ts` mapping `node:sqlite`
onto `bun:sqlite` in case bun could not resolve it. Moot twice over: bun 1.4+ implements
`node:sqlite` natively (`DatabaseSync` opens the corpus — the production Arabic prerender
already reads sqlite under bun), and the app now runs on bun in production, so there is no
second binding to disclose. App code stays untouched either way.

---

## 4. Route families — separate suites, never blended

Each family is its own benchmark run with its own targets and its own report row.

| suite                | URL shape                                                | what it exercises                                                   |
| -------------------- | -------------------------------------------------------- | ------------------------------------------------------------------- |
| `translated-surah`   | `/<slug>/t/<lang>/<translator>`                          | SSR + disk cache + Axum range fetch                                 |
| `translated-page`    | `/t/<lang>/<translator>/page/<n>`                        | same, global-page keyspace (604)                                    |
| `translated-juz`     | `/t/<lang>/<translator>/juz/<n>`                         | same, largest payloads (30 juz)                                     |
| `arabic-prerendered` | `/<slug>`, `/page/<n>`, `/juz/<n>`                       | adapter-bun static file serving (native `Bun.serve` routes), no SSR |
| `immutable-assets`   | `/_app/immutable/**`                                     | static throughput ceiling / header path                             |
| `data-json`          | `?__data.json` client navs                               | SvelteKit data path (explicitly **not** cached by hooks)            |
| `text-endpoints`     | `/[slug].md`, `/[slug].txt`, `/llms.txt`, `/sitemap.xml` | crawler/LLM surface                                                 |

Reader paths are scheme A — prefix-less, no `/app` marker (legacy shapes 308) — and one URL per
surah (no surah-local page tails); see `docs/quran-system.md` Part 5. Targets that use legacy
shapes would measure the redirect table, not the reader.

`arabic-prerendered` + `immutable-assets` serve as the control: they share the runtime and HTTP
stack but skip SSR entirely, so translated-page cost is `translated-* minus control`.

---

## 5. Popularity model

Zipf over a realistic key space, deterministic (seeded, same key stream for every scenario and
repeat — this is what makes runs comparable rather than separate experiments).

**Key = (translation source × navigation index).** Sampled independently:

- **Translation weight** — Zipf(α = 1.0) over the 115 catalogue ids, ordered by a hand-set realism
  ranking: `en.sahih`, `en.pickthall`, `en.yusufali`, `ur.jalandhry`, `id.indonesian`,
  `tr.diyanet`, `fr.hamidullah`, `bn.bengali`, `ru.kuliev`, `es.cortes` … then the remaining 105 by
  catalogue order. Head ≈ 5 ids take ~45% of traffic; the tail is never zero.
- **Index weight** — Zipf(α = 0.8) over the family's index space, re-ranked so the known-popular
  units come first: surahs `1, 2, 18, 36, 55, 67, 112, 113, 114`, juz `1, 30, 29`, pages `1, 2, 582+`.
- **Local page** — retired as a URL dimension (one URL per surah since the navigation audit's
  M5; page drift is client-side virtualization on the surah root). Page popularity now lives
  only in the global-page families above.

Targets are pre-generated to a plain vegeta targets file per (suite × scenario × stage) with the
exact request count that stage needs, then streamed with `-lazy`. Pre-generation keeps sampling
cost out of the measurement loop and makes every run byte-identical replayable.

Key-space sizes recorded in the report: ~115 × 662 ≈ 76k translated surah-pages, 115 × 604 ≈ 69k
global pages, 115 × 30 = 3,450 juz.

---

## 6. Rate ladder

Open model, constant arrival rate per stage, **12 s per stage** (chosen over 25 s to keep the full
matrix inside ~2 h; p999 at the lowest rate is correspondingly weaker and flagged as such in the
report).

```
stage:  1      2       3       4        5        6
rate:   100 → 1,000 → 5,000 → 10,000 → 25,000 → 100,000   req/s (target/offered)
```

- 10 s warmup at stage-1 rate before stage 1, discarded.
- 3 s inter-stage gap; 30 s cooldown between scenarios (thermal).
- **Saturation policy: record and continue.** Every stage runs to 100k regardless of failure.
  Reported per stage: _offered_ rate vs _achieved_ rate, error taxonomy (conn refused, timeout,
  reset, non-2xx), and latency at achieved rate.
- Generator connection cap `-max-workers` tuned per stage (≤ 4,096) so vegeta sheds load instead of
  exhausting file descriptors; `ulimit -n` raised to 65,536 in the recipe and the effective value
  recorded. Without the cap, macOS fd limits would masquerade as server failure.

---

## 7. Scenarios (cache behaviors)

Each runs against `translated-surah` by default; `--deep` extends the set to the other suites.

| id                | setup                                                                                                                                                                  | question                                                                                                                                                                                  |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cold`            | cache dir wiped, no warmup priming                                                                                                                                     | miss-storm cost; how much SSR+Axum the server sustains with 0% hit ratio                                                                                                                  |
| `warm`            | cache pre-primed with the **distinct keys the stage's Zipf stream actually touches** (not the 76k key space — priming that is 5+ min per run), TTL 7 d, budget 256 MiB | hit-path ceiling; disk read + HTTP throughput                                                                                                                                             |
| `zipf-steady`     | cache starts empty, natural Zipf traffic, prod TTL/budget                                                                                                              | realistic hit-ratio curve over time; the headline number                                                                                                                                  |
| `ttl-expiry`      | `QURAN_SSR_CACHE_TTL_MS=15000`, primed cache                                                                                                                           | re-render waves as entries age out mid-ladder                                                                                                                                             |
| `lru-evict`       | `QURAN_SSR_CACHE_BUDGET_BYTES=16MiB` (~forces churn), Zipf traffic                                                                                                     | eviction rate, write amplification, whether budget enforcement itself costs                                                                                                               |
| `tail-starvation` | shrunk budget + Zipf(α=1.3) hot head, tail requests tagged separately                                                                                                  | **does the hot head evict the tail?** Hit ratio reported separately for head / body / tail cohorts                                                                                        |
| `stampede`        | primed single key, TTL set to expire exactly at t=0, N concurrent requests for that one key                                                                            | do concurrent misses collapse to one render, or does every request render? (Today's `hooks.server.ts` has no single-flight — this scenario is expected to expose that, and quantifies it) |
| `compression`     | `warm` run twice: `Accept-Encoding: gzip, br` vs identity                                                                                                              | compression cost on the warm hit path (gzip/br vs identity)                                                                                                                               |

Between every scenario: server killed, cache dir wiped, fresh dir created, server restarted, health
probe polled until ready.

---

## 8. Isolation protocol (full rigor)

- Fresh web server process + wiped `QURAN_SSR_CACHE_DIR` per scenario.
- Axum: started once, pre-warmed, shared, sampled.
- **3 repeats per (runtime × suite × scenario), interleaved** (historical interleave
  `node24 → bun14 → node24 → …`; meaningless until §3's redesign call lands) so thermal drift
  and background-load drift hit every slot equally. **Median** of the 3 reported,
  with min/max spread — a spread > 10% on any cell is flagged in the report as untrustworthy.
- 10 s warmup discarded, 30 s cooldown between scenarios.
- Pre-flight gate, aborts the run if violated: load average below threshold, both ports free, disk
  free > 5 GB, `ulimit -n` raised, no other `node`/`bun`/`vite` process running, AC power connected,
  Low Power Mode off.
- Host facts recorded once per run: `sysctl` CPU/mem, macOS version, thermal pressure sampled
  during the run (`pmset -g therm`), runtime versions, git SHA, SvelteKit build id.

### Profiles

| profile | scope                                                                                        | repeats                   | ladder                                                                    | wall clock                  |
| ------- | -------------------------------------------------------------------------------------------- | ------------------------- | ------------------------------------------------------------------------- | --------------------------- |
| `quick` | `translated-surah` × `zipf-steady`                                                           | 1                         | 3 stages × 12 s                                                           | ~5 min                      |
| `15m`   | `translated-surah` × `cold`,`warm`,`zipf-steady`                                             | 1                         | 4 stages (200 / 2k / 10k / 50k) × 8 s, 2 s gaps, 5 s warmup, 5 s cooldown | **~10.5 min**               |
| `full`  | `translated-surah` × `cold`,`warm`,`zipf-steady`                                             | 3                         | 6 stages × 12 s                                                           | ~80 min                     |
| `deep`  | `full` + the 5 cache scenarios on `translated-surah` + 1 throughput pass per remaining suite | 3 (2 for deep-only cells) | 6 stages × 12 s                                                           | ~3.8 h (2.5 h at 2 repeats) |

The `15m` profile is a **directional** ranking tool, not a publishable measurement: single pass
means no spread, so every cell is emitted with `"confidence": "unverified"` and the report renders
those cells muted with an explicit banner. p999 is suppressed entirely at this profile — 8 s at
200 req/s is ~1,600 samples, far too few. It does still produce the cold-vs-warm miss-cost delta,
the hit-ratio curve, saturation point, and mem/CPU traces for the server.

**Deep scope is per-suite, not cross-product.** The five extra cache scenarios (`ttl-expiry`,
`lru-evict`, `tail-starvation`, `stampede`, `compression`) run on `translated-surah` only — running
them across all seven suites is a ~10 h matrix that answers nothing the single suite doesn't. The
other two translated suites and the four control suites each get one `zipf-steady` pass.

---

## 9. Metrics

**From vegeta (per stage):** offered rate, achieved rate, success ratio, status-code histogram,
bytes in/out, latency p50 / p90 / p95 / p99 / p999 / max / mean, full latency histogram buckets.

**From the app:**

- `X-EasyQuran-Quran-Cache: hit|miss` and `Server-Timing: quran_ssr_cache` — sampled on a
  low-rate side-channel prober (1 req/s alongside the load) so header inspection never taxes the
  main generator.
- `/health/quran` polled before/after every stage → deltas for `hits`, `misses`, `writes`,
  `evictions`, `errors`, `entries`, `bytes`. **This is the authoritative hit-ratio source**;
  vegeta never parses response headers.

**From the OS (1 Hz sampler):** web-server RSS + CPU%, Axum RSS + CPU%, vegeta CPU% (so
generator-bound stages are identifiable), system load average, cache-dir size on disk.

**Derived:** hit-ratio-over-time per stage; cost-per-miss (`p50 miss − p50 hit` from the cold/warm
delta); RPS-vs-p99 knee for the server; head/body/tail hit ratio for `tail-starvation`;
achieved-rate ceiling; bytes-written per eviction cycle.

---

## 10. Output

```
bench/results/<timestamp>/
├── meta.json           # host, versions, git sha, build id, profile, seed
├── raw/                # vegeta .bin + .json per (runtime × suite × scenario × stage × repeat)
├── samples/            # 1 Hz process + health-endpoint samples (ndjson)
├── results.json        # normalized, joined, median-reduced — the machine-readable artifact
└── report.html         # self-contained page (inline CSS/JS/SVG), published via Artifact
```

`report.html` carries: matrix summary table, RPS-vs-latency knee curves, achieved-vs-
offered rate bars per stage, hit-ratio-over-time lines per scenario, head/body/tail starvation
chart, memory/CPU traces, and a plain-language findings section. Theme-aware, no external assets.

---

## 11. Layout

```
bench/
├── README.md              # this file
├── .gitignore             # .tools/ .run/ results/
├── .tools/                # vegeta via brew
├── .run/                  # per-scenario cache dirs, pid files, target files (disposable)
├── shims/bun-node-sqlite.ts   # retired by the kit 3 migration — never created
├── tsconfig.json          # harness-only TS config (node types, explicit .ts imports, noEmit)
├── src/
│   ├── config.ts          # runtimes, suites, scenarios, ladder, seeds
│   ├── keyspace.ts        # catalogue + metadata load, family index spaces
│   ├── zipf.ts            # seeded Zipf sampler (deterministic, unit-tested)
│   ├── targets.ts         # vegeta targets-file generator
│   ├── server.ts          # spawn/health-probe/kill web + axum, env matrix, port guards
│   ├── sampler.ts         # 1 Hz process/health sampling
│   ├── runner.ts          # matrix driver: interleave, repeat, warmup, cooldown, preflight; vegeta report → results.json
│   ├── sample-proc.ts     # standalone 1 Hz ps-sampler process (runner's loop blocks during attacks)
│   └── report.ts          # results.json → report.html
└── results/
```

Driven by `just`:

```
just bench-setup      # brew install vegeta, build web (prod) + axum (release), verify
just bench-quick      # quick profile   — ~5 min
just bench-15         # 15m profile     — ~10.5 min, directional
just bench            # full profile    — ~80 min, publishable
just bench-deep       # deep profile    — ~3.8 h
just bench-report     # rebuild report.html from an existing results dir
```

---

## 12. Known limits, stated up front

- **Generator shares the host.** All numbers are _relative_ (across scenarios and stages), not
  absolute capacity. Stages where vegeta CPU% is the ceiling are marked generator-bound in the
  report and excluded from conclusions about the server.
- **100k RPS will not be reached** for SSR suites on this topology. Those stages measure overload
  and failure behavior, which is the point of keeping them.
- **The bun shim is retired** — every row exercises the same `node:sqlite` binding bun runs in
  production, so there is no per-row binding difference to disclose.
- **Axum is in the path** for every translated miss. Its CPU is reported; if it saturates, that
  stage's conclusion is about the pair, not the server.
- **12 s stages** weaken p999 confidence at 100 req/s (~1,200 samples). p999 is reported but marked
  low-confidence below stage 3.
- Quran data is untouched: read-only sqlite, no hashing anywhere in the harness, cache dirs are
  disposable HTML only. Per `AGENTS.MD`, no SHA-256 over Quran data in any bench path.
