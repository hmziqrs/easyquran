import { browser } from "$app/environment";
import type { FirebasePerformance, PerformanceTrace } from "firebase/performance";

import { isConfigured, initApp } from "./index";

let perf: FirebasePerformance | null = null;
let initPromise: Promise<FirebasePerformance | null> | null = null;

export interface PerfInitOptions {
  dataCollectionEnabled?: boolean;
  instrumentationEnabled?: boolean;
}

export function initPerformance(opts: PerfInitOptions = {}): Promise<FirebasePerformance | null> {
  if (!browser || !isConfigured) return Promise.resolve(null);
  if (!initPromise) {
    initPromise = (async () => {
      try {
        const core = await initApp();
        if (!core) return null;
        const { initializePerformance } = await import("firebase/performance");
        perf = initializePerformance(core, {
          dataCollectionEnabled: opts.dataCollectionEnabled ?? true,
          instrumentationEnabled: opts.instrumentationEnabled ?? true,
        });
      } catch (err) {
        console.warn("[firebase] performance failed to start:", err);
      }
      return perf;
    })();
  }
  return initPromise;
}

export function getPerf(): FirebasePerformance | null {
  return perf;
}

export async function startTrace(name: string): Promise<PerformanceTrace | null> {
  if (!perf) await initPerformance();
  if (!perf) return null;
  try {
    const { trace } = await import("firebase/performance");
    const t = trace(perf, name);
    t.start();
    return t;
  } catch (err) {
    console.warn(`[firebase] trace "${name}" failed:`, err);
    return null;
  }
}

export async function instrument<T>(name: string, fn: () => Promise<T>): Promise<T> {
  const t = await startTrace(name);
  try {
    const result = await fn();
    t?.putAttribute("result", "success");
    return result;
  } catch (err) {
    t?.putAttribute("result", "error");
    throw err;
  } finally {
    t?.stop();
  }
}
