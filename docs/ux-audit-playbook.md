# UX audit playbook

How to audit every reader-facing screen and write findings someone can fix and re-check. Run it
after a big UI change lands (next: once the next-features UI merges). Capture tooling:
[`scripts/ux-audit/`](../scripts/ux-audit/README.md).

The one question for every screen: **would a first-time, non-technical reader — on a phone, maybe
older, maybe Arabic-first, maybe unable to read Arabic — understand it at once?**

## 1. Scope

Every screen a reader can reach, in both UI locales (English, Arabic), both modes (light, dark),
at phone, tablet and desktop widths.

| Area              | Routes and surfaces                                                                                                     |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Marketing         | `/`, `/ar/`, `/about`, `/faq`, `/contact`, `/privacy`, `/terms`, an unknown path (404)                                  |
| Indexes           | `/surah`, `/juz`, `/pages` (+ `/ar/…`)                                                                                  |
| Reader            | `/al-fatihah`, `/al-baqarah`, `/an-nas`, `/page/1`, `/juz/1`, `/hizb/1` (+ `/ar/…`); verse and reading mode             |
| Translated reader | `/al-baqarah/t/en/sahih`, `/al-fatihah/t/en/sahih?more=ur.jalandhry`, `/t/en/sahih/page/1`, an RTL translation          |
| Overlays          | header menu, appearance panel, reader sidebar, translation picker, ⌘K palette, verse tools and notes                    |
| Personal          | `/yours`, `/bookmarks` (empty and with data), `/search` (empty, English word, Arabic word), `/settings` (every section) |
| Account           | `/login`, `/register`, `/forgot-password`, `/verify-email`, account page, sign-out                                      |
| Edge states       | offline, API unreachable, slow network, 320 px width, trailing-slash and malformed reader URLs                          |

After the next-features merge, add: snippets (`/azkar`), the `/yours` widgets and session switcher,
the merge-review dialog on sign-in, locale suggestion, search recents, reading-analytics opt-in,
bookmark folders and undo, and the sign-out and session-expired notices.

**Second pass (release readiness):** four palettes × light/dark on ten key routes; custom colours;
every Arabic script × font; browser text size, forced colours, increased contrast, reduced motion;
280–2560 px plus landscape; signed-in flows end to end; keyboard and screen-reader paths through
every overlay; throttled loading on a production build; manifest, icons, tab titles, link previews;
WebKit (iPhone profile) against Chromium.

## 2. Environment

- Provision Quran DBs once: `just quran-fetch`.
- Web: `cd web && PUBLIC_ENV=local bun run dev --port 5391 --strictPort`.
- API (translations, account, sync): `cd rust && cargo run -p ruxlog`. Run part of the audit
  **without** the API and label those findings "API unreachable" — real for an offline-first app
  on bad signal, but not the healthy path.
- Production-build checks (loading, offline pack, PWA, analytics): build locally and serve it;
  never audit against the live site with test traffic.
- Signed-in checks: a throwaway local account in the local dev DB only. Delete it afterwards.
- Freeze app code for the duration and record the commit the audit ran on.

## 3. Methods

1. **First-time walkthrough.** Per screen: Can I tell where I am? Can I tell what this does before
   I tap it? Is the Qur'an text the clearest thing on the screen? Would my grandparent understand
   this word?
2. **Parity sweep.** Compare the same thing everywhere it appears — surah names, search boxes,
   buttons, selected states, theme switches, page widths, tab titles, icons. Log every mismatch.
3. **Automated accessibility.** axe-core (WCAG 2.0/2.1/2.2 A+AA + best practice) on the key routes
   × both modes (`"axe": true` in a shot spec).
4. **Measured probes.** Tap-target sizes, text under 14 px, contrast of specific elements against
   the colour actually behind them (`"probe"` in a shot spec).
5. **Display conditions** via Chrome DevTools Protocol: `Page.setFontSizes` (browser text size),
   `Emulation.setEmulatedMedia` (forced colours, increased contrast, reduced motion),
   `Accessibility.getFullAXTree` (screen-reader view), network/CPU throttling with
   PerformanceObserver for CLS/LCP.
6. **Code reading** to confirm each cause and name the file to change.

## 4. Severity

| Level  | Meaning                                                                              | Rule of thumb           |
| ------ | ------------------------------------------------------------------------------------ | ----------------------- |
| **P0** | Breaks trust or blocks a core task (read, find a verse, come back to where you were) | Fix before next release |
| **P1** | Real friction for many readers, or a clear WCAG 2.2 AA failure                       | Fix this cycle          |
| **P2** | Inconsistency or polish that makes the app feel unfinished                           | Schedule                |
| **P3** | Nice to have                                                                         | Backlog                 |

## 5. Writing findings

IDs are `AREA-NN`, one prefix per area: `NAV` navigation · `HOME` home · `RDR` reader · `TR`
translations · `SRCH` search · `LIST` indexes · `BM` bookmarks/notes · `SET` settings · `A11Y`
accessibility · `RTL` Arabic UI · `VIS` visual consistency · `STATE` loading/empty/error/offline ·
`AUTH` sign-in · `ACCT` account · `MKT` marketing · `THEME` palettes · `SCRIPT` Arabic scripts and
fonts · `DISP` display conditions · `FLOW` multi-step flows · `KEY` keyboard and screen reader ·
`INT` interaction details · `LOAD` loading performance · `PWA` installed app and metadata · `BRW`
cross-browser.

```
### AREA-NN · Short title
Severity · Who it hurts · Where (route + file)
Screenshot (red boxes mark the problem)
What's wrong — plain description
Why it matters — for a non-technical reader
Fix — concrete change, with the file to edit
Done when — a check anyone can run
```

Output, in `docs/research/ux-audit/`: a README (scope, commit, headline themes, P0 list, finding
index), one document per area, and one release checklist with every finding and its done-when
check. Keep only screenshots a write-up references; scratch captures go to `.cache/ux-audit/`.

## 6. Guardrails to propose

Turn recurring findings into tests so they stay fixed: contrast floors in
`token-contrast.test.ts`; a target-size check at 390 px on app routes; a lint ban on sub-13.5 px
text and `font-mono` in app routes; an i18n test for string literals in app `.svelte` files; one
source for surah names across sidebar, lists and palette; axe in CI on the key routes × both modes
(fail on serious/critical); a route test that every 404 renders the app shell.

## 7. Validate with real people

Five 20-minute sessions with the real audience — two older readers, one Arabic-first reader, one
who can't read Arabic, one phone-only user:

1. "Open Surah Al-Kahf and read verse 10 with an English translation."
2. "Save this verse so you can find it tomorrow, then find it."
3. "Make the Arabic text bigger."
4. "Find verses about patience."
5. "Switch the app to Arabic, close it, open it again."

Success = each task in under a minute without help. Every hesitation is a finding.

## 8. Before tagging the release

- [ ] Re-run the captures on the key routes and compare with the audit's screenshots.
- [ ] axe: zero serious/critical on the ten key routes × every palette × light/dark.
- [ ] Manual pass on a real iPhone (Safari) and a real Android phone (Chrome): read a surah, add a
      translation, bookmark, search, switch to Arabic, go offline.
- [ ] 20-minute VoiceOver or TalkBack pass: home → reader → bookmark → settings.
- [ ] The five-task test above.
- [ ] Every English string has an Arabic twin; Arabic UI walked end to end after a hard refresh.
- [ ] `bun run check`, `bun run lint`, `bun run test` green (in `web/`).

## 9. Coverage this environment can't give

| Gap                                                                | Do instead                        |
| ------------------------------------------------------------------ | --------------------------------- |
| Real iPhone Safari and Android Chrome                              | 30-minute manual pass on each     |
| Firefox (Playwright Firefox does not launch on macOS here)         | Manual Firefox pass               |
| Real screen readers (VoiceOver, TalkBack, NVDA)                    | 20-minute pass each               |
| Real users                                                         | §7                                |
| Real email and real Google/Apple sign-in                           | Test on staging                   |
| Service-worker update prompt and push notifications                | Deploy twice to staging and watch |
| Windows High Contrast themes, iOS Dynamic Type, Android font scale | Check on devices                  |
| Production host (CDN, compression, latency)                        | Lighthouse on the deployed URL    |
| Every surah × script × font                                        | Automated glyph-coverage test     |
