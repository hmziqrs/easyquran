# IndoPak v4 verification progress

Updated 2026-10-06. Candidate font: `584569c`; verification plan: `7dd3e0e`.
Font bytes unchanged. This records completed Phase A/B work; **v4 has not completed the
verification plan or received editorial approval**.

Evidence lives on this Mac in `.cache/indopak-v4/2026-10-06-run1/`. Tracked corpus summary:
[`segmentation-report.json`](../scripts/fonts/indopak/segmentation-report.json).

| Phase                                                       | Status                                                    | Evidence                                                    |
| ----------------------------------------------------------- | --------------------------------------------------------- | ----------------------------------------------------------- |
| A — static font verification                                | 9/10 pass; name-record decision pending                   | `phaseA/summary.json`, `env.json`, logs                     |
| B — corpus segmentation                                     | Pass: 6,236 verses, 78,292 boxes, zero failures           | `phaseB/segmentation-full.json`, tracked summary            |
| B — web gates after renderer fix                            | Pass: check zero errors/warnings, lint clean, 2,188 tests | `phaseB/web-check.log`, `web-lint.log`, `web-test.log`      |
| B — browser regression                                      | Pass: 32 regression matrices per engine; exact DOM text   | `phaseB/browser-regression.json`, captures                  |
| C — harness upgrades and full ink/reader/platform runs      | Pending                                                   | Partial uncommitted `deep_audit.py` change; `phaseC-smoke/` |
| D–H — reference, Gecko, devices, delivery, editorial review | Pending                                                   | Owner decisions in plan §12                                 |

## Phase A name-record decision

Two rebuilds match packaged TTF (242,176 bytes) and WOFF2 (91,876 bytes). Eleven font tests
pass; all 6,236 verses shape without `.notdef`. Attribution in name IDs 8, 9, 11 and 12
contains SIL, while family/style/full/PostScript records (IDs 1–6) are clean. The plan's
strict name-record wording therefore remains unsatisfied. No names or font bytes changed.

## Phase B renderer result

The same segmentation rule now applies to every word, including the final word. Signs
after source whitespace attach to the preceding word; following letters open their own
box. This fixes 29 pause-sign prefixes and merged boxes in 19:17, 91:14 and 97:5.
Annotation whitespace plus trailing format characters remain in DOM text while drawing
at zero width (4:171). The specimen opens both Quran DBs read-only with `immutable=1`.

The corpus audit checks exact source reconstruction, offsets and keys, break positions,
stop flags, annotations, glyph advances and annotation ink boxes. All 223 context classes
map to 170 capture verses; another 148 review verses cover exceptional starts, stacks,
splits and predicted overflow.

Eighteen reviewed exceptions remain: 76:17 has one invisible verse-initial sign-only box;
17 verses begin with invisible U+200B. Nothing precedes these format characters, and they
paint no ink. Their verse keys and offsets appear in the tracked summary.

At 56px, the model predicts overflow for 12:21 and 56:23 in a 286px specimen column, and
12:21, 18:110 and 56:23 in a 272px reader column. The 12:21 specimen excess includes the
0.27em stop margin. 56:23 contains a source token joining words without whitespace.
**Actual reader widths and clipping still require Phase C verification.**

## Browser regression scope

Chromium 153.0.8010.12 and Playwright WebKit 26.6 each pass 30 full-specimen matrices:
both reading/verse modes × 22/24/33/48/56px × 320/640/960px. Each matrix includes 1,280
specimens, including the Phase B capture and review keys. Noninitial word boxes start with
letters, final words remain unbroken, and overflow is limited to the predicted verses.
Raw results: `phaseB/browser-all-regression.json`.

An additional regression run passes 32 matrices per engine across 387 specimens. It
checks exact DOM text against the immutable DB, measured word widths, and a 390×844 phone
viewport (48px/320px in both modes). All 170 context-capture keys and 148 review keys are
present. No unexpected overflow or page errors occur. Results:
`phaseB/browser-regression.json`; command output: `phaseB/browser-regression.log`.

Reviewed Chromium captures at 56px/320px show 19:17 and 91:14 no longer held in merged
boxes. Captures also cover 97:5, 2:229, 12:21, 18:110 and 56:23 in both modes.

Measured Chromium widths at 56px in the 286px specimen column:

| Verse  | Widest box | Box plus margin | Result                                                 |
| ------ | ---------- | --------------- | ------------------------------------------------------ |
| 12:21  | 277.48px   | 292.60px        | Stop margin causes excess                              |
| 19:17  | 185.89px   | 201.01px        | Fits after segmentation fix                            |
| 91:14  | 203.47px   | 218.59px        | Fits after segmentation fix                            |
| 97:5   | 133.95px   | 141.14px        | Fits after segmentation fix                            |
| 56:23  | 321.77px   | 326.25px        | Unbreakable source token plus marker exceeds column    |
| 18:110 | 273.48px   | 273.48px        | Fits specimen; predicted excess in 272px reader column |

These checks do not constitute Phase C ink, ring/digit, end-sign clearance, real-reader
clipping, native Safari, or flow-matrix approval. Existing v3/v4 browser reports predate
this renderer fix and cannot establish those passes.

## Reproduce

```sh
node scripts/fonts/indopak/segmentation_audit.mjs
pnpm --dir web check
pnpm --dir web lint
pnpm --dir web test
pnpm --dir web dev --port 5391 --strictPort
node .cache/indopak-v4/2026-10-06-run1/phaseB/browser-regression.mjs
```

The last command uses an ignored local evidence script, requiring this Mac. The corpus
tool and its summary are tracked and work on a fresh clone after DB/font provisioning.
Full handoff: `.cache/indopak-v4/2026-10-06-run1/HANDOFF.md`.
