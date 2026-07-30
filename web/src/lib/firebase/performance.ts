/* ════════════════════════════════════════════════════════════════════════
   firebase/performance.ts — Firebase Performance Monitoring.

   Browser-only, loaded LAZILY. Init is gated by the user's performance-consent
   flag (set at init time — the SDK cannot be fully torn down after the fact, so
   we simply don't initialize it when the user has opted out). Provides a
   `trace()` helper and an `instrument()` wrapper to measure async code paths.

   Automatic instrumentation (page-load + network-request traces) is on by
   default once initialized; pass { instrumentationEnabled: false } via consent
   to suppress the automatic parts.
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import type { FirebasePerformance, PerformanceTrace } from "firebase/performance";
import { isConfigured, initApp } from "./index";

let perf: FirebasePerformance | null = null;
let initPromise: Promise<FirebasePerformance | null> | null = null;

export interface PerfInitOptions {
  /** Collect custom + automatic traces. Defaults true. */
  dataCollectionEnabled?: boolean;
  /** Auto-instrument page load + network requests. Defaults true. */
  instrumentationEnabled?: boolean;
}

/**
 * Start Performance Monitoring, exactly once. Browser-only; no-op during SSR or
 * when unconfigured. Pass consent-derived flags so an opted-out user's session
 * is never instrumented in the first place.
 */
export function initPerformance(opts: PerfInitOptions = {}): Promise<FirebasePerformance | null> {
  if (!browser || !isConfigured) return Promise.resolve(null);
  if (!initPromise) {
    initPromise = (async () => {
      try {
        const core = await initApp();
        if (!core) return null;
        const { initializePerformance } = await import("firebase/performance");
        // initializePerformance may only be called once per app; the flags apply
        // for the lifetime of this page session.
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

/** The shared FirebasePerformance instance once initialized, else null. */
export function getPerf(): FirebasePerformance | null {
  return perf;
}

/**
 * Create and start a named custom trace. Returns the trace (call `.stop()` when
 * the measured work is done) or null if performance isn't initialized.
 *
 *   const t = await startTrace("reader_load");
 *   try { … } finally { await t?.stop(); }
 */
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

/**
 * Measure an async function end-to-end as a custom trace, recording failures as
 * a trace attribute so you can break down latency by outcome. No-op (just runs
 * the function) when performance is unavailable.
 *
 *   const surahs = await instrument("surah_search", () => search(q));
 */
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

/*
 * NOTE: there is intentionally NO runtime collection toggle here. The modular
 * `@firebase/performance` SDK applies its dataCollection/instrumentation flags
 * only at `initializePerformance()` (once per app) and exports no setter, so a
 * mid-session opt-out cannot stop traces that already started. Collection is
 * therefore gated SOLELY by the consent flag passed to initPerformance() at load
 * (see +layout.svelte). Changing the Performance setting reloads the page so the
 * next mount honours the new flag — the Tweaks control does exactly that.
 */
