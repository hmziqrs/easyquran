import type { UiLocale } from "#lib/i18n/locales.js";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** "5 minutes ago" / "yesterday" in the UI locale; "" for an unparsable stamp. */
export function timeAgo(iso: string, locale: UiLocale, nowMs: number = Date.now()): string {
  const at = Date.parse(iso);
  if (Number.isNaN(at)) return "";
  const elapsed = Math.max(0, nowMs - at);
  const format = new Intl.RelativeTimeFormat(locale, { numeric: "auto", style: "short" });
  if (elapsed < HOUR) return format.format(-Math.max(0, Math.floor(elapsed / MINUTE)), "minute");
  if (elapsed < DAY) return format.format(-Math.floor(elapsed / HOUR), "hour");
  return format.format(-Math.floor(elapsed / DAY), "day");
}
