# EasyQuran — web

The EasyQuran web app: a **SvelteKit** app with a typed design system, shadcn-svelte
primitives, and Firebase Analytics. Same stack and conventions as `oxlabs.dev`.

## Stack

| Concern       | Choice                                                                       |
| ------------- | ---------------------------------------------------------------------------- |
| Framework     | **SvelteKit 2** + **Svelte 5** (runes, forced on outside `node_modules`)     |
| Bundler / dev | **Vite+** (`vp`) — Vite + **Rolldown**, with **Oxlint** + **Oxfmt** unified  |
| Language      | **TypeScript** (strict)                                                      |
| Styling       | **Tailwind CSS v4** (`@tailwindcss/vite`), tokens mapped via `@theme`        |
| UI primitives | **shadcn-svelte** pattern (`tailwind-variants` + `cn`), themed to our tokens |
| Analytics     | **Firebase Analytics** — browser-only, lazily imported, env-configured       |
| Output        | **`@sveltejs/adapter-static`** — fully prerendered, deployable anywhere      |

## Commands

Run from this directory (`web/`):

```bash
pnpm dev      # dev server (HMR)          — alias for `vp dev`
pnpm build    # production build → build/  (prerendered static site)
pnpm preview  # preview the production build
pnpm check    # svelte-check type-check
vp lint       # Oxlint (type-aware)
vp fmt        # Oxfmt
vp check      # format + lint + type-check in one pass
```

> `vp` is the Vite+ CLI (install once: `curl -fsSL https://vite.plus | bash`).
> Plain `pnpm` / `vite` commands work too — `vp` is a drop-in.

Dependencies are installed from the repo root (pnpm workspace, `packages: [web]`),
which is also where the pnpm `catalog:` pins for `vite` / `vite-plus` live.

## Firebase

Copy `.env.example` → `.env` and fill in the web config from the Firebase console
(Project settings → General → Your apps → SDK setup and configuration):

```
PUBLIC_FIREBASE_API_KEY=…
PUBLIC_FIREBASE_APP_ID=…
…
```

These values are public by design — they ship in the client bundle; access is gated
by Security Rules / App Check, not by hiding them. They're read through
`$env/dynamic/public`, so **until they're set, `src/lib/firebase.ts` no-ops
entirely** and analytics simply never starts — nothing breaks.

Analytics starts in `+layout.svelte` via `initAnalytics()` inside `onMount` (never
during SSR, and only when `isSupported()`). Fire custom events from anywhere with
`track("event_name", { … })`; it silently drops until analytics is ready.

## shadcn-svelte

`components.json` is wired to the shadcn-svelte registry with `src/lib` as the
frontend dir, so components land in `$lib/components/ui`:

```bash
pnpm dlx shadcn-svelte@latest add dialog
```

Registry components inherit our look automatically: `routes/layout.css` maps the
shadcn semantic variables (`--background`, `--primary`, `--border`, `--ring` …)
onto our own tokens, and dark mode is driven by `[data-theme="dark"]` via a
`@custom-variant` rather than a `.dark` class.

`ui/button` is included as the reference primitive (tailwind-variants + `cn`).

## Architecture

```
src/
├─ app.html                    # <html data-theme/data-accent> + no-FOUC inline script + fonts
├─ routes/
│  ├─ layout.css               # THE design system: Tailwind v4 @theme + token values + base
│  ├─ +layout.svelte           # <main>, prefs applied on mount, analytics start, JSON-LD
│  ├─ +layout.ts               # prerender = true (static)
│  ├─ +page.svelte, about/     # pages
│  ├─ sitemap.xml/+server.ts   # derived from NAV_PAGES
│  └─ llms.txt/+server.ts      # derived from PAGE_META
└─ lib/
   ├─ config/site.ts           # single source of truth: nav, accents, page metadata
   ├─ firebase.ts              # Firebase app + Analytics (SSR-safe, lazy, env-driven)
   ├─ stores/prefs.svelte.ts   # theme/accent prefs (runes), persisted, SSR-safe
   ├─ utils.ts                 # cn(), externalLinkAttrs(), shadcn prop-type helpers
   └─ components/
      ├─ ui/button/            # shadcn-style Button (tailwind-variants)
      ├─ layout/               # Container, Section
      └─ seo/                  # <Seo path="…" /> — title/description/OG/canonical
```

### Conventions

- **Single source of truth** — `lib/config/site.ts` drives nav, page metadata,
  sitemap, llms.txt and `<Seo>`. Nothing about site structure is duplicated.
- **Design tokens → Tailwind** — `routes/layout.css` declares dark/light/accent token
  values, then maps them into Tailwind v4 with `@theme inline` so utilities
  (`bg-bg-1`, `text-fg-3`, `text-accent`, `font-mono` …) are theme-aware at runtime.
- **No FOUC** — the inline script in `app.html` applies the saved theme/accent to
  `<html>` before first paint; `prefs.apply()` re-syncs on mount.
- **Arabic type** — Amiri is loaded in `app.html` and exposed as the `--font-arabic`
  token plus an `arabic` utility (RTL, generous leading) for mushaf text.

## Placeholders to replace

- `SITE.domain` / `SITE.url` in `lib/config/site.ts` (currently `easyquran.app`) —
  also update the `Sitemap:` line in `static/robots.txt`.
- `.env` Firebase credentials (see above).
- `lib/assets/favicon.svg` and the `about/` page copy.
