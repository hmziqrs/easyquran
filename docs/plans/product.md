# Product and copy

[Back to index](../README.md)

Product features, then the copy truth pass.

## Product

#### F19. Recitation audio and player

Build: Engine, queue, mini and full player, speed, media session, retry, offline limits and reader entry points are built and dark (`web/src/lib/audio/`). Left: add the licensed source to `web/src/lib/audio/catalogue.ts`, its CSP entries (listed in that file), and attribution text.
Files: `web/src/lib/audio/catalogue.ts`, CSP config
Done when: continuous surah playback survives navigation and backgrounding; failures recover.
Blocked by: O22

#### F20. Memorization repeat engine

Build: Built and dark (`web/src/lib/audio/repeat.ts`). Turns on with F19. Left: run it against a real recording.
Files: none expected
Done when: a 3-verse range repeats the set count with the delay and stops cleanly.
Blocked by: F19

#### F21. Word-by-word

Build: Coordinates, tokens, popovers and gate are built and dark (`web/src/lib/quran/words/`). Left: add the licensed source to the registry with meaning, transliteration, licence and offline delivery.
Files: `web/src/lib/quran/words/`
Done when: every token maps to its source word; reader text bytes are unchanged.
Blocked by: O23

#### F22. More tafsir sources

Build: Source switching, credit, missing-verse note and offline rules already work. Left: add each licensed tafsir as a new immutable database id in the baked map and the tafsir allow-list. Footnotes and citation links need a data field the sources must supply.
Files: `web/src/lib/data/translations.json`, tafsir allow-list
Done when: each new source is credited and missing verses show a clear state.
Blocked by: O24

## Copy
