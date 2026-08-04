# EasyQuran — web

SvelteKit 2 + Svelte 5 (runes). TS strict. Tailwind v4. shadcn-svelte. Firebase. Vite+ (`vp`). `adapter-static` now (prerendered); -> `adapter-node` SSR + disk-TTL for translated pages (see `docs/quran.md`).

## Run

Repo root (`just` picks DB source):

    just web-dev local     # dev, tracked SQLite at /_quran
    just web-dev prod      # dev, R2 files
    just web-build prod    # static build (default)
    just docker-up local   # Caddy build, localhost:8080, always prod

`PUBLIC_ENV` = `local|prod`. Selects DB origin only. `pnpm dev`->local, `pnpm build`->prod, Docker always prod.

In `web/`:

    pnpm dev | build | preview
    pnpm check      # svelte-check + worker tsconfig
    vp lint | fmt | check

Deps from repo root (pnpm workspace). `vite` / `vite-plus` pinned via `catalog:`.

## Layout

    src/
    ├─ app.html              # data-theme/accent + no-FOUC inline script
    ├─ service-worker.ts     # root SW: cache + FCM push
    ├─ routes/
    │  ├─ layout.css         # design system: @theme + token values
    │  ├─ +layout.svelte     # global: css, JSON-LD, prefs, analytics boot
    │  ├─ (marketing)/       # public, indexable, prerendered
    │  ├─ (application)/app/ # product UI, noindex, prerendered
    │  ├─ sitemap.xml/ llms.txt/ llms-full.txt/
    │  └─ [slug].md/ [slug].txt/   # text variants from prerendered HTML
    └─ lib/
       ├─ config/site.ts     # source of truth: nav, meta, QURAN config
       ├─ quran/             # canonical view, offline engine, worker client, search
       ├─ firebase/ boot/ stores/
       └─ components/        # ui/ (shadcn) + layout/text/nav/footer/seo/status

## Rules

- `site.ts` drives nav/meta/sitemap/llms/Seo. Add page -> `MARKETING_PAGES` + `PAGE_META`.
- tokens in `layout.css` -> mapped via `@theme inline` -> theme-aware utils.
- shadcn names declared twice (`:root` + `@theme inline`); one half -> renders unstyled.
- no FOUC: `app.html` inline script applies theme pre-paint.
- Firebase config hardcoded `lib/firebase/index.ts` (public by design). Push native in SW, no gstatic `importScripts`.
- no CI. sidebar lists virtualized (`@tanstack/svelte-virtual`); only visible rows render.
