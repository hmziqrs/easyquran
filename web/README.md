# EasyQuran — web

SvelteKit 2 + Svelte 5 (runes), TypeScript strict, Tailwind v4, shadcn-svelte
primitives, Firebase (analytics/performance/messaging), built with Vite+ (`vp`)
and `@sveltejs/adapter-static` — fully prerendered.

## Commands

From the repo root, `just` picks the Quran database source:

```bash
just web-dev local      # dev server, tracked SQLite files at /_quran
just web-dev prod       # dev server, production R2 files
just web-build prod     # static build (default)
just docker-up local    # Caddy/Docker build at localhost:8080 (always a prod build)
```

`PUBLIC_ENV` is the only selector — `local` or `prod`. It changes the database
download origin, nothing else. `pnpm dev` defaults to local, `pnpm build` to
prod, and Docker is always prod.

From `web/`:

```bash
pnpm dev / build / preview
pnpm check    # svelte-check + tsconfig.worker.json (service worker)
vp lint       # Oxlint, type-aware
vp fmt
vp check      # fmt + lint + type-check
```

Dependencies install from the repo root (pnpm workspace); `vite` / `vite-plus`
are pinned there via `catalog:`.

## Layout

Two route groups; names in parentheses don't appear in URLs.

```
src/
├─ app.html                  # <html data-theme/data-accent> + no-FOUC inline script
├─ service-worker.ts         # THE root worker: caching + FCM push (see below)
├─ routes/
│  ├─ layout.css             # design system: Tailwind v4 @theme + token values
│  ├─ +layout.svelte         # global only: css, JSON-LD, prefs, analytics boot
│  ├─ (marketing)/           # public, indexable, prerendered
│  ├─ (application)/app/     # product UI, noindex, prerendered
│  ├─ sitemap.xml/ llms.txt/ llms-full.txt/   # derived from site.ts
│  └─ [slug].md/ [slug].txt/ # text variants, generated from prerendered HTML
└─ lib/
   ├─ config/site.ts         # single source of truth: nav, page metadata, QURAN config
   ├─ quran/                 # canonical view, offline engine, worker client, search
   ├─ firebase/ boot/ stores/
   └─ components/            # ui/ (shadcn) + layout, text, nav, footer, seo, status
```

## Conventions

- **Single source of truth** — `lib/config/site.ts` drives nav, metadata,
  sitemap, llms.txt and `<Seo>`. Adding a page to `MARKETING_PAGES` + `PAGE_META`
  extends all of them at once; `/app` routes stay `noindex` by construction.
- **Design tokens → Tailwind** — `layout.css` declares token values, then maps
  them via `@theme inline` so utilities are theme-aware at runtime.
- **shadcn semantic names must be declared twice** — as `:root` custom
  properties _and_ inside `@theme inline`. With only the `:root` half, registry
  components compile but render **unstyled**.
- **No FOUC** — the inline script in `app.html` applies theme/accent before
  first paint; `prefs.apply()` re-syncs on mount.
- **Text variants** are built by fetching each page's own prerendered HTML and
  converting `<main>` with turndown, so they can't drift from what visitors see.
- **`og.png` has the domain baked in as pixels** — re-run `./design/generate.sh`
  whenever `SITE.domain` changes. Nothing else will catch it.

## Firebase

Config is **hardcoded** in `src/lib/firebase/index.ts` (public by design, gated
by Security Rules / App Check). All three services are browser-only and lazily
imported so they never enter the critical path or run during prerender. Consent
lives in `stores/consent.svelte.ts` and gates analytics/performance.

Push is handled natively by `src/service-worker.ts` — **no `importScripts` of
the gstatic SDK**: a failing top-level cross-origin import aborts the whole
worker and would take offline reading down with it. The payload is
self-contained, so the worker just calls `showNotification`. The client
subscribes with `getToken`, passing the registration from
`lib/boot/service-worker.ts`. Backend token registration only happens when
`PUBLIC_API_BASE_URL` is set.

## Standing decisions

- **No CI.** The quality floor is the local `package.json` scripts run before
  commit. Guidance elsewhere that assumes CI does not apply here.
- **No list virtualization.** Measured 2026-08-01 against a production build:
  the 604-page list mounts in ~80 ms with zero long tasks, and the verse body is
  already covered by `content-visibility`. No bottleneck to solve.

Open follow-ups:

- The `createPreferences` / `createConsent` / `createNotifications` factories
  exist but nothing consumes them; `reader` still resolves via its singleton
  fallback.
- `prefs.svelte.ts` is not yet migrated onto the `$lib/storage` foundation that
  `consent`, `notifications`, and `reader` use.
