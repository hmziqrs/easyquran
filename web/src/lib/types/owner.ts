/* ════════════════════════════════════════════════════════════════════════
   owner.ts — the ONLY owner shape ever serialized into page/client data.

   The full profile (websites, CV, github, etc.) lives server-side only —
   see $lib/server/owner.ts — and is never returned by any load. This shared
   type is client-safe, so components and app.d.ts can import it without
   crossing the $lib/server/* boundary (which SvelteKit forbids).
   ════════════════════════════════════════════════════════════════════════ */

export interface OwnerPublic {
  email: string;
  /** Absolute https://x.com/<handle> URL. */
  x: string;
  /** Display handle, e.g. "@hmziqrs". */
  xHandle: string;
}
