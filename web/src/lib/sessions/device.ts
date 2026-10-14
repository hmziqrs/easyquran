import type { DeviceKind } from "./schema";

/**
 * Coarse device class for the handoff label ("on phone · 5 min ago"). Never a
 * fingerprint: three buckets only, derived from the UA hints the browser
 * already exposes. iPadOS reports a desktop Mac UA, so touch points break the
 * tie there.
 */
export function deviceKind(): DeviceKind {
  if (!("navigator" in globalThis)) return "desktop";
  const ua = navigator.userAgent;
  if (/iPad|Tablet|PlayBook|Silk/iu.test(ua)) return "tablet";
  if (/Android/iu.test(ua)) return /Mobile/iu.test(ua) ? "phone" : "tablet";
  if (/iPhone|iPod|Mobile|Windows Phone/iu.test(ua)) return "phone";
  if (/Macintosh/iu.test(ua) && navigator.maxTouchPoints > 1) return "tablet";
  return "desktop";
}
