// Build-time (prerender) load: fetches the owner profile server-side once and
// exposes only the render-safe {email, x, xHandle} subset to every marketing
// page + the footer. `prerender` is inherited from ./+layout.ts, so this runs
// at `vp build` and the result is baked into static HTML — the browser never
// calls hmziq.rs.
import { getOwnerPublic } from "$lib/server/owner";
import type { OwnerPublic } from "$lib/types/owner";

export async function load(): Promise<{ owner: OwnerPublic }> {
  return { owner: await getOwnerPublic() };
}
