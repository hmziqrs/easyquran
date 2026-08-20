import { QURAN } from "$lib/config/site";
import type { ArtifactSpec } from "$lib/data/quran-types";

export interface ResolvedManifest {
  scripts: readonly ArtifactSpec[];
}

export function bakedManifest(): ResolvedManifest {
  return { scripts: QURAN.scripts };
}
