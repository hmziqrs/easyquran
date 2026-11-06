/**
 * Word-by-word contracts (F21). A word source is a licensed, baked id table
 * keyed by canonical word coordinates. Display only: the Arabic bytes come
 * from the reader text and are never rewritten by this module.
 */

export type WordDirection = "ltr" | "rtl";

/** Canonical coordinate: surah, ayah and word are 1-based. */
export interface WordCoordinate {
  readonly surah: number;
  readonly ayah: number;
  readonly word: number;
}

/** `surah:ayah:word`, the only key shape a word table is indexed by. */
export type WordKey = `${number}:${number}:${number}`;

export interface WordEntry {
  readonly meaning: string;
  readonly transliteration: string;
}

export interface WordSource {
  /** Immutable catalogue-style id, like a translation id. */
  readonly id: string;
  /** BCP-47 language of `meaning`. */
  readonly meaningLanguage: string;
  readonly meaningDirection: WordDirection;
  /** Licence name or SPDX id, shown in every popover. */
  readonly licence: string;
  /** Credited author or work, shown in every popover. */
  readonly attribution: string;
  /**
   * Offline rule: entries ship baked into the app. The literal type forbids a
   * runtime network source, so a word source always works offline.
   */
  readonly delivery: "bundled";
  /** Per-surah table. Called once per surah and cached by the engine. */
  readonly load: (surah: number) => Promise<ReadonlyMap<WordKey, WordEntry>>;
}

export function wordKey(coordinate: WordCoordinate): WordKey {
  return `${coordinate.surah}:${coordinate.ayah}:${coordinate.word}`;
}
