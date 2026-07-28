// The app shell is prerendered too, so the first paint is a static file and
// the UI hydrates on top of it. Reading state lives client-side.
//
// If /app ever needs per-request data or dynamic (non-enumerable) routes such
// as /app/read/[surah], switch this to `prerender = false` + `ssr = false` and
// point the adapter's `fallback` at a SPA shell (200.html) in vite.config.ts —
// the current fallback (404.html) assumes a fully prerendered site.
export const prerender = true;
