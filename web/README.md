# EasyQuran — web

SvelteKit 2 + Svelte 5 (runes). TS strict. Tailwind v4. shadcn-svelte. Firebase. Vite+ (`vp`). `adapter-node`: Arabic reader routes stay prerendered; translation routes render on demand and use bounded 7-day disk-TTL HTML caching (see `docs/quran-system.md`).

## Run

Fresh clone: provision the Quran DBs first — `just quran-fetch` (public R2 base; `just quran-fetch all`
also pulls every translation sqlite); `db/` is gitignored and starts empty.

Repo root (`just` picks DB source):

    just web-dev local     # dev, SQLite served at /_quran from the gitignored db/quran/
    just web-dev prod      # dev, same-origin gateway to R2
    just web-build prod    # adapter-node build + Arabic prerender output
    just docker-up local   # Bun web + Axum API, localhost:8080 / :8888

`PUBLIC_ENV` = `local|prod`. Selects DB origin only. `pnpm dev`->local, `pnpm build`->prod, Docker always prod.
Local development serves SQLite from the gitignored `db/quran/` at `/_quran`; production uses the same URL
space as an allowlisted streaming gateway to R2, so OPFS downloads never depend on cross-origin bucket CORS.

In `web/`:

    pnpm dev | build | preview
    pnpm start      # run the production build + static/dynamic header policy
    pnpm check      # svelte-check + worker tsconfig
    vp lint | fmt | check

Deps from repo root (pnpm workspace). `vite` / `vite-plus` pinned via `catalog:`.

Development-only IndoPak font specimen: `/design/indopak`, using provisioned IndoPak/Uthmani
DBs and packaged OFL compatibility font. Version 4 (SIL Lateef base, Quran.com-style word
boxes) serves both IndoPak reader modes.
`?audit=all` covers every private occurrence; `?audit=flow` checks optional-ayah marks with
original neighboring verses. All specimen variants return 404 in production.
Build/audit/browser commands and evidence: [font tooling](../scripts/fonts/indopak/README.md),
[compatibility report](../docs/indopak-font-compatibility.md),
[deep audit and unresolved source differences](../docs/indopak-deep-audit.md).

Development-only homepage comparisons: `/design/home/cobalt`, `/design/home/mono`,
`/design/home/slate`, and `/design/home/berry`. Each uses baked surah metadata, with
local controls for Onest/Nunito/JetBrains Mono, accent, neutral background, and light/dark
mode. Selecting another variant resets the controls; reader appearance preferences are
unaffected. These previews return 404 in production and are linked from `/design` in dev.

## Layout

    src/
    ├─ app.html              # data-theme/accent + no-FOUC inline script
    ├─ service-worker.ts     # root SW: cache + FCM push
    ├─ routes/
    │  ├─ layout.css         # design system: @theme + token values
    │  ├─ +layout.svelte     # global: css, JSON-LD, prefs, analytics boot
    │  ├─ (marketing)/       # public, indexable, prerendered
    │  ├─ (application)/     # product UI, noindex; Arabic prerendered (surah, page, juz, hizb, rub — scheme A: unprefixed en, /ar ar), translated routes SSR + disk-TTL (see Part 3, divergence #1)
    │  ├─ sitemap.xml/ llms.txt/ llms-full.txt/
    │  ├─ [slug=marketingText].md/ [slug].txt/  # marketing text variants (matcher-split from the reader twins)
    │  └─ (application)/[surah=surahSlug].md/   # reader text variants (114 baked slugs)
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
- Image CI only (`.github/workflows/images.yml`): builds+pushes both images on arm64 runners on master
  push/tag; no deploy step (a Dokploy Schedule Job pulls them — deploy/README.md) and no check/lint/test
  job — those stay local gates before push. sidebar lists
  virtualized (`@tanstack/svelte-virtual`); only visible rows render.
