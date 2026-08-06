# EasyQuran SSR runtime benchmark — design

Static, reproducible load benchmark of the **same `adapter-node` build** executed on three JS
runtimes, measuring how the translated-page SSR path and its disk-TTL HTML cache behave under
realistic, popularity-skewed traffic.

Status: **design** (not yet implemented). Everything below is decided; nothing here changes app
behavior — the harness is read-only against `web/` and `rust/` except for env vars and a
disposable cache dir.

---

## 1. What this answers

1. Requests/sec and latency distribution per runtime at a fixed set of offered rates.
2. Cost of a translated-page **cache miss** (full SSR + Axum range fetch) vs **cache hit**
   (disk read) — and the ratio at which each runtime falls over.
3. How TTL expiry, LRU budget eviction, and Zipf popularity interact — specifically whether a hot
   head starves the long tail out of the cache.
4. Whether bun (1.3.14 / 1.4) can run this production server at all, and at what cost.

**Non-goals.** Absolute capacity numbers (generator shares the host), CDN/edge behavior, browser
render, Rust API benchmarking (Axum is a fixed dependency here, kept out of saturation on purpose).

---

## 2. Topology

```
┌─ same Mac (16 cores / 128 GB) ───────────────────────────────────────────┐
│                                                                          │
│  vegeta  ──HTTP/1.1 keep-alive──▶  web server (one runtime at a time)     │
│  (targets streamed from a                 :3100                          │
│   pre-generated Zipf list)          node ./web/server.ts                 │
│                                     bun  ./web/server.ts                 │
│                                          │                               │
│                                          │ INTERNAL_QURAN_API_BASE       │
│                                          ▼                               │
│                                 Axum (cargo release) :8888               │
│                                 started once, pre-warmed, shared         │
│                                          │                               │
│                                          ▼                               │
│                              db/quran/tanzil/**.sqlite (read-only)       │
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
restarted between runtimes. Its own CPU use is sampled and reported so a saturated upstream is
visible rather than silently attributed to the runtime.

---

## 3. Runtime matrix

| slot | binary | source |
| --- | --- | --- |
| `node24` | `node` v24.11.1 | already installed |
| `bun13` | `bun` 1.3.14 | already installed |
| `bun14` | `bench/.tools/bun-1.4/bin/bun` | downloaded from `oven-sh/bun` GitHub releases (darwin-aarch64), gitignored, never on PATH, never replaces the installed bun |

All three execute the **identical** `web/build` output produced by one `pnpm build`
(`PUBLIC_ENV=prod`). The build is produced once, its build-id recorded, and never rebuilt mid-matrix
— the disk cache key is namespaced by SvelteKit's build id, so a rebuild would silently invalidate
every scenario's cache.

### bun compat shim

`web/src/lib/server/quran-node-query-runner.ts` uses `node:sqlite` (Node 24 native). If bun cannot
resolve it, the harness injects a **bench-only** preload (`bench/shims/bun-node-sqlite.ts`, loaded
via `bun --preload`) that maps `node:sqlite` onto `bun:sqlite`. App code is untouched. The report
states explicitly that bun rows used a different SQLite binding — that is a real, disclosed
difference in the comparison, not a hidden one. If bun still cannot boot, that result is recorded
as a finding and the runtime is skipped for the remainder of the matrix.

---

## 4. Route families — separate suites, never blended

Each family is its own benchmark run with its own targets and its own report row.

| suite | URL shape | what it exercises |
| --- | --- | --- |
| `translated-surah` | `/app/<slug>/t/<lang>/<translator>[/page/<n>]` | SSR + disk cache + Axum range fetch |
| `translated-page` | `/app/t/<lang>/<translator>/page/<n>` | same, global-page keyspace (604) |
| `translated-juz` | `/app/t/<lang>/<translator>/juz/<n>` | same, largest payloads (30 juz) |
| `arabic-prerendered` | `/app/<slug>`, `/app/page/<n>`, `/app/juz/<n>` | adapter-node static file serving, no SSR |
| `immutable-assets` | `/_app/immutable/**` | static throughput ceiling / header path |
| `data-json` | `?__data.json` client navs | SvelteKit data path (explicitly **not** cached by hooks) |
| `text-endpoints` | `/[slug].md`, `/[slug].txt`, `/llms.txt`, `/sitemap.xml` | crawler/LLM surface |

`arabic-prerendered` + `immutable-assets` serve as the control: they share the runtime and HTTP
stack but skip SSR entirely, so translated-page cost is `translated-* minus control`.

---

## 5. Popularity model

Zipf over a realistic key space, deterministic (seeded, same key stream for every runtime — this is
what makes the comparison an A/B rather than three separate experiments).

**Key = (translation source × navigation index).** Sampled independently:

- **Translation weight** — Zipf(α = 1.0) over the 115 catalogue ids, ordered by a hand-set realism
  ranking: `en.sahih`, `en.pickthall`, `en.yusufali`, `ur.jalandhry`, `id.indonesian`,
  `tr.diyanet`, `fr.hamidullah`, `bn.bengali`, `ru.kuliev`, `es.cortes` … then the remaining 105 by
  catalogue order. Head ≈ 5 ids take ~45% of traffic; the tail is never zero.
- **Index weight** — Zipf(α = 0.8) over the family's index space, re-ranked so the known-popular
  units come first: surahs `1, 2, 18, 36, 55, 67, 112, 113, 114`, juz `1, 30, 29`, pages `1, 2, 582+`.
- **Local page** — for multi-page surahs, page 1 gets 55%, remaining pages share the rest Zipf-wise
  (readers open at the top and drift down).

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
  Reported per stage: *offered* rate vs *achieved* rate, error taxonomy (conn refused, timeout,
  reset, non-2xx), and latency at achieved rate.
- Generator connection cap `-max-workers` tuned per stage (≤ 4,096) so vegeta sheds load instead of
  exhausting file descriptors; `ulimit -n` raised to 65,536 in the recipe and the effective value
  recorded. Without the cap, macOS fd limits would masquerade as server failure.

---

## 7. Scenarios (cache behaviors)

Each runs against `translated-surah` by default; `--deep` extends the set to the other suites.

| id | setup | question |
| --- | --- | --- |
| `cold` | cache dir wiped, no warmup priming | miss-storm cost; how much SSR+Axum a runtime sustains with 0% hit ratio |
| `warm` | cache pre-primed with the **distinct keys the stage's Zipf stream actually touches** (not the 76k key space — priming that is 5+ min per run), TTL 7 d, budget 256 MiB | hit-path ceiling; disk read + HTTP throughput |
| `zipf-steady` | cache starts empty, natural Zipf traffic, prod TTL/budget | realistic hit-ratio curve over time; the headline number |
| `ttl-expiry` | `QURAN_SSR_CACHE_TTL_MS=15000`, primed cache | re-render waves as entries age out mid-ladder |
| `lru-evict` | `QURAN_SSR_CACHE_BUDGET_BYTES=16MiB` (~forces churn), Zipf traffic | eviction rate, write amplification, whether budget enforcement itself costs |
| `tail-starvation` | shrunk budget + Zipf(α=1.3) hot head, tail requests tagged separately | **does the hot head evict the tail?** Hit ratio reported separately for head / body / tail cohorts |
| `stampede` | primed single key, TTL set to expire exactly at t=0, N concurrent requests for that one key | do concurrent misses collapse to one render, or does every request render? (Today's `hooks.server.ts` has no single-flight — this scenario is expected to expose that, and quantifies it) |
| `compression` | `warm` run twice: `Accept-Encoding: gzip, br` vs identity | compression cost as a runtime differentiator (bun vs node zlib) |

Between every scenario: server killed, cache dir wiped, fresh dir created, server restarted, health
probe polled until ready.

---

## 8. Isolation protocol (full rigor)

- Fresh web server process + wiped `QURAN_SSR_CACHE_DIR` per scenario.
- Axum: started once, pre-warmed, shared, sampled.
- **3 repeats per (runtime × suite × scenario), interleaved** `node24 → bun13 → bun14 → node24 → …`
  so thermal drift and background-load drift hit all three equally. **Median** of the 3 reported,
  with min/max spread — a spread > 10% on any cell is flagged in the report as untrustworthy.
- 10 s warmup discarded, 30 s cooldown between scenarios.
- Pre-flight gate, aborts the run if violated: load average below threshold, both ports free, disk
  free > 5 GB, `ulimit -n` raised, no other `node`/`bun`/`vite` process running, AC power connected,
  Low Power Mode off.
- Host facts recorded once per run: `sysctl` CPU/mem, macOS version, thermal pressure sampled
  during the run (`pmset -g therm`), runtime versions, git SHA, SvelteKit build id.

### Profiles

| profile | scope | repeats | ladder | wall clock |
| --- | --- | --- | --- | --- |
| `quick` | `translated-surah` × `zipf-steady` | 1 | 3 stages × 12 s | ~5 min |
| `15m` | `translated-surah` × `cold`,`warm`,`zipf-steady` | 1 | 4 stages (200 / 2k / 10k / 50k) × 8 s, 2 s gaps, 5 s warmup, 5 s cooldown | **~10.5 min** |
| `full` | `translated-surah` × `cold`,`warm`,`zipf-steady` | 3 | 6 stages × 12 s | ~80 min |
| `deep` | `full` + the 5 cache scenarios on `translated-surah` + 1 throughput pass per remaining suite | 3 (2 for deep-only cells) | 6 stages × 12 s | ~3.8 h (2.5 h at 2 repeats) |

The `15m` profile is a **directional** ranking tool, not a publishable measurement: single pass
means no spread, so every cell is emitted with `"confidence": "unverified"` and the report renders
those cells muted with an explicit banner. p999 is suppressed entirely at this profile — 8 s at
200 req/s is ~1,600 samples, far too few. It does still produce the cold-vs-warm miss-cost delta,
the hit-ratio curve, saturation point, and mem/CPU traces for all three runtimes.

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
delta); RPS-vs-p99 knee per runtime; head/body/tail hit ratio for `tail-starvation`; achieved-rate
ceiling per runtime; bytes-written per eviction cycle.

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

`report.html` carries: matrix summary table, RPS-vs-latency knee curves per runtime, achieved-vs-
offered rate bars per stage, hit-ratio-over-time lines per scenario, head/body/tail starvation
chart, memory/CPU traces, and a plain-language findings section. Theme-aware, no external assets.

---

## 11. Layout

```
bench/
├── README.md              # this file
├── .gitignore             # .tools/ .run/ results/
├── .tools/                # bun 1.4 (downloaded); vegeta via brew
├── .run/                  # per-scenario cache dirs, pid files, target files (disposable)
├── shims/bun-node-sqlite.ts
├── src/
│   ├── config.ts          # runtimes, suites, scenarios, ladder, seeds
│   ├── keyspace.ts        # catalogue + metadata load, family index spaces
│   ├── zipf.ts            # seeded Zipf sampler (deterministic, unit-tested)
│   ├── targets.ts         # vegeta targets-file generator
│   ├── server.ts          # spawn/health-probe/kill web + axum, env matrix, port guards
│   ├── sampler.ts         # 1 Hz process/health sampling
│   ├── runner.ts          # matrix driver: interleave, repeat, warmup, cooldown, preflight
│   ├── collect.ts         # vegeta report parsing → results.json
│   └── report.ts          # results.json → report.html
└── results/
```

Driven by `just`:

```
just bench-setup      # fetch bun 1.4, brew install vegeta, build web (prod) + axum (release), verify
just bench-quick      # quick profile   — ~5 min
just bench-15         # 15m profile     — ~10.5 min, directional
just bench            # full profile    — ~80 min, publishable
just bench-deep       # deep profile    — ~3.8 h
just bench-report     # rebuild report.html from an existing results dir
```

---

## 12. Known limits, stated up front

- **Generator shares the host.** All numbers are *relative between runtimes*, not absolute capacity.
  Stages where vegeta CPU% is the ceiling are marked generator-bound in the report and excluded from
  runtime conclusions.
- **100k RPS will not be reached** for SSR suites on this topology. Those stages measure overload
  and failure behavior, which is the point of keeping them.
- **bun rows may use `bun:sqlite`** via the bench shim — disclosed per row, never silently merged.
- **Axum is in the path** for every translated miss. Its CPU is reported; if it saturates, that
  stage's conclusion is about the pair, not the runtime.
- **12 s stages** weaken p999 confidence at 100 req/s (~1,200 samples). p999 is reported but marked
  low-confidence below stage 3.
- Quran data is untouched: read-only sqlite, no hashing anywhere in the harness, cache dirs are
  disposable HTML only. Per `AGENTS.MD`, no SHA-256 over Quran data in any bench path.
```
