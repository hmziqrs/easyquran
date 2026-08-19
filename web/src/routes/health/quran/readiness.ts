import { json } from "@sveltejs/kit";

const NO_STORE = { "cache-control": "no-store" } as const;

export function readinessResponse(manifest: boolean, writable: boolean): Response {
  if (manifest && writable) {
    return json({ ready: true }, { headers: NO_STORE });
  }
  return json({ ready: false }, { status: 503, headers: NO_STORE });
}
