/* ════════════════════════════════════════════════════════════════════════
   /firebase-config.js — prerendered config for the FCM service worker.

   A classic service worker can't import SvelteKit's `$env/*` (and the static
   build must still succeed when no Firebase config is present), so the worker
   reads its config from THIS file via importScripts. We prerender it at build
   time, writing `self.__easyquranFirebase = {…}` from the PUBLIC_FIREBASE_*
   env vars — empty object when unconfigured, which makes the worker no-op.

   The web config is PUBLIC by Firebase design (it ships in the client bundle
   regardless); exposing it here adds nothing an attacker couldn't already read.
   ════════════════════════════════════════════════════════════════════════ */

import type { RequestHandler } from "@sveltejs/kit";
import { env } from "$env/dynamic/public";

export const prerender = true;

export const GET: RequestHandler = () => {
  const config = {
    apiKey: env.PUBLIC_FIREBASE_API_KEY,
    authDomain: env.PUBLIC_FIREBASE_AUTH_DOMAIN,
    projectId: env.PUBLIC_FIREBASE_PROJECT_ID,
    storageBucket: env.PUBLIC_FIREBASE_STORAGE_BUCKET,
    messagingSenderId: env.PUBLIC_FIREBASE_MESSAGING_SENDER_ID,
    appId: env.PUBLIC_FIREBASE_APP_ID,
    measurementId: env.PUBLIC_FIREBASE_MEASUREMENT_ID,
  };
  const body = `self.__easyquranFirebase=${JSON.stringify(config)};`;
  return new Response(body, {
    headers: {
      "content-type": "text/javascript;charset=utf-8",
      // Always revalidate so a config change is picked up on the next SW update.
      "cache-control": "public, max-age=0, must-revalidate",
    },
  });
};
