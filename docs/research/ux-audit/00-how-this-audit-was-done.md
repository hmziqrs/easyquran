# 00 · How this audit was done

[← Back to the index](README.md)

## Scope

Every screen a reader can reach, in both languages the app ships (English, Arabic), both themes (light, dark), at phone, tablet and desktop widths:

| Area | Routes checked |
| --- | --- |
| Marketing | `/`, `/ar/`, `/about`, `/faq`, `/contact`, `/nope` (error page) |
| App home | `/en/app`, `/ar/app` (first visit and returning reader) |
| Browse lists | `/en/app/surah`, `/en/app/juz`, `/en/app/pages` (+ `/ar/…`) |
| Reader | `/en/app/al-fatihah`, `/en/app/al-baqarah`, `/en/app/an-nas`, `/en/app/page/1`, `/en/app/juz/1` (+ `/ar/…`) |
| Translated reader | `/en/app/al-baqarah/t/en/sahih`, `/en/app/al-fatihah/t/en/sahih?more=ur.jalandhry`, `/en/app/t/en/sahih/page/1` |
| Overlays | header menu panel, floating "Customize appearance" panel, reader sidebar, translations modal, ⌘K palette, verse note/tafsir panel |
| Your stuff | `/app/yours`, `/app/bookmarks` (empty and with data) |
| Search | `/app/search` (empty, English word, Arabic word) |
| Settings | `/app/settings` — Storage, Appearance, Reading, Privacy, Account |
| Account | `/login`, `/register`, `/account` |
| Error / edge | unknown routes, locale-prefixed non-reader routes, offline, API unreachable, 320 px width |
| **Second pass (17–26)** | 4 palettes × light/dark on 10 routes; custom colours; 4 Arabic scripts × 6 fonts; browser text size, forced colours, increase contrast, reduced motion, 280–2560 px + landscape; legal pages, full landing, juz/page edges, reading-mode dialog, offline pack, clear data, search translation picker, deep links; register/sign-in/verify/forgot/account/sessions/folders/sync/sign-out/OAuth failure; tab order and every overlay's focus handling, accessibility trees, hotkeys; RTL and stacked translations, tafsir as main text, transliteration, empty verses; hover/press states, scroll restoration; throttled loading on a production build; manifest, icons, titles, OG images; WebKit (iPhone profile) vs Chromium |

## Environment

- Dates: 2026-09-29 (first pass) and 2026-09-29 → 30 (second pass), branch `master`; no app code changed during the audit.
- Web: `PUBLIC_ENV=local pnpm dev` (Vite dev server, SQLite served from the local `db/quran/`).
- API: the first half of the session ran **without** the Rust API; the second half ran it with `cargo run -p ruxlog` on `:8888`. Findings that only appear when the API is unreachable are labelled **"API unreachable"** — they are real for an offline-first app (a phone on bad signal is the same situation) but are not what a user sees when everything is healthy.
- Browser: headless Chrome (Puppeteer from `web/node_modules`), fresh profile per screenshot, `prefers-color-scheme` emulated, device scale 1× desktop / 2× phone. Second pass also used Playwright **WebKit 26.6** (iPhone 15 profile + desktop).
- Production build: `PUBLIC_ENV=local pnpm build` served with `bun server.ts` on :5392 for loading, offline-pack, analytics and PWA checks (no tracked files changed by the build).
- Signed-in checks used a local test account `ux-audit.test+1@example.com` in the local dev DB (`rust/data/easyquran.db`); only that row was marked verified because no email can arrive locally. Registration from :5391 needed a temporary proxy on :5173 because the API's CORS allow-list only contains :5173. **Delete the test account when convenient.**

## Methods

1. **Walkthrough as a first-time, non-technical reader.** Each screen was judged on: *Can I tell where I am? Can I tell what this does before I tap it? Is the Qur'an text the clearest thing on the screen? Would my grandparent understand this word?*
2. **Parity sweep.** The same thing (a surah name, a search box, a button, a "selected" state, a theme switch) was compared everywhere it appears. Every mismatch is logged.
3. **Automated accessibility scan.** [axe-core 4.10](https://github.com/dequelabs/axe-core) with WCAG 2.0/2.1/2.2 A+AA + best-practice rules, on 18 routes × 2 themes (36 scans). Raw results are summarised in [09 · Accessibility](09-accessibility.md).
4. **Measured probes.** A script measured every interactive element's size (tap targets), every text node under 14 px, and the contrast of specific elements (verse numbers, tool icons, wordmark, chips) against the colour actually behind them.
5. **Code reading** to confirm the cause of each finding and point at the file that needs to change.
6. **Second pass (release readiness)** — four parallel audits, each with its own docs and screenshots: themes/scripts/display (17–18), remaining screens/flows/signed-in (19–20), keyboard/screen-reader/translations/interactions (21–23), loading/PWA/cross-browser (24–26). Tools added: CDP `Page.setFontSizes`, `Emulation.setEmulatedMedia` (forced-colors, prefers-contrast), `Accessibility.getFullAXTree`, network/CPU throttling with PerformanceObserver (CLS/LCP), filmstrips. Their notes that extend first-pass findings were folded into those findings as **"Second pass adds."** blocks; raw notes live in [`_merge/`](_merge/).

## Severity scale

| Level | Meaning | Rule of thumb |
| --- | --- | --- |
| **P0** | Breaks trust or blocks a core task (read, find a verse, come back to where you were) | Fix before the next release |
| **P1** | Real friction for many readers, or a clear WCAG 2.2 AA failure | Fix this cycle |
| **P2** | Inconsistency or polish that makes the app feel unfinished | Schedule |
| **P3** | Nice to have | Backlog |

## How each finding is written

```
### AREA-NN · Short title
Severity · Who it hurts · Where (route + file)
Screenshot (red boxes mark the problem)
What's wrong — plain description
Why it matters — for a non-technical reader
Fix — concrete change, with the file to edit
Done when — a check anyone can run
```

## Re-running the captures

The capture harness used for every screenshot is saved in [`tools/`](tools/README.md). It needs the dev server running and the Quran DBs provisioned (`just quran-fetch`).

## Coverage and gaps

**Covered (both passes):** every route in `web/src/routes/**` that a reader can reach (marketing, app, auth, account, error), both locales, all four palettes in both modes, all Arabic scripts and fonts (sampled surahs), signed-out and signed-in, online / offline / API-unreachable, dev server and production build, widths 280–2560 px plus landscape, Chromium and WebKit.

**Still not covered — needs people, devices or services this environment doesn't have:**

| Gap | Why | What to do before release |
| --- | --- | --- |
| Real iPhone Safari and Android Chrome | Only emulation (WebKit engine, Chrome device mode) | 30-minute manual pass on each ([27](27-release-polish-checklist.md#before-you-tag-the-release)) |
| Firefox | Playwright Firefox would not launch on this macOS; [26](26-cross-browser.md) has a code-level risk list | Manual Firefox pass |
| Real screen readers (VoiceOver, TalkBack, NVDA) | Findings come from the accessibility tree and live-region observation | 20-minute VoiceOver + TalkBack pass |
| Real users | Expert judgement only | 5-person task test ([16](16-fix-roadmap.md#validate-with-real-people)) |
| Real email (verify, reset) and real Google/Apple sign-in | No SMTP or OAuth configured locally | Test on staging |
| A real service-worker update prompt and push notifications | Update toast reviewed from its markup | Deploy twice to staging and watch the prompt |
| 2FA and passkey enrolment; change password / delete account | Not exercised (the last two don't exist yet — [ACCT-02](20-signed-in-experience.md)) | Test once built |
| Real Windows High Contrast themes; iOS Dynamic Type / Android font scale | Only Chrome's emulation | Check on devices |
| Production host (CDN, compression, real latency) | Local production build only | Lighthouse on the deployed URL |
| Every surah × every script × every font; automated glyph coverage | Sampled visually | Add the glyph-coverage test from [17](17-themes-palettes-and-scripts.md) |
