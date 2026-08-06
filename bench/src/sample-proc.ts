/**
 * Standalone 1 Hz sampler. Runs as its own process because the runner blocks its event loop inside
 * `execFileSync` for the whole attack window — an in-process timer never fires while load is on.
 *
 * argv: <outFile> <pid:label> [<pid:label> …]
 */
import { execFile } from "node:child_process";
import { appendFileSync, mkdirSync } from "node:fs";
import path from "node:path";
import { promisify } from "node:util";

const exec = promisify(execFile);

const outFile = process.argv[2];
if (!outFile) throw new Error("[sampler] missing out file");
const targets = new Map<number, string>(
  process.argv.slice(3).map((arg) => {
    const [pid, ...rest] = arg.split(":");
    return [Number(pid), rest.join(":")] as const;
  }),
);
mkdirSync(path.dirname(outFile), { recursive: true });

function parseCpuTime(value: string): number {
  const parts = value.split(":").map(Number);
  if (parts.some((n) => !Number.isFinite(n))) return Number.NaN;
  return parts.reduce((acc, n) => acc * 60 + n, 0);
}

const last = new Map<number, { t: number; cpuSeconds: number }>();

async function tick(): Promise<void> {
  const pids = [...targets.keys()];
  const { stdout } = await exec("ps", ["-o", "pid=,rss=,time=", "-p", pids.join(",")]).catch(() => ({
    stdout: "",
  }));
  const t = Date.now();
  const rows: string[] = [];
  for (const line of stdout.trim().split("\n")) {
    const [pidRaw, rssRaw, timeRaw] = line.trim().split(/\s+/);
    if (!pidRaw) continue;
    const pid = Number(pidRaw);
    const cpuSeconds = parseCpuTime(timeRaw ?? "");
    const previous = last.get(pid);
    last.set(pid, { t, cpuSeconds });
    // Percent of ONE core, from cumulative CPU-time deltas. 400 = four cores saturated.
    const cpu =
      previous && t > previous.t
        ? ((cpuSeconds - previous.cpuSeconds) / ((t - previous.t) / 1000)) * 100
        : Number.NaN;
    rows.push(
      JSON.stringify({
        t,
        label: targets.get(pid) ?? `pid:${pid}`,
        cpu,
        rssMb: Number(rssRaw) / 1024,
        cpuSeconds,
      }),
    );
  }
  if (rows.length > 0) appendFileSync(outFile, `${rows.join("\n")}\n`, "utf8");
}

setInterval(() => void tick(), 1000);
void tick();
