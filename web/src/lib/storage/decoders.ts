export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

export type JsonRecord = Record<string, JsonValue>;

export type BooleanRecord = Record<string, boolean>;
export type StringRecord = Record<string, string>;
export type NumberRecord = Record<string, number>;

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is arbitrary JSON.parse output; this decoder is the boundary parser.
export function asObject(raw: unknown): JsonRecord | undefined {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- typeof-object is the only discriminator for an arbitrary JSON value at this parse boundary.
  if (raw && typeof raw === "object" && !Array.isArray(raw)) {
    // SAFETY: guards above proved raw is a non-null, non-array JSON object.
    return raw as JsonRecord;
  }
  return undefined;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is arbitrary JSON.parse output; this decoder is the boundary parser.
export function asNumber(raw: unknown, min: number, max: number): number | undefined {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- typeof-number is the only discriminator for an arbitrary JSON value at this parse boundary.
  return typeof raw === "number" && Number.isFinite(raw) && raw >= min && raw <= max
    ? raw
    : undefined;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is arbitrary JSON.parse output; this decoder is the boundary parser.
export function asString(raw: unknown): string | undefined {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- typeof-string is the only discriminator for an arbitrary JSON value at this parse boundary.
  return typeof raw === "string" ? raw : undefined;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is arbitrary JSON.parse output; this decoder is the boundary parser.
export function asLiteral<T extends string>(raw: unknown, options: readonly T[]): T | undefined {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- typeof-string is the only discriminator for an arbitrary JSON value at this parse boundary.
  if (typeof raw === "string" && options.some((option) => option === raw)) {
    // SAFETY: some() above proved raw equals one of options' elements, each of type T.
    return raw as T;
  }
  return undefined;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is arbitrary JSON.parse output; this decoder is the boundary parser.
export function asNullableObject(raw: unknown): "null" | "object" | undefined {
  if (raw === null) return "null";
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- typeof-object is the only discriminator for an arbitrary JSON value at this parse boundary.
  if (raw && typeof raw === "object" && !Array.isArray(raw)) return "object";
  return undefined;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is arbitrary JSON.parse output; this decoder is the boundary parser.
export function asBooleanRecord(raw: unknown) {
  const obj = asObject(raw);
  const out: BooleanRecord = {};
  if (obj) {
    for (const [k, v] of Object.entries(obj)) {
      if (v === true) out[k] = true;
    }
  }
  return out;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is arbitrary JSON.parse output; this decoder is the boundary parser.
export function asStringRecord(raw: unknown) {
  const obj = asObject(raw);
  const out: StringRecord = {};
  if (obj) {
    for (const [k, v] of Object.entries(obj)) {
      // eslint-disable-next-line anti-slop/no-runtime-typeof -- typeof-string is the only discriminator for an arbitrary JSON record value.
      if (typeof v === "string") out[k] = v;
    }
  }
  return out;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is arbitrary JSON.parse output; this decoder is the boundary parser.
export function asNumberRecord(raw: unknown, min: number, max: number) {
  const obj = asObject(raw);
  const out: NumberRecord = {};
  if (obj) {
    for (const [k, v] of Object.entries(obj)) {
      const n = asNumber(v, min, max);
      if (n !== undefined && Number.isSafeInteger(n)) out[k] = n;
    }
  }
  return out;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is arbitrary JSON.parse output and items are its untyped elements; decodeItem is the per-element parser at this boundary.
export function asArray<T>(raw: unknown, decodeItem: (item: unknown) => T | undefined): T[] {
  if (!Array.isArray(raw)) return [];
  const out: T[] = [];
  for (const item of raw) {
    const decoded = decodeItem(item);
    if (decoded !== undefined) out.push(decoded);
  }
  return out;
}
