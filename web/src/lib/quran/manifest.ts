import { QURAN } from "$lib/config/site";
import type { ArtifactSpec } from "$lib/data/quran-types";
import { DEFAULT_QURAN_SOURCE_PLAN, plannedSourceIds } from "./source-plan";
import { resolveSourceProfile } from "./view/source-profiles";
import { decodeScriptsPayload, decodeVersionPayload } from "./wire";

export const ManifestSource = {
  Api: "api",
  Baked: "baked",
} as const;
export type ManifestSource = (typeof ManifestSource)[keyof typeof ManifestSource];

export interface ResolvedManifest {
  contentVersion: string;
  searchVersion: string;
  scripts: readonly ArtifactSpec[];
  source: ManifestSource;
}

const baked: ResolvedManifest = {
  contentVersion: QURAN.contentVersion,
  searchVersion: QURAN.searchVersion,
  scripts: QURAN.scripts,
  source: ManifestSource.Baked,
};

function hasRegisteredPlan(scripts: readonly ArtifactSpec[]): boolean {
  try {
    return plannedSourceIds(DEFAULT_QURAN_SOURCE_PLAN).every((sourceId) => {
      const artifact = scripts.find((candidate) => candidate.id === sourceId);
      if (!artifact) return false;
      resolveSourceProfile(artifact.id, artifact.sha256);
      return true;
    });
  } catch {
    return false;
  }
}

export async function resolveManifest(signal?: AbortSignal): Promise<ResolvedManifest> {
  if (!QURAN.apiBase) return baked;
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), 3000);
  const onAbort = () => ctrl.abort();
  signal?.addEventListener("abort", onAbort);
  try {
    const [vRes, sRes] = await Promise.all([
      fetch(`${QURAN.apiBase}/version`, {
        signal: ctrl.signal,
        headers: { accept: "application/json" },
      }),
      fetch(`${QURAN.apiBase}/scripts`, {
        signal: ctrl.signal,
        headers: { accept: "application/json" },
      }),
    ]);
    if (!vRes.ok || !sRes.ok) return baked;

    const sBody = await sRes.json();
    const scripts = decodeScriptsPayload(sBody);
    if (!scripts || !hasRegisteredPlan(scripts)) return baked;

    const vBody = await vRes.json();
    const version = decodeVersionPayload(vBody) ?? {
      contentVersion: null,
      searchVersion: null,
    };
    return {
      contentVersion: version.contentVersion ?? QURAN.contentVersion,
      searchVersion: version.searchVersion ?? QURAN.searchVersion,
      scripts,
      source: ManifestSource.Api,
    };
  } catch {
    return baked;
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener("abort", onAbort);
  }
}
