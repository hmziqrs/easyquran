export interface Pack {
  entries: Record<string, number>;
  bodies: string[];
}

function fail(msg: string): never {
  throw new Error(`[offline] ${msg}`);
}

export function decodePack(text: string): Pack {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    fail("pack is not valid JSON");
  }

  // eslint-disable-next-line anti-slop/no-runtime-typeof -- JSON.parse boundary guard: typeof-object is the only honest way to discriminate a non-null object from primitives; Array.isArray only covers the array case.
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed))
    fail("pack root is not an object");
  // SAFETY: preceding null/typeof-object/!Array.isArray guards proved parsed is a non-null object whose fields are unknown JSON values.
  // eslint-disable-next-line anti-slop/no-unsafe-dictionary-type -- JSON.parse root bag; each field is validated by name below before use.
  const root = parsed as Record<string, unknown>;

  if (root.version !== 1) fail(`pack version unsupported: ${String(root.version)}`);

  const rawEntries = root.entries;
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- JSON.parse boundary guard for the entries field; typeof-object is the only honest primitive/object discrimination here.
  if (!rawEntries || typeof rawEntries !== "object" || Array.isArray(rawEntries)) {
    fail("pack entries is not an object");
  }
  const rawBodies = root.bodies;
  if (!Array.isArray(rawBodies)) fail("pack bodies is not an array");

  // SAFETY: preceding null/typeof-object/!Array.isArray guards proved rawEntries is a non-array non-null object whose values are unknown JSON.
  // eslint-disable-next-line anti-slop/no-unsafe-dictionary-type -- JSON.parse entries bag; every value is validated as a finite in-range integer below.
  const entries = rawEntries as Record<string, unknown>;
  const bodies = rawBodies;

  for (let i = 0; i < bodies.length; i++) {
    // eslint-disable-next-line anti-slop/no-runtime-typeof -- JSON.parse boundary check: bodies come from untrusted JSON and each element must be discriminated as string.
    if (typeof bodies[i] !== "string") fail(`pack body ${i} is not a string`);
  }

  for (const [key, value] of Object.entries(entries)) {
    if (
      // eslint-disable-next-line anti-slop/no-runtime-typeof -- JSON.parse boundary check: entry values are untrusted and must be discriminated as number before range validation.
      typeof value !== "number" ||
      !Number.isInteger(value) ||
      value < 0 ||
      value >= bodies.length
    ) {
      fail(`pack entry "${key}" has invalid index`);
    }
  }

  if (Object.keys(entries).length !== bodies.length) {
    fail(`pack entries/bodies length mismatch: ${Object.keys(entries).length} != ${bodies.length}`);
  }

  // SAFETY: every entry value was checked to be an integer in [0, bodies.length), and every body element was checked to be a string.
  return { entries: entries as Record<string, number>, bodies: bodies as string[] };
}
