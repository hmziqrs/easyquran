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

## Environment

- Date: 2026-09-29, branch `master` at `49e33b04`.
- Web: `PUBLIC_ENV=local pnpm dev` (Vite dev server, SQLite served from the local `db/quran/`).
- API: the first half of the session ran **without** the Rust API; the second half ran it with `cargo run -p ruxlog` on `:8888`. Findings that only appear when the API is unreachable are labelled **"API unreachable"** — they are real for an offline-first app (a phone on bad signal is the same situation) but are not what a user sees when everything is healthy.
- Browser: headless Chrome (Puppeteer from `web/node_modules`), fresh profile per screenshot, `prefers-color-scheme` emulated, device scale 1× desktop / 2× phone.

## Methods

1. **Walkthrough as a first-time, non-technical reader.** Each screen was judged on: *Can I tell where I am? Can I tell what this does before I tap it? Is the Qur'an text the clearest thing on the screen? Would my grandparent understand this word?*
2. **Parity sweep.** The same thing (a surah name, a search box, a button, a "selected" state, a theme switch) was compared everywhere it appears. Every mismatch is logged.
3. **Automated accessibility scan.** [axe-core 4.10](https://github.com/dequelabs/axe-core) with WCAG 2.0/2.1/2.2 A+AA + best-practice rules, on 18 routes × 2 themes (36 scans). Raw results are summarised in [09 · Accessibility](09-accessibility.md).
4. **Measured probes.** A script measured every interactive element's size (tap targets), every text node under 14 px, and the contrast of specific elements (verse numbers, tool icons, wordmark, chips) against the colour actually behind them.
5. **Code reading** to confirm the cause of each finding and point at the file that needs to change.

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

## Limits

- Only Chrome was used. Safari (iOS) and Firefox were not checked; RTL shaping and font fallback can differ there.
- A real screen reader (VoiceOver/TalkBack/NVDA) was not driven. axe catches structure problems, not the full listening experience — schedule a 30-minute VoiceOver pass after the P0/P1 fixes.
- No real users were interviewed. Findings marked "for non-technical readers" are expert judgement; validate the top ones with 3–5 people from the target audience (see [16 · Roadmap](16-fix-roadmap.md#validate-with-real-people)).
