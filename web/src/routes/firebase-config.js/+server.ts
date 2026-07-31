/* ════════════════════════════════════════════════════════════════════════
   /firebase-config.js — prerendered config for the FCM service worker.

   Emits `self.__easyquranFirebase = {…}` from the SINGLE source of truth — the
   hardcoded config in $lib/firebase/index.ts — so the worker and the client can
   never drift. The web config is PUBLIC by Firebase design (it ships in the
   client bundle regardless); exposing it here adds nothing an attacker couldn't
   already read.
   ════════════════════════════════════════════════════════════════════════ */

import type { RequestHandler } from "@sveltejs/kit";
import { firebaseConfig } from "$lib/firebase";

export const prerender = true;

export const GET: RequestHandler = () => {
  const body = `self.__easyquranFirebase=${JSON.stringify(firebaseConfig)};`;
  return new Response(body, {
    headers: {
      "content-type": "text/javascript;charset=utf-8",
      // Always revalidate so a config change is picked up on the next SW update.
      "cache-control": "public, max-age=0, must-revalidate",
    },
  });
};
