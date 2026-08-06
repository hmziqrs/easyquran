import { execFile, execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import {
  MAX_WORKERS,
  PROFILES,
  REPO,
  REQUEST_TIMEOUT,
  RESULTS_DIR,
  RUNTIMES,
  RUN_DIR,
  SEED,
  WEB_ORIGIN,
  scenarioEnv,
  type Profile,
  type Runtime,
  type Stage,
} from "./config.ts";
import {
  cacheStats,
  control,
  primeCache,
  resetCacheDir,
  startApi,
  startWeb,
  statsDelta,
  type CacheStats,
  type Proc,
} from "./server.ts";
import { Sampler } from "./sampler.ts";
import { buildTargets } from "./targets.ts";

const exec = promisify(execFile);
const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

/** SvelteKit build id — namespaces every disk-cache key, so it must be identical across runtimes. */
let appBuildId = "unknown";

export interface StageResult {
  runtime: string;
  suite: string;
  scenario: string;
  repeat: number;
  stage: number;
  offeredRate: number;
  durationSec: number;
  achievedRate: number;
  successRatio: number;
  statuses: Record<string, number>;
  latencyMs: { mean: number; p50: number; p90: number; p95: number; p99: number; max: number };
  bytesIn: number;
  errors: string[];
  cache: CacheStats;
  cacheHitRatio: number | null;
  /** True when the load generator, not the server, was the limit — the stage ranks nothing. */
  generatorBound: boolean;
  /** Per-process CPU and RSS over exactly this stage's window. The headline metric. */
  proc: Record<string, { peakRssMb: number; endRssMb: number; meanCpu: number; peakCpu: number }>;
}

/** Client-side socket exhaustion on the generator host, not server rejection. */
const GENERATOR_FAULTS = [
  "can't assign requested address",
  "cannot assign requested address",
  "too many open files",
];

export function isGeneratorBound(errors: readonly string[]): boolean {
  return errors.some((e) => GENERATOR_FAULTS.some((fault) => e.toLowerCase().includes(fault)));
}

interface VegetaReport {
  latencies: { mean: number; "50th": number; "90th": number; "95th": number; "99th": number; max: number };
  bytes_in: { total: number };
  rate: number;
  throughput: number;
  requests: number;
  success: number;
  status_codes: Record<string, number>;
  errors: string[];
}

function attack(targetsFile: string, stage: Stage, outFile: string): VegetaReport {
  execFileSync(
    "sh",
    [
      "-c",
      `vegeta attack -targets=${targetsFile} -rate=${stage.rate} -duration=${stage.durationSec}s ` +
        `-max-workers=${MAX_WORKERS} -connections=${MAX_WORKERS} -timeout=${REQUEST_TIMEOUT} -keepalive ` +
        `-header 'Accept-Encoding: gzip, br' ` +
        `-header 'User-Agent: easyquran-bench/1.0' > ${outFile}`,
    ],
    { stdio: ["ignore", "inherit", "inherit"], maxBuffer: 1 << 28 },
  );
  const json = execFileSync("sh", ["-c", `vegeta report -type=json ${outFile}`], {
    maxBuffer: 1 << 28,
  }).toString();
  return JSON.parse(json) as VegetaReport;
}

async function runOne(
  runtime: Runtime,
  suite: string,
  scenario: string,
  repeat: number,
  profile: Profile,
  dirs: { raw: string; samples: string; logs: string },
  apiPid: number,
): Promise<StageResult[]> {
  const cacheDir = path.join(RUN_DIR, "cache", `${runtime.id}-${suite}-${scenario}`);
  resetCacheDir(cacheDir);

  const web = await startWeb(
    runtime,
    scenarioEnv(scenario, cacheDir),
    path.join(dirs.logs, `${runtime.id}-${suite}-${scenario}-${repeat}.log`),
  );
  const health = (await control(`${WEB_ORIGIN}/health/quran`).then((r) => r.json())) as {
    appBuildId: string;
  };
  if (appBuildId === "unknown") appBuildId = health.appBuildId;
  else if (appBuildId !== health.appBuildId) {
    throw new Error(
      `[bench] build id changed mid-matrix (${appBuildId} → ${health.appBuildId}) — cache keys are namespaced by it, results would not be comparable`,
    );
  }

  const sampler = new Sampler(
    path.join(dirs.samples, `${runtime.id}-${suite}-${scenario}-${repeat}.ndjson`),
    new Map([
      [web.pid, `web:${runtime.id}`],
      [apiPid, "axum"],
    ]),
  );
  sampler.start();

  const results: StageResult[] = [];
  try {
    // Warmup: never measured, only there to JIT the render path and fill the API's translation pool.
    const warmupFile = path.join(RUN_DIR, "targets", `${runtime.id}-warmup.txt`);
    const warmup = buildTargets(suite, profile.stages[0]!.rate * profile.warmupSec, warmupFile, SEED);
    attack(warmupFile, { rate: profile.stages[0]!.rate, durationSec: profile.warmupSec },
      path.join(RUN_DIR, "warmup.bin"));

    if (scenario === "warm") {
      const stream = buildTargets(
        suite,
        profile.stages.reduce((n, s) => n + s.rate * s.durationSec, 0),
        path.join(RUN_DIR, "targets", `${runtime.id}-prime.txt`),
        SEED,
      );
      const primed = await primeCache(stream.distinct);
      console.log(`    primed ${primed}/${stream.distinct.length} distinct keys`);
    } else if (scenario === "cold") {
      // Cold means cold: throw away everything the warmup put on disk. The cache holds no
      // in-memory index (every get() stats the file), so wiping the dir is enough — no restart.
      resetCacheDir(cacheDir);
    }
    void warmup;

    for (const [index, stage] of profile.stages.entries()) {
      const targetsFile = path.join(
        RUN_DIR,
        "targets",
        `${suite}-${scenario}-s${index}-${runtime.id}.txt`,
      );
      buildTargets(suite, stage.rate * stage.durationSec, targetsFile, SEED + index);
      const before = await cacheStats();
      const startedAt = Date.now();
      const binFile = path.join(
        dirs.raw,
        `${runtime.id}__${suite}__${scenario}__r${repeat}__s${index}.bin`,
      );
      const report = attack(targetsFile, stage, binFile);
      const endedAt = Date.now();
      const after = await cacheStats();
      const delta = statsDelta(before, after);
      const served = delta.hits + delta.misses;

      results.push({
        runtime: runtime.id,
        suite,
        scenario,
        repeat,
        stage: index,
        offeredRate: stage.rate,
        durationSec: stage.durationSec,
        achievedRate: report.throughput,
        successRatio: report.success,
        statuses: report.status_codes,
        latencyMs: {
          mean: report.latencies.mean / 1e6,
          p50: report.latencies["50th"] / 1e6,
          p90: report.latencies["90th"] / 1e6,
          p95: report.latencies["95th"] / 1e6,
          p99: report.latencies["99th"] / 1e6,
          max: report.latencies.max / 1e6,
        },
        bytesIn: report.bytes_in.total,
        errors: [...new Set(report.errors ?? [])].slice(0, 5),
        cache: delta,
        cacheHitRatio: served > 0 ? delta.hits / served : null,
        generatorBound: isGeneratorBound(report.errors ?? []),
        proc: sampler.window(startedAt, endedAt),
      });
      const proc = sampler.window(startedAt, endedAt);
      const web = proc[`web:${runtime.id}`];
      console.log(
        `    stage ${index} @${stage.rate}/s → ${report.throughput.toFixed(0)}/s ` +
          `cpu=${web ? web.meanCpu.toFixed(0) : "?"}% rss=${web ? web.peakRssMb.toFixed(0) : "?"}MB ` +
          `p50=${(report.latencies["50th"] / 1e6).toFixed(1)}ms ` +
          `p99=${(report.latencies["99th"] / 1e6).toFixed(1)}ms ` +
          `ok=${(report.success * 100).toFixed(1)}% ` +
          `hit=${served > 0 ? ((delta.hits / served) * 100).toFixed(0) : "-"}%` +
          (isGeneratorBound(report.errors ?? []) ? " [GENERATOR-BOUND]" : ""),
      );
      if (index < profile.stages.length - 1) await sleep(profile.gapSec * 1000);
    }
  } finally {
    sampler.stop();
    await web.stop();
  }
  return results;
}

async function hostFacts(): Promise<Record<string, unknown>> {
  const git = await exec("git", ["rev-parse", "--short", "HEAD"], { cwd: REPO }).catch(() => ({
    stdout: "unknown",
  }));
  return {
    at: new Date().toISOString(),
    gitSha: git.stdout.trim(),
    buildId: appBuildId,
    cpus: os.cpus().length,
    cpuModel: os.cpus()[0]?.model,
    memGb: Math.round(os.totalmem() / 1024 ** 3),
    platform: `${os.type()} ${os.release()}`,
    loadavg: os.loadavg(),
  };
}

async function main(): Promise<void> {
  const profileId = process.argv[2] ?? "quick";
  const profile = PROFILES[profileId];
  if (!profile) throw new Error(`[bench] unknown profile ${profileId}`);

  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  const outDir = path.join(RESULTS_DIR, `${stamp}__${profile.id}`);
  const dirs = {
    raw: path.join(outDir, "raw"),
    samples: path.join(outDir, "samples"),
    logs: path.join(outDir, "logs"),
  };
  for (const dir of [outDir, ...Object.values(dirs), path.join(RUN_DIR, "targets")]) {
    mkdirSync(dir, { recursive: true });
  }

  const runtimes: Runtime[] = [];
  for (const runtime of RUNTIMES) {
    const version = await exec(runtime.bin, ["--version"]).catch(() => null);
    if (!version) {
      console.warn(`[bench] skipping ${runtime.id} — binary not runnable (${runtime.bin})`);
      continue;
    }
    runtimes.push({ ...runtime, version: version.stdout.trim() });
  }
  if (runtimes.length === 0) throw new Error("[bench] no runnable runtimes");

  console.log(`[bench] profile=${profile.id} runtimes=${runtimes.map((r) => r.id).join(",")}`);
  console.log("[bench] starting axum (release)…");
  const api: Proc = await startApi(path.join(dirs.logs, "axum.log"));

  const results: StageResult[] = [];
  try {
    // Interleaved repeats: runtime order rotates per repeat so thermal drift hits all three equally.
    for (let repeat = 0; repeat < profile.repeats; repeat += 1) {
      for (const suite of profile.suites) {
        for (const scenario of profile.scenarios) {
          for (const runtime of runtimes) {
            console.log(`\n[bench] ${runtime.id} · ${suite} · ${scenario} · repeat ${repeat + 1}`);
            results.push(
              ...(await runOne(runtime, suite, scenario, repeat, profile, dirs, api.pid)),
            );
            await sleep(profile.cooldownSec * 1000);
          }
        }
      }
    }
  } finally {
    await api.stop();
  }

  const meta = {
    ...(await hostFacts()),
    profile,
    runtimes: runtimes.map((r) => ({ id: r.id, bin: r.bin, version: r.version })),
    seed: SEED,
    origin: WEB_ORIGIN,
    note:
      profile.repeats < 2
        ? "single pass per cell — no spread, treat every number as directional"
        : undefined,
  };
  writeFileSync(path.join(outDir, "meta.json"), JSON.stringify(meta, null, 2), "utf8");
  writeFileSync(path.join(outDir, "results.json"), JSON.stringify(results, null, 2), "utf8");
  console.log(`\n[bench] done → ${outDir}`);
}

await main();
