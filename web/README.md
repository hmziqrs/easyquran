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
| Type          | **Geist / Geist Mono / Amiri**, self-hosted via `@fontsource` (no CDN)       |
| SEO           | canonical + OG/Twitter + JSON-LD, sitemap, `llms.txt`, `.md`/`.txt` variants |
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

> **Note:** those semantic names are declared **twice** on purpose — as `:root`
> custom properties (section 5, the values) _and_ inside `@theme inline`
> (section 3, the Tailwind mapping). Tailwind v4 only generates the
> `bg-background` / `border-border` / `ring-ring` utilities for names present in
> `@theme`; with the `:root` half alone, registry components compile but render
> **unstyled**. Add both halves when introducing a new semantic name.

`ui/button` is the reference primitive (tailwind-variants + `cn`); `ui/separator`
was installed straight from the registry to verify the wiring end to end.

## Architecture

The route tree is split into two **route groups**. Group names in parentheses do
not appear in URLs — `(marketing)/about` serves `/about`, and
`(application)/app/read` serves `/app/read`.

```
src/
├─ app.html                    # <html data-theme/data-accent> + no-FOUC inline script
├─ routes/
│  ├─ layout.css               # THE design system: Tailwind v4 @theme + token values + base
│  ├─ +layout.svelte           # GLOBAL only: css, icons/manifest, site JSON-LD, prefs, analytics
│  ├─ +layout.ts               # trailingSlash (prerender is declared per group)
│  ├─ +error.svelte            # 404 / error page (noindex)
│  │
│  ├─ (marketing)/             # ── public, indexable ──────────────────────────
│  │  ├─ +layout.svelte        #    Nav + <main id="main"> + Footer + Tweaks
│  │  ├─ +layout.ts            #    prerender = true
│  │  ├─ +page.svelte          #    /
│  │  ├─ about/ download/ privacy/
│  │
│  ├─ (application)/app/       # ── product UI, noindex ────────────────────────
│  │  ├─ +layout.svelte        #    app shell: tabs + <main id="main"> + mobile tab bar
│  │  ├─ +layout.ts            #    prerender = true
│  │  ├─ +page.svelte          #    /app
│  │  └─ read/ bookmarks/ settings/
│  │
│  ├─ sitemap.xml/+server.ts   # derived from MARKETING_PAGES
│  ├─ llms.txt/+server.ts      # LLM index, derived from MARKETING_PAGES + PAGE_META
│  ├─ llms-full.txt/+server.ts # every marketing page's markdown, concatenated
│  ├─ [slug].md/+server.ts     # markdown variant  (/index.md, /about.md, …)
│  └─ [slug].txt/+server.ts    # plain-text variant (/index.txt, /about.txt, …)
└─ lib/
   ├─ config/site.ts           # single source of truth: nav, accents, page metadata
   ├─ firebase.ts              # Firebase Analytics (SSR-safe, fully lazy, env-driven)
   ├─ seo/render.ts            # HTML → markdown → plain text (turndown) + llms index
   ├─ stores/prefs.svelte.ts   # theme/accent prefs (runes), persisted, SSR-safe
   ├─ utils.ts                 # cn(), externalLinkAttrs(), shadcn prop-type helpers
   └─ components/
      ├─ ui/button/ ui/separator/   # shadcn-style + registry-installed primitives
      ├─ layout/ text/ icon/ brand/ # Container, Section, Eyebrow, Icon, Brand
      ├─ card/ chip/ panel/ status/ # Card, Chip, Panel, StatusDot, Pulse
      ├─ nav/ footer/ tweaks/       # site chrome + appearance panel
      └─ seo/                       # <Seo path="…" /> — meta, OG, JSON-LD, alternates
```

### Marketing vs. application

`lib/config/site.ts` keeps the two halves apart, and everything else derives from
that split:

|                         | `MARKETING_PAGES`                      | `APP_PAGES`                               |
| ----------------------- | -------------------------------------- | ----------------------------------------- |
| URLs                    | `/`, `/about`, `/download`, `/privacy` | `/app`, `/app/read`, …                    |
| Top nav                 | `NAV_PAGES` (the `nav: true` subset)   | app shell tabs                            |
| Sitemap / llms.txt      | yes                                    | **no**                                    |
| `.md` / `.txt` variants | yes                                    | **no**                                    |
| Robots                  | `index, follow`                        | `noindex, follow` via `<Seo … noindex />` |

So adding a public page to `MARKETING_PAGES` + `PAGE_META` extends the nav,
sitemap, llms.txt and both text variants at once, while `/app` routes stay out of
search results by construction. App pages still get the site-level
WebSite/Organization JSON-LD (that's site identity, not page content), but no
per-page `WebPage`/`BreadcrumbList` node, canonical, or social card.

### Text variants (for LLMs and crawlers)

Every page is also served as markdown and plain text: `/about` → `/about.md` and
`/about.txt` (home uses `/index.md` / `/index.txt`), plus `/llms.txt` (an index)
and `/llms-full.txt` (every page concatenated). These are generated at build time
by **fetching each page's own prerendered HTML** and converting its `<main>` with
turndown — the rendered page is the single source of truth, so the text variants
can never drift from what visitors see. `static/_headers` marks them
`noindex, follow` so the canonical HTML is what ranks.

Adding a page to `NAV_PAGES` + `PAGE_META` automatically extends the sitemap,
llms.txt, and both text variants — no endpoint edits needed.

### Conventions

- **Single source of truth** — `lib/config/site.ts` drives nav, page metadata,
  sitemap, llms.txt and `<Seo>`. Nothing about site structure is duplicated.
- **Design tokens → Tailwind** — `routes/layout.css` declares dark/light/accent token
  values, then maps them into Tailwind v4 with `@theme inline` so utilities
  (`bg-bg-1`, `text-fg-3`, `text-accent`, `font-mono` …) are theme-aware at runtime.
- **No FOUC** — the inline script in `app.html` applies the saved theme/accent to
  `<html>` before first paint; `prefs.apply()` re-syncs on mount.
- **Arabic type** — Amiri is self-hosted (imported in `layout.css`) and exposed as
  the `--font-arabic` token plus an `arabic` utility (RTL, generous leading) for
  mushaf text.

## Static assets

`static/` carries the full icon/PWA set — `favicon.ico`, `apple-touch-icon.png`,
`icons/icon-{16,32,192,512}.png`, `logo.png` (JSON-LD), `og.png` (1200×630 social
card), `manifest.webmanifest`, plus `_headers` and `_redirects` (Cloudflare Pages /
Netlify; mirror them in `vercel.json` on Vercel). All were generated from the
placeholder mark in `lib/assets/favicon.svg` — regenerate them if the mark changes.

## Not ported from oxlabs.dev

Deliberately left out as site-specific rather than infrastructure: the lazy
**three.js scene system**, the **contact form**, `lib/data/*` page copy, the
marquee **Band** / **Tech** chips / **CodeBlock** components, and the build-time
**owner-profile fetch** (`stores/owner.ts` + `+layout.server.ts`, which pulls
`hmziq.rs/me.json` to populate the footer and Person JSON-LD). Say the word if you
want any of these.

## Placeholders to replace

- `SITE.domain` / `SITE.url` in `lib/config/site.ts` (currently `easyquran.app`) —
  also update the `Sitemap:` line in `static/robots.txt` and `static/_redirects`.
- `.env` Firebase credentials (see above).
- `lib/assets/favicon.svg` + the generated `static/` images, and the page copy.
