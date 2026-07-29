/* ════════════════════════════════════════════════════════════════════════
   owner.ts — SERVER-ONLY. Fetches the maker/owner profile once and exposes
   only the render-safe {email, x, xHandle} subset to the app.

   Why server-only ($lib/server/*): the full payload (websites, CV, github,
   linkedin, instagram, …) must never reach the browser. SvelteKit hard-
   excludes this module from the client bundle; the only permitted importer
   is a +layout.server.ts load.

   The marketing site is fully prerendered, so this runs exactly once per
   `vp build` (module-level memo + single-flight) and the result is baked
   into static HTML — the browser never calls hmziq.rs. A fetch failure can
   never throw (a throw in a prerender load fails the build), so every error
   path falls back to hardcoded personal values.
   ════════════════════════════════════════════════════════════════════════ */

import { env } from "$env/dynamic/private";
import type { OwnerPublic } from "$lib/types/owner";

const DEFAULT_SOURCE_URL = "https://hmziq.rs/me.json";
/** Optional override (e.g. for staging); falls back to the canonical URL. */
const OWNER_SOURCE_URL = env.OWNER_SOURCE_URL || DEFAULT_SOURCE_URL;
const FETCH_TIMEOUT_MS = 5_000;
/** Cache TTL — decorative for a prerender build (one fetch either way), but
 *  bounds reuse if this module is later called from a runtime server. */
const CACHE_TTL_MS = 60 * 60 * 1000;

/** Full payload shape from hmziq.rs/me.json. Held server-side only. */
export interface OwnerProfile {
  username: string;
  name: string;
  title: string;
  tagline: string;
  yearsOfExperience: number;
  email: string;
  websites: { alternative: string; portfolio: string; cv: string };
  social: { github: string; linkedin: string; twitter: string; instagram: string };
}

/**
 * Rendered when the build-time fetch fails, so the contact surface is stable
 * regardless of hmziq.rs availability. These are the personal values — NOT the
 * project's salam@easyquran.app address — so an outage can't resurrect the
 * address we deliberately dropped. Only `email` + `social.twitter` are
 * load-bearing (everything else is rendered only on future opt-in).
 */
const FALLBACK_PROFILE: OwnerProfile = {
  username: "hmziqrs",
  name: "hmziqrs",
  title: "",
  tagline: "",
  yearsOfExperience: 0,
  email: "hmziqrs@gmail.com",
  websites: { alternative: "", portfolio: "https://hmziq.rs", cv: "" },
  social: { github: "hmziqrs", linkedin: "hmziqrs", twitter: "hmziqrs", instagram: "hmziqrs" },
};

let cached: { profile: OwnerProfile; expires: number } | null = null;
let inflight: Promise<OwnerProfile> | null = null;

function isOwnerProfile(d: unknown): d is OwnerProfile {
  return (
    !!d &&
    typeof (d as OwnerProfile).email === "string" &&
    !!d &&
    typeof (d as OwnerProfile).social?.twitter === "string"
  );
}

/** Fetch + memoize the full profile. Never throws — falls back on any error. */
export async function fetchOwnerProfile(opts?: { force?: boolean }): Promise<OwnerProfile> {
  if (!opts?.force && cached && cached.expires > Date.now()) return cached.profile;

  // Single-flight: concurrent callers (e.g. parallel prerender loads) share
  // one in-flight request rather than stampeding hmziq.rs.
  if (!inflight || opts?.force) {
    inflight = (async () => {
      const ctrl = new AbortController();
      const timer = setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS);
      try {
        const res = await fetch(OWNER_SOURCE_URL, { signal: ctrl.signal });
        if (!res.ok) throw new Error(`owner fetch HTTP ${res.status}`);
        const data: unknown = await res.json();
        if (!isOwnerProfile(data)) throw new Error("owner payload shape invalid");
        cached = { profile: data, expires: Date.now() + CACHE_TTL_MS };
        return cached.profile;
      } catch (err) {
        console.warn("[owner] fetch failed, using fallback profile:", err);
        cached = { profile: FALLBACK_PROFILE, expires: Date.now() + CACHE_TTL_MS };
        return FALLBACK_PROFILE;
      } finally {
        clearTimeout(timer);
        inflight = null;
      }
    })();
  }
  return inflight;
}

/** The render-safe subset — the ONLY owner shape returned to the client. */
export async function getOwnerPublic(): Promise<OwnerPublic> {
  const p = await fetchOwnerProfile();
  return {
    email: p.email,
    x: `https://x.com/${p.social.twitter}`,
    xHandle: `@${p.social.twitter}`,
  };
}

/** Full profile, for future server-side opt-in rendering. Not wired to any load. */
export async function getOwnerProfile(): Promise<OwnerProfile> {
  return fetchOwnerProfile();
}
