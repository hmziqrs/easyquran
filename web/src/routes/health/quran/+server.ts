import { version as appBuildId } from "$app/environment";
import { getQuranDiskCacheStats } from "$lib/server/quran-disk-cache";
import { json } from "@sveltejs/kit";

import type { RequestHandler } from "./$types";

export const GET: RequestHandler = async () =>
  json(
    {
      ready: true,
      appBuildId,
      translatedPageCache: await getQuranDiskCacheStats(),
    },
    { headers: { "cache-control": "no-store" } },
  );
