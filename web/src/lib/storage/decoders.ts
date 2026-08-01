/* ════════════════════════════════════════════════════════════════════════
   decoders.ts — tiny type-narrowing helpers for decoding `unknown` localStorage
   blobs into validated domain data.

   Each domain store owns its own `decode()` (it knows its schema and defaults),
   but the mechanical `typeof`/range/literal/record narrowing is identical
   across reader/prefs/consent/notifications. Centralizing these removes the
   repeated boilerplate without forcing a shared schema library. Every helper
   returns `undefined` (or an empty record) for anything that does not match so
   callers can "keep the default" by simply skipping an `undefined` field.
   ════════════════════════════════════════════════════════════════════════ */

export function asObject(raw: unknown): Record<string, unknown> | undefined {
  if (raw && typeof raw === "object" && !Array.isArray(raw)) {
    return raw as Record<string, unknown>;
  }
  return undefined;
}

export function asNumber(raw: unknown, min: number, max: number): number | undefined {
  return typeof raw === "number" && Number.isFinite(raw) && raw >= min && raw <= max
    ? raw
    : undefined;
}

export function asString(raw: unknown): string | undefined {
  return typeof raw === "string" ? raw : undefined;
}

export function asLiteral<T extends string>(raw: unknown, options: readonly T[]): T | undefined {
  if (typeof raw === "string" && (options as readonly string[]).includes(raw)) {
    return raw as T;
  }
  return undefined;
}

export function asNullableObject(raw: unknown): "null" | "object" | undefined {
  if (raw === null) return "null";
  if (raw && typeof raw === "object" && !Array.isArray(raw)) return "object";
  return undefined;
}

export function asBooleanRecord(raw: unknown): Record<string, boolean> {
  const obj = asObject(raw);
  if (!obj) return {};
  const out: Record<string, boolean> = {};
  for (const [k, v] of Object.entries(obj)) {
    if (v === true) out[k] = true;
  }
  return out;
}

export function asStringRecord(raw: unknown): Record<string, string> {
  const obj = asObject(raw);
  if (!obj) return {};
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(obj)) {
    if (typeof v === "string") out[k] = v;
  }
  return out;
}
