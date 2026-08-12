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

export function asNumberRecord(raw: unknown, min: number, max: number): Record<string, number> {
  const obj = asObject(raw);
  if (!obj) return {};
  const out: Record<string, number> = {};
  for (const [k, v] of Object.entries(obj)) {
    const n = asNumber(v, min, max);
    if (n !== undefined && Number.isSafeInteger(n)) out[k] = n;
  }
  return out;
}

export function asArray<T>(raw: unknown, decodeItem: (item: unknown) => T | undefined): T[] {
  if (!Array.isArray(raw)) return [];
  const out: T[] = [];
  for (const item of raw) {
    const decoded = decodeItem(item);
    if (decoded !== undefined) out.push(decoded);
  }
  return out;
}
