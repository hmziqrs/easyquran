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

  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed))
    fail("pack root is not an object");
  const root = parsed as Record<string, unknown>;

  if (root.version !== 1) fail(`pack version unsupported: ${String(root.version)}`);

  const rawEntries = root.entries;
  if (!rawEntries || typeof rawEntries !== "object" || Array.isArray(rawEntries)) {
    fail("pack entries is not an object");
  }
  const rawBodies = root.bodies;
  if (!Array.isArray(rawBodies)) fail("pack bodies is not an array");

  const entries = rawEntries as Record<string, unknown>;
  const bodies = rawBodies as unknown[];

  for (let i = 0; i < bodies.length; i++) {
    if (typeof bodies[i] !== "string") fail(`pack body ${i} is not a string`);
  }

  for (const [key, value] of Object.entries(entries)) {
    if (!Number.isInteger(value) || (value as number) < 0 || (value as number) >= bodies.length) {
      fail(`pack entry "${key}" has invalid index`);
    }
  }

  if (Object.keys(entries).length !== bodies.length) {
    fail(`pack entries/bodies length mismatch: ${Object.keys(entries).length} != ${bodies.length}`);
  }

  return { entries: entries as Record<string, number>, bodies: bodies as string[] };
}
