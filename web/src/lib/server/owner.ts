import { env } from "$env/dynamic/private";
import type { OwnerPublic } from "$lib/types/owner";

const DEFAULT_SOURCE_URL = "https://hmziq.rs/me.json";
const OWNER_SOURCE_URL = env.OWNER_SOURCE_URL || DEFAULT_SOURCE_URL;
const FETCH_TIMEOUT_MS = 5_000;
const CACHE_TTL_MS = 60 * 60 * 1000;

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

export async function fetchOwnerProfile(opts?: { force?: boolean }): Promise<OwnerProfile> {
  if (!opts?.force && cached && cached.expires > Date.now()) return cached.profile;

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

export async function getOwnerPublic(): Promise<OwnerPublic> {
  const p = await fetchOwnerProfile();
  return {
    email: p.email,
    x: `https://x.com/${p.social.twitter}`,
    xHandle: `@${p.social.twitter}`,
  };
}

export async function getOwnerProfile(): Promise<OwnerProfile> {
  return fetchOwnerProfile();
}
