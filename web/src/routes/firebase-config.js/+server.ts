import { firebaseConfig } from "$lib/firebase";
import type { RequestHandler } from "@sveltejs/kit";

export const prerender = true;

export const GET: RequestHandler = () => {
  const body = `self.__easyquranFirebase=${JSON.stringify(firebaseConfig)};`;
  return new Response(body, {
    headers: {
      "content-type": "text/javascript;charset=utf-8",
      "cache-control": "public, max-age=0, must-revalidate",
    },
  });
};
