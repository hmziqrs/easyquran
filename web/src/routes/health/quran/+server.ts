import { json } from "@sveltejs/kit";

import type { RequestHandler } from "./$types";

export const GET: RequestHandler = async () =>
  json({ ready: true }, { headers: { "cache-control": "no-store" } });
