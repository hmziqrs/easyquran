import path from "node:path";
import { fileURLToPath } from "node:url";

export const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
export const BENCH = path.join(REPO, "bench");
export const RUN_DIR = path.join(BENCH, ".run");
export const RESULTS_DIR = path.join(BENCH, "results");
export const TOOLS = path.join(BENCH, ".tools");

export const WEB_PORT = 3100;
/** Not 8888: a dev API commonly holds that port, and a debug build would dominate every miss. */
export const API_PORT = 8899;
export const WEB_ORIGIN = `http://127.0.0.1:${WEB_PORT}`;
export const API_BASE = `http://127.0.0.1:${API_PORT}/quran`;

export interface Runtime {
  readonly id: string;
  readonly bin: string;
  readonly args: readonly string[];
  version?: string;
}

export const RUNTIMES: readonly Runtime[] = [
  { id: "node24", bin: "node", args: [] },
  { id: "bun13", bin: "bun", args: ["--bun"] },
  {
    id: "bun14-canary",
    bin: path.join(TOOLS, "bun-darwin-aarch64/bun"),
    args: ["--bun"],
  },
];

export interface Stage {
  readonly rate: number;
  readonly durationSec: number;
}

export interface Profile {
  readonly id: string;
  readonly suites: readonly string[];
  readonly scenarios: readonly string[];
  readonly repeats: number;
  readonly stages: readonly Stage[];
  readonly warmupSec: number;
  readonly gapSec: number;
  readonly cooldownSec: number;
}

export const PROFILES: Record<string, Profile> = {
  quick: {
    id: "quick",
    suites: ["translated-surah"],
    scenarios: ["zipf-steady"],
    repeats: 1,
    stages: [
      { rate: 100, durationSec: 12 },
      { rate: 1000, durationSec: 12 },
      { rate: 5000, durationSec: 12 },
    ],
    warmupSec: 5,
    gapSec: 3,
    // Long enough for the kernel to drain TIME_WAIT sockets before the next runtime starts;
    // 5s was not, and the third runtime died on EADDRNOTAVAIL.
    cooldownSec: 20,
  },
  "15m": {
    id: "15m",
    suites: ["translated-surah"],
    scenarios: ["cold", "warm", "zipf-steady"],
    repeats: 1,
    stages: [
      { rate: 200, durationSec: 8 },
      { rate: 2000, durationSec: 8 },
      { rate: 10000, durationSec: 8 },
      { rate: 50000, durationSec: 8 },
    ],
    warmupSec: 5,
    gapSec: 2,
    cooldownSec: 20,
  },
  html: {
    id: "html",
    suites: ["translated-surah", "arabic-prerendered"],
    scenarios: ["zipf-steady"],
    repeats: 1,
    stages: [
      { rate: 100, durationSec: 12 },
      { rate: 1000, durationSec: 12 },
      { rate: 5000, durationSec: 12 },
    ],
    warmupSec: 5,
    gapSec: 3,
    cooldownSec: 20,
  },
  full: {
    id: "full",
    suites: ["translated-surah"],
    scenarios: ["cold", "warm", "zipf-steady"],
    repeats: 3,
    stages: [
      { rate: 100, durationSec: 12 },
      { rate: 1000, durationSec: 12 },
      { rate: 5000, durationSec: 12 },
      { rate: 10000, durationSec: 12 },
      { rate: 25000, durationSec: 12 },
      { rate: 100000, durationSec: 12 },
    ],
    warmupSec: 10,
    gapSec: 3,
    cooldownSec: 30,
  },
};

/** Cache env per scenario. Undefined values fall back to app defaults. */
export function scenarioEnv(scenario: string, cacheDir: string): Record<string, string> {
  const base = { QURAN_SSR_CACHE_DIR: cacheDir };
  switch (scenario) {
    case "ttl-expiry":
      return { ...base, QURAN_SSR_CACHE_TTL_MS: "15000" };
    case "lru-evict":
    case "tail-starvation":
      return { ...base, QURAN_SSR_CACHE_BUDGET_BYTES: String(16 * 1024 * 1024) };
    default:
      return base;
  }
}

export const SEED = 0x5eed_1234;
/**
 * Capped well below the old 4096: unbounded workers exhausted macOS ephemeral ports, which showed
 * up as client-side `bind: can't assign requested address` and voided the whole stage.
 */
export const MAX_WORKERS = 1000;
export const REQUEST_TIMEOUT = "10s";
