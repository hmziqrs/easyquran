export const prerender = true;

import { renderLlmsIndex } from "$lib/seo/render";
import type { RequestHandler } from "./$types";

export const GET: RequestHandler = () =>
  new Response(renderLlmsIndex(), {
    headers: { "content-type": "text/plain; charset=utf-8" },
  });
