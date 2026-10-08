// kit 3 deprecates @sveltejs/kit's `json()` response helper; Response.json()
// is the canonical constructor and carries the identical init surface.
const NO_STORE = { "cache-control": "no-store" } as const;

export function readinessResponse(manifest: boolean, writable: boolean): Response {
  if (manifest && writable) {
    return Response.json({ ready: true }, { headers: NO_STORE });
  }
  return Response.json({ ready: false }, { status: 503, headers: NO_STORE });
}
