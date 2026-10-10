# quran.com reference mirror

## Purpose

The mirror is the audit reference for parity work against quran.com. It stores quran.com's
script-level assets exactly as served — the script JSON payloads from API v4 and the fonts
their reader pages load — under committed `web/static/qurancom/`. Parity checks compare our
databases and rendering against these exact files, so the inventory answers one question
directly: are we missing anything quran.com ships at script level?

Nothing in the mirror is app-referenced in production. Every catalogue entry in
`scripts/quran/qurancom-assets.ts` has `productionEnabled = false`; no production code path
loads these files (the dev-only `/design/qurancom` specimen reads them). They live under
`web/static/`, so the raw files do ship inside the static bundle
and remain fetchable by direct URL — that presence is accepted by campaign decision (gating is
env-var only; no middleware or build-exclusion machinery). `pnpm quran:mirror` proves the
on-disk mirror matches the catalogue by id + byte size.

## Production policy

- Production serves the Uthmani family only.
- IndoPak, Tajweed, and restricted assets are production-disabled. They stay committed and
  referenced here, gated by an app env var later. They are never hidden or dropped.
- Every mirror entry is reference/audit-only today (`productionEnabled = false`), including
  cleared ones — nothing here is wired into the app yet. Enabling any entry in production
  requires its licence to clear first (see Restricted and clearance).
- No production code path references the mirror (the dev-only `/design/qurancom` specimen
  reads it). The raw files do sit in the static bundle (see Purpose); remove or exclude them
  from the build if a stricter posture is wanted later. R12 (`docs/indopak-v4-verification-plan.md`:
  no restricted font asset in repo or build) is explicitly amended for this lane by campaign
  decision — committed audit references, production-disabled — while the build/app side keeps
  the Uthmani-only production policy. `uthmanic-hafs-v22.ttf` was already shipped pre-campaign
  (`9781438f`); this lane neither introduced nor widened that.

## Scope

In scope: 6 script payloads (`uthmani`, `indopak`, `indopak_nastaleeq`, `imlaei`,
`imlaei_simple`, `uthmani_tajweed`) and 18 font files (Uthmani Hafs, QuranWBW IndoPak
Nastaleeq 4.2.1-WL, surah names v1, Noto Naskh Arabic, Mehr Nastaliq Web, Noto Serif Bengali,
Droid Naskh, Figtree, Playfair Display, Dhivehi Akuru).

Deliberate exclusions: QCF v1/v2 page fonts and the `code_v1`/`code_v2` page-glyph scripts.
Page mushafs are per-page glyph fonts plus page-coded scripts — too heavy to store and cache
for audit purposes, and our reader does not use page-glyph rendering.

Unreachable paths: quran.com's `fonts.scss` lists Dhivehi at
`/fonts/lang/dhivehi/dhivehi/DhivehiAkuru.{woff2,woff,otf}`; all three return 404 after two
retries. The live alternate `/fonts/lang/dhivehi/DhivehiAkuru.woff2` is mirrored as
`font-dhivehi-akuru-woff2`. No catalogue entry exists for the 404 paths.

## Refresh

```bash
pnpm quran:mirror             # verify; fetch missing or size-mismatched files
pnpm quran:mirror --dry-run   # report only, no writes, no network
pnpm quran:mirror --record    # fetch all; print id -> bytes to rebake sizeBytes
```

Node 24 + tsx + global `fetch`. Writes go to `web/static/qurancom/` through `.part` + rename;
`db/` is never touched. Identity is id + baked byte size — no SHA-256 anywhere in the mirror
path (AGENTS.MD hard rule). Any size mismatch exits non-zero listing id/expected/actual, and
mismatched bytes never land on disk.

`web/static/qurancom/**` is excluded from `vp fmt` / `vp check` formatting
(`fmt.ignorePatterns` in `web/vite.config.ts` and in the root `vite.config.ts` — the staged
hook formats through the root config) so payloads stay byte-verbatim through commit hooks.
Never remove those ignores — the mirror verify (and its `quran:check` test) fails on any byte
drift.

## Asset table

| id                                               | kind   | source URL                                                                                       | license    | notice summary                                                     | target path                                                                    |
| ------------------------------------------------ | ------ | ------------------------------------------------------------------------------------------------ | ---------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------ |
| `script-uthmani`                                 | script | `https://api.quran.com/api/v4/quran/verses/uthmani`                                              | unknown    | no explicit license; verify before production use                  | `scripts/uthmani.json`                                                         |
| `script-indopak`                                 | script | `https://api.quran.com/api/v4/quran/verses/indopak`                                              | unknown    | no explicit license; verify before production use                  | `scripts/indopak.json`                                                         |
| `script-indopak-nastaleeq`                       | script | `https://api.quran.com/api/v4/quran/verses/indopak_nastaleeq`                                    | unknown    | no explicit license; verify before production use                  | `scripts/indopak_nastaleeq.json`                                               |
| `script-imlaei`                                  | script | `https://api.quran.com/api/v4/quran/verses/imlaei`                                               | unknown    | no explicit license; verify before production use                  | `scripts/imlaei.json`                                                          |
| `script-imlaei-simple`                           | script | `https://api.quran.com/api/v4/quran/verses/imlaei_simple`                                        | unknown    | no explicit license; verify before production use                  | `scripts/imlaei_simple.json`                                                   |
| `script-uthmani-tajweed`                         | script | `https://api.quran.com/api/v4/quran/verses/uthmani_tajweed`                                      | unknown    | no explicit license; verify before production use                  | `scripts/uthmani_tajweed.json`                                                 |
| `font-uthmanic-hafs-woff2`                       | font   | `https://quran.com/fonts/quran/hafs/uthmanic_hafs/UthmanicHafs1Ver18.woff2`                      | restricted | KFGQPC; restricted per repo policy (R12)                           | `fonts/quran/hafs/uthmanic_hafs/UthmanicHafs1Ver18.woff2`                      |
| `font-uthmanic-hafs-ttf`                         | font   | `https://quran.com/fonts/quran/hafs/uthmanic_hafs/UthmanicHafs1Ver18.ttf`                        | restricted | KFGQPC; restricted per repo policy (R12)                           | `fonts/quran/hafs/uthmanic_hafs/UthmanicHafs1Ver18.ttf`                        |
| `font-indopak-nastaleeq-waqf-lazim-v4.2.1-woff2` | font   | `https://quran.com/fonts/quran/hafs/nastaleeq/indopak/indopak-nastaleeq-waqf-lazim-v4.2.1.woff2` | restricted | QuranWBW notice (verbatim below)                                   | `fonts/quran/hafs/nastaleeq/indopak/indopak-nastaleeq-waqf-lazim-v4.2.1.woff2` |
| `font-indopak-nastaleeq-waqf-lazim-v4.2.1-woff`  | font   | `https://quran.com/fonts/quran/hafs/nastaleeq/indopak/indopak-nastaleeq-waqf-lazim-v4.2.1.woff`  | restricted | QuranWBW notice (verbatim below)                                   | `fonts/quran/hafs/nastaleeq/indopak/indopak-nastaleeq-waqf-lazim-v4.2.1.woff`  |
| `font-indopak-nastaleeq-waqf-lazim-v4.2.1-ttf`   | font   | `https://quran.com/fonts/quran/hafs/nastaleeq/indopak/indopak-nastaleeq-waqf-lazim-v4.2.1.ttf`   | restricted | QuranWBW notice (verbatim below)                                   | `fonts/quran/hafs/nastaleeq/indopak/indopak-nastaleeq-waqf-lazim-v4.2.1.ttf`   |
| `font-sura-names-v1-woff2`                       | font   | `https://quran.com/fonts/quran/surah-names/v1/sura_names.woff2`                                  | unknown    | quran.com-authored; verify before production use                   | `fonts/quran/surah-names/v1/sura_names.woff2`                                  |
| `font-sura-names-v1-woff`                        | font   | `https://quran.com/fonts/quran/surah-names/v1/sura_names.woff`                                   | unknown    | quran.com-authored; verify before production use                   | `fonts/quran/surah-names/v1/sura_names.woff`                                   |
| `font-noto-naskh-arabic-regular-woff2`           | font   | `https://quran.com/fonts/lang/arabic/NotoNaskhArabic-Regular.woff2`                              | cleared    | SIL OFL 1.1                                                        | `fonts/lang/arabic/NotoNaskhArabic-Regular.woff2`                              |
| `font-noto-naskh-arabic-regular-ttf`             | font   | `https://quran.com/fonts/lang/arabic/NotoNaskhArabic-Regular.ttf`                                | cleared    | SIL OFL 1.1                                                        | `fonts/lang/arabic/NotoNaskhArabic-Regular.ttf`                                |
| `font-mehr-nastaliq-web-woff2`                   | font   | `https://quran.com/fonts/lang/urdu/MehrNastaliqWeb.woff2`                                        | unknown    | no published terms; verify before production use                   | `fonts/lang/urdu/MehrNastaliqWeb.woff2`                                        |
| `font-mehr-nastaliq-web-woff`                    | font   | `https://quran.com/fonts/lang/urdu/MehrNastaliqWeb.woff`                                         | unknown    | no published terms; verify before production use                   | `fonts/lang/urdu/MehrNastaliqWeb.woff`                                         |
| `font-mehr-nastaliq-web-ttf`                     | font   | `https://quran.com/fonts/lang/urdu/MehrNastaliqWeb.ttf`                                          | unknown    | no published terms; verify before production use                   | `fonts/lang/urdu/MehrNastaliqWeb.ttf`                                          |
| `font-noto-serif-bengali-regular-woff2`          | font   | `https://quran.com/fonts/lang/bengali/NotoSerifBengali-Regular.woff2`                            | cleared    | SIL OFL 1.1                                                        | `fonts/lang/bengali/NotoSerifBengali-Regular.woff2`                            |
| `font-droid-naskh-regular-woff2`                 | font   | `https://quran.com/fonts/lang/kurdish/droid-naskh-regular.woff2`                                 | cleared    | Apache License 2.0                                                 | `fonts/lang/kurdish/droid-naskh-regular.woff2`                                 |
| `font-droid-naskh-regular-ttf`                   | font   | `https://quran.com/fonts/lang/kurdish/droid-naskh-regular.ttf`                                   | cleared    | Apache License 2.0                                                 | `fonts/lang/kurdish/droid-naskh-regular.ttf`                                   |
| `font-figtree-ttf`                               | font   | `https://quran.com/fonts/lang/Figtree/Figtree.ttf`                                               | cleared    | SIL OFL 1.1                                                        | `fonts/lang/Figtree/Figtree.ttf`                                               |
| `font-playfair-display-black-ttf`                | font   | `https://quran.com/fonts/lang/PlayfairDisplay/PlayfairDisplay-Black.ttf`                         | cleared    | SIL OFL 1.1                                                        | `fonts/lang/PlayfairDisplay/PlayfairDisplay-Black.ttf`                         |
| `font-dhivehi-akuru-woff2`                       | font   | `https://quran.com/fonts/lang/dhivehi/DhivehiAkuru.woff2`                                        | unknown    | fonts.scss path 404s; live alternate; verify before production use | `fonts/lang/dhivehi/DhivehiAkuru.woff2`                                        |

Target paths are relative to `web/static/qurancom/`; resolved absolute paths always stay under
that root.

## Restricted and clearance

Restricted entries stay in the repository as audit references with `productionEnabled = false`.
No production code path references them; their raw files share the static-bundle presence described
under Purpose. Clearing a licence requires a written basis recorded here; until then the entry
stays disabled.

- QuranWBW IndoPak Nastaleeq 4.2.1-WL — `font-indopak-nastaleeq-waqf-lazim-v4.2.1-woff2`,
  `font-indopak-nastaleeq-waqf-lazim-v4.2.1-woff`,
  `font-indopak-nastaleeq-waqf-lazim-v4.2.1-ttf`. Verbatim notice:

  > NOT FOR SALE, NOT FOR MODIFICATION, NOT FOR DISTRIBUTION OR NOT FOR DEVELOPMENT WITHOUT WRITTEN NOTICE BY QURANWBW.COM

  Contact for written notice: quranwbw@gmail.com.

- KFGQPC Uthmanic Script HAFS — `font-uthmanic-hafs-woff2`, `font-uthmanic-hafs-ttf`.
  Repo policy treats KFGQPC assets as restricted (`docs/indopak-v4-verification-plan.md` R12);
  written permission is needed before any production use.
- Branch note: `indopak-nastaleeq-hanafi-v4.2.2` (Hanafi variant) is a known restricted asset
  living on the unpushed `indopak` branch. It is not in this catalogue because it is not on
  this branch; gate it with the same env-var mechanism when that branch merges.
- Dhivehi: the three paths `fonts.scss` lists under `/fonts/lang/dhivehi/dhivehi/` return 404
  after retries, so they have no catalogue entries. The reachable alternate is mirrored as
  `font-dhivehi-akuru-woff2`.

## Manual digests

If a digest is ever needed, it follows the AGENTS.MD manual-audit rule — a human runs it by
hand, never a script; no command is prescribed here so the three-command manual-audit list
stays exact. No build, boot, runtime, ETag, cache, or catalogue path may
compute SHA-256 over these assets (AGENTS.MD hard rule; a DB's identity is its id, and a mirror
asset's identity is id + byte size).
