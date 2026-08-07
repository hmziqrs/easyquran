import { QURAN } from "$lib/config/site";
import type { ArtifactSpec } from "$lib/data/quran-types";
import { FETCH_TIMEOUT_MS, fetchWithTimeout } from "./fetch";
import { DEFAULT_QURAN_SOURCE_PLAN, plannedSourceIds } from "./source-plan";
import { resolveSourceProfile } from "./view/source-profiles";
import { decodeScriptsPayload } from "./wire";

export const ManifestSource = {
  Api: "api",
  Baked: "baked",
} as const;
export type ManifestSource = (typeof ManifestSource)[keyof typeof ManifestSource];

export interface ResolvedManifest {
  scripts: readonly ArtifactSpec[];
  source: ManifestSource;
}

const baked: ResolvedManifest = {
  scripts: QURAN.scripts,
  source: ManifestSource.Baked,
};

function hasRegisteredPlan(scripts: readonly ArtifactSpec[]): boolean {
  try {
    return plannedSourceIds(DEFAULT_QURAN_SOURCE_PLAN).every((sourceId) => {
      const artifact = scripts.find((candidate) => candidate.id === sourceId);
      if (!artifact) return false;
      resolveSourceProfile(artifact.id);
      return true;
    });
  } catch {
    return false;
  }
}

function localizeDeliveryUrls(scripts: readonly ArtifactSpec[]): ArtifactSpec[] {
  return scripts.map((script) => {
    const local = QURAN.scripts.find((candidate) => candidate.id === script.id);
    return local ? { ...script, downloadUrl: local.downloadUrl } : script;
  });
}

export async function resolveManifest(signal?: AbortSignal): Promise<ResolvedManifest> {
  if (!QURAN.apiBase) return baked;
  try {
    const res = await fetchWithTimeout(`${QURAN.apiBase}/scripts`, {
      signal,
      timeout: FETCH_TIMEOUT_MS,
      headers: { accept: "application/json" },
    });
    if (!res.ok) return baked;
    const scripts = decodeScriptsPayload(await res.json());
    if (!scripts || !hasRegisteredPlan(scripts)) return baked;
    return { scripts: localizeDeliveryUrls(scripts), source: ManifestSource.Api };
  } catch {
    return baked;
  }
}
