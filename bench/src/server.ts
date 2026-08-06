import { spawn, type ChildProcess } from "node:child_process";
import { createWriteStream, existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import net from "node:net";
import path from "node:path";
import { API_BASE, API_PORT, REPO, WEB_ORIGIN, WEB_PORT, type Runtime } from "./config.ts";

export interface Proc {
  readonly child: ChildProcess;
  readonly pid: number;
  stop(): Promise<void>;
}

const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

export function portFree(port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = net.connect({ port, host: "127.0.0.1" });
    socket.once("connect", () => {
      socket.destroy();
      resolve(false);
    });
    socket.once("error", () => resolve(true));
    socket.setTimeout(500, () => {
      socket.destroy();
      resolve(true);
    });
  });
}

/** Root .env is the API's config; the web server must never inherit its PORT (that is Axum's). */
export function dotenv(): Record<string, string> {
  const file = path.join(REPO, ".env");
  if (!existsSync(file)) return {};
  const out: Record<string, string> = {};
  for (const line of readFileSync(file, "utf8").split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const eq = trimmed.indexOf("=");
    if (eq < 0) continue;
    out[trimmed.slice(0, eq)] = trimmed.slice(eq + 1).replace(/^["']|["']$/g, "");
  }
  return out;
}

async function waitReady(url: string, timeoutMs: number, label: string): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  let lastError = "";
  while (Date.now() < deadline) {
    try {
      const res = await fetch(url, { signal: AbortSignal.timeout(2000) });
      if (res.ok) {
        await res.arrayBuffer();
        return;
      }
      lastError = `HTTP ${res.status}`;
    } catch (cause) {
      lastError = cause instanceof Error ? cause.message : String(cause);
    }
    await sleep(250);
  }
  throw new Error(`[server] ${label} never became ready (${lastError})`);
}

function wrap(child: ChildProcess, label: string): Proc {
  return {
    child,
    pid: child.pid!,
    async stop() {
      if (child.exitCode !== null || child.signalCode !== null) return;
      expectedExits.add(child.pid!);
      child.kill("SIGTERM");
      const deadline = Date.now() + 5000;
      while (child.exitCode === null && child.signalCode === null && Date.now() < deadline) {
        await sleep(50);
      }
      if (child.exitCode === null && child.signalCode === null) {
        console.warn(`[server] ${label} ignored SIGTERM, sending SIGKILL`);
        child.kill("SIGKILL");
        await sleep(200);
      }
    },
  };
}

export async function startApi(logPath: string): Promise<Proc> {
  if (!(await portFree(API_PORT))) {
    throw new Error(`[server] port ${API_PORT} busy — stop the running API before benching`);
  }
  const bin = path.join(REPO, "rust/target/release/ruxlog");
  if (!existsSync(bin)) throw new Error(`[server] missing release binary: ${bin}`);
  const child = spawn(bin, [], {
    cwd: path.join(REPO, "rust"),
    env: { ...process.env, ...dotenv(), PORT: String(API_PORT), RUST_LOG: "warn" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  pipeTo(child, logPath, "axum");
  await waitReady(`${API_BASE}/health/ready`, 90_000, "axum");
  return wrap(child, "axum");
}

export async function startWeb(
  runtime: Runtime,
  env: Record<string, string>,
  logPath: string,
): Promise<Proc> {
  if (!(await portFree(WEB_PORT))) {
    throw new Error(`[server] port ${WEB_PORT} busy`);
  }
  const child = spawn(runtime.bin, [...runtime.args, "server.ts"], {
    cwd: path.join(REPO, "web"),
    env: {
      ...process.env,
      NODE_ENV: "production",
      HOST: "127.0.0.1",
      PORT: String(WEB_PORT),
      ORIGIN: WEB_ORIGIN,
      PUBLIC_QURAN_API_BASE: API_BASE,
      INTERNAL_QURAN_API_BASE: API_BASE,
      ...env,
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  pipeTo(child, logPath, `web:${runtime.id}`);
  await waitReady(`${WEB_ORIGIN}/health/quran`, 30_000, `web:${runtime.id}`);
  return wrap(child, `web:${runtime.id}`);
}

function pipeTo(child: ChildProcess, logPath: string, label: string): void {
  mkdirSync(path.dirname(logPath), { recursive: true });
  const log = createWriteStream(logPath, { flags: "a" });
  child.stdout?.pipe(log, { end: false });
  child.stderr?.pipe(log, { end: false });
  child.on("exit", (code, signal) => {
    log.write(`\n[bench] ${label} exited code=${code} signal=${signal}\n`);
    if (!expectedExits.has(child.pid!)) {
      console.error(`[bench] ${label} died unexpectedly (code=${code} signal=${signal}) — see ${logPath}`);
    }
  });
}

/** PIDs we asked to stop, so an expected SIGTERM is not reported as a crash. */
const expectedExits = new Set<number>();

export function resetCacheDir(dir: string): void {
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
}

export interface CacheStats {
  hits: number;
  misses: number;
  writes: number;
  evictions: number;
  errors: number;
  entries: number;
  bytes: number;
}

/**
 * Control-plane fetch. Never keep-alive: Node's server closes idle sockets after 5s, and undici
 * happily reuses one it just closed, which surfaces as a spurious ECONNRESET between stages.
 */
export async function control(url: string, attempts = 8): Promise<Response> {
  let lastError: unknown;
  for (let i = 0; i < attempts; i += 1) {
    try {
      return await fetch(url, {
        headers: { connection: "close" },
        signal: AbortSignal.timeout(10_000),
      });
    } catch (cause) {
      // EADDRNOTAVAIL means the host ran out of ephemeral ports (TIME_WAIT backlog from the
      // previous stage). Backing off linearly lets the kernel reclaim them.
      lastError = cause;
      await sleep(400 * (i + 1));
    }
  }
  throw new Error(`[server] control fetch failed after ${attempts} attempts: ${url}`, {
    cause: lastError,
  });
}

export async function cacheStats(): Promise<CacheStats> {
  const res = await control(`${WEB_ORIGIN}/health/quran`);
  const body = (await res.json()) as { translatedPageCache: CacheStats };
  return body.translatedPageCache;
}

export function statsDelta(before: CacheStats, after: CacheStats): CacheStats {
  return {
    hits: after.hits - before.hits,
    misses: after.misses - before.misses,
    writes: after.writes - before.writes,
    evictions: after.evictions - before.evictions,
    errors: after.errors - before.errors,
    entries: after.entries,
    bytes: after.bytes,
  };
}

/** Sequentially fetch each distinct key once so the disk cache is populated before a `warm` run. */
export async function primeCache(urls: readonly string[], concurrency = 8): Promise<number> {
  let index = 0;
  let ok = 0;
  const workers = Array.from({ length: concurrency }, async () => {
    while (index < urls.length) {
      const url = urls[index++]!;
      try {
        const res = await control(`${WEB_ORIGIN}${url}`, 1);
        await res.arrayBuffer();
        if (res.ok) ok += 1;
      } catch {
        /* priming failures surface as misses in the run itself */
      }
    }
  });
  await Promise.all(workers);
  return ok;
}
