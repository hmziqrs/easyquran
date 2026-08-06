import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SAMPLE_PROC = path.join(path.dirname(fileURLToPath(import.meta.url)), "sample-proc.ts");

export interface ProcSample {
  t: number;
  label: string;
  /** Percent of ONE core. 400 = four cores saturated. */
  cpu: number;
  rssMb: number;
  cpuSeconds: number;
}

export interface ProcWindow {
  peakRssMb: number;
  endRssMb: number;
  meanCpu: number;
  peakCpu: number;
}

/**
 * Out-of-process 1 Hz sampler. The runner blocks on `execFileSync` during an attack, so sampling
 * must not share its event loop.
 */
export class Sampler {
  readonly #file: string;
  #child: ChildProcess | undefined;

  constructor(file: string, targets: Map<number, string>) {
    this.#file = file;
    mkdirSync(path.dirname(file), { recursive: true });
    this.#targets = targets;
  }

  readonly #targets: Map<number, string>;

  start(): void {
    if (this.#child) return;
    this.#child = spawn(
      process.execPath,
      [SAMPLE_PROC, this.#file, ...[...this.#targets].map(([pid, label]) => `${pid}:${label}`)],
      { stdio: ["ignore", "ignore", "inherit"] },
    );
  }

  stop(): void {
    this.#child?.kill("SIGTERM");
    this.#child = undefined;
  }

  #read(): ProcSample[] {
    if (!existsSync(this.#file)) return [];
    return readFileSync(this.#file, "utf8")
      .trim()
      .split("\n")
      .filter(Boolean)
      .map((line) => JSON.parse(line) as ProcSample);
  }

  /** Samples inside [from, to), summarized per label. */
  window(from: number, to: number): Record<string, ProcWindow> {
    const all = this.#read();
    const out: Record<string, ProcWindow> = {};
    for (const label of new Set(all.map((s) => s.label))) {
      const rows = all.filter((s) => s.label === label && s.t >= from && s.t < to);
      if (rows.length === 0) continue;
      const cpus = rows.map((r) => r.cpu).filter((n) => Number.isFinite(n));
      out[label] = {
        peakRssMb: Math.max(...rows.map((r) => r.rssMb)),
        endRssMb: rows.at(-1)!.rssMb,
        meanCpu: cpus.length > 0 ? cpus.reduce((a, b) => a + b, 0) / cpus.length : Number.NaN,
        peakCpu: cpus.length > 0 ? Math.max(...cpus) : Number.NaN,
      };
    }
    return out;
  }
}
