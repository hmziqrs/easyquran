/* ════════════════════════════════════════════════════════════════════════
   quran.ts — the Quran content the reader works against.

   This is the same selection the design comp ships: a handful of surahs with
   their Uthmani verses, plus a "verse of the day". The full Tanzil dataset
   lives in /db (SQLite + SQL); when the reader is wired to the real source it
   swaps in here without touching the components — the types below are the
   contract.
   ════════════════════════════════════════════════════════════════════════ */

export interface Surah {
  /** surah number, 1..114 */
  num: number;
  /** transliterated name, e.g. "Al-Fatihah" */
  name: string;
  /** Arabic name */
  arabic: string;
  /** "Meccan" | "Medinan" */
  place: "Meccan" | "Medinan";
  /** true when this selection omits verses of a longer surah */
  partial?: boolean;
  /** Uthmani verse text, in order */
  verses: string[];
}

/** A coordinate into the text — "surah:ayah". */
export type VerseKey = string;

export const verseKey = (surah: number, ayah: number): VerseKey => `${surah}:${ayah}`;
export const parseKey = (key: VerseKey): { num: number; n: number } => {
  const [num, n] = key.split(":").map(Number);
  return { num, n };
};

export const SURAHS: Surah[] = [
  {
    num: 1,
    name: "Al-Fatihah",
    arabic: "الفاتحة",
    place: "Meccan",
    verses: [
      "بِسْمِ اللَّهِ الرَّحْمَٰنِ الرَّحِيمِ",
      "الْحَمْدُ لِلَّهِ رَبِّ الْعَالَمِينَ",
      "الرَّحْمَٰنِ الرَّحِيمِ",
      "مَالِكِ يَوْمِ الدِّينِ",
      "إِيَّاكَ نَعْبُدُ وَإِيَّاكَ نَسْتَعِينُ",
      "اهْدِنَا الصِّرَاطَ الْمُسْتَقِيمَ",
      "صِرَاطَ الَّذِينَ أَنْعَمْتَ عَلَيْهِمْ غَيْرِ الْمَغْضُوبِ عَلَيْهِمْ وَلَا الضَّالِّينَ",
    ],
  },
  {
    num: 2,
    name: "Al-Baqarah",
    arabic: "البقرة",
    place: "Medinan",
    partial: true,
    verses: [
      "الٓمٓ",
      "ذَٰلِكَ الْكِتَابُ لَا رَيْبَ فِيهِ هُدًى لِّلْمُتَّقِينَ",
      "الَّذِينَ يُؤْمِنُونَ بِالْغَيْبِ وَيُقِيمُونَ الصَّلَاةَ وَمِمَّا رَزَقْنَاهُمْ يُنفِقُونَ",
      "وَالَّذِينَ يُؤْمِنُونَ بِمَا أُنزِلَ إِلَيْكَ وَمَا أُنزِلَ مِن قَبْلِكَ وَبِالْآخِرَةِ هُمْ يُوقِنُونَ",
      "أُولَٰئِكَ عَلَىٰ هُدًى مِّن رَّبِّهِمْ وَأُولَٰئِكَ هُمُ الْمُفْلِحُونَ",
    ],
  },
  {
    num: 103,
    name: "Al-'Asr",
    arabic: "العصر",
    place: "Meccan",
    verses: [
      "وَالْعَصْرِ",
      "إِنَّ الْإِنسَانَ لَفِي خُسْرٍ",
      "إِلَّا الَّذِينَ آمَنُوا وَعَمِلُوا الصَّالِحَاتِ وَتَوَاصَوْا بِالْحَقِّ وَتَوَاصَوْا بِالصَّبْرِ",
    ],
  },
  {
    num: 105,
    name: "Al-Fil",
    arabic: "الفيل",
    place: "Meccan",
    verses: [
      "أَلَمْ تَرَ كَيْفَ فَعَلَ رَبُّكَ بِأَصْحَابِ الْفِيلِ",
      "أَلَمْ يَجْعَلْ كَيْدَهُمْ فِي تَضْلِيلٍ",
      "وَأَرْسَلَ عَلَيْهِمْ طَيْرًا أَبَابِيلَ",
      "تَرْمِيهِم بِحِجَارَةٍ مِّن سِجِّيلٍ",
      "فَجَعَلَهُمْ كَعَصْفٍ مَّأْكُولٍ",
    ],
  },
  {
    num: 106,
    name: "Quraysh",
    arabic: "قريش",
    place: "Meccan",
    verses: [
      "لِإِيلَافِ قُرَيْشٍ",
      "إِيلَافِهِمْ رِحْلَةَ الشِّتَاءِ وَالصَّيْفِ",
      "فَلْيَعْبُدُوا رَبَّ هَٰذَا الْبَيْتِ",
      "الَّذِي أَطْعَمَهُم مِّن جُوعٍ وَآمَنَهُم مِّنْ خَوْفٍ",
    ],
  },
  {
    num: 108,
    name: "Al-Kawthar",
    arabic: "الكوثر",
    place: "Meccan",
    verses: [
      "إِنَّا أَعْطَيْنَاكَ الْكَوْثَرَ",
      "فَصَلِّ لِرَبِّكَ وَانْحَرْ",
      "إِنَّ شَانِئَكَ هُوَ الْأَبْتَرُ",
    ],
  },
  {
    num: 109,
    name: "Al-Kafirun",
    arabic: "الكافرون",
    place: "Meccan",
    verses: [
      "قُلْ يَا أَيُّهَا الْكَافِرُونَ",
      "لَا أَعْبُدُ مَا تَعْبُدُونَ",
      "وَلَا أَنتُمْ عَابِدُونَ مَا أَعْبُدُ",
      "وَلَا أَنَا عَابِدٌ مَّا عَبَدتُّمْ",
      "وَلَا أَنتُمْ عَابِدُونَ مَا أَعْبُدُ",
      "لَكُمْ دِينُكُمْ وَلِيَ دِينِ",
    ],
  },
  {
    num: 110,
    name: "An-Nasr",
    arabic: "النصر",
    place: "Medinan",
    verses: [
      "إِذَا جَاءَ نَصْرُ اللَّهِ وَالْفَتْحُ",
      "وَرَأَيْتَ النَّاسَ يَدْخُلُونَ فِي دِينِ اللَّهِ أَفْوَاجًا",
      "فَسَبِّحْ بِحَمْدِ رَبِّكَ وَاسْتَغْفِرْهُ إِنَّهُ كَانَ تَوَّابًا",
    ],
  },
  {
    num: 112,
    name: "Al-Ikhlas",
    arabic: "الإخلاص",
    place: "Meccan",
    verses: [
      "قُلْ هُوَ اللَّهُ أَحَدٌ",
      "اللَّهُ الصَّمَدُ",
      "لَمْ يَلِدْ وَلَمْ يُولَدْ",
      "وَلَمْ يَكُن لَّهُ كُفُوًا أَحَدٌ",
    ],
  },
  {
    num: 113,
    name: "Al-Falaq",
    arabic: "الفلق",
    place: "Meccan",
    verses: [
      "قُلْ أَعُوذُ بِرَبِّ الْفَلَقِ",
      "مِن شَرِّ مَا خَلَقَ",
      "وَمِن شَرِّ غَاسِقٍ إِذَا وَقَبَ",
      "وَمِن شَرِّ النَّفَّاثَاتِ فِي الْعُقَدِ",
      "وَمِن شَرِّ حَاسِدٍ إِذَا حَسَدَ",
    ],
  },
  {
    num: 114,
    name: "An-Nas",
    arabic: "الناس",
    place: "Meccan",
    verses: [
      "قُلْ أَعُوذُ بِرَبِّ النَّاسِ",
      "مَلِكِ النَّاسِ",
      "إِلَٰهِ النَّاسِ",
      "مِن شَرِّ الْوَسْوَاسِ الْخَنَّاسِ",
      "الَّذِي يُوَسْوِسُ فِي صُدُورِ النَّاسِ",
      "مِنَ الْجِنَّةِ وَالنَّاسِ",
    ],
  },
];

/** Lookup by surah number; falls back to Al-Fatihah. */
export const surahByNum = (num: number): Surah => SURAHS.find((s) => s.num === num) ?? SURAHS[0];

/** Index-aware neighbours, wrapping at both ends. */
export const adjacentSurahs = (
  num: number,
): { prev: Surah; next: Surah } => {
  const idx = SURAHS.findIndex((s) => s.num === surahByNum(num).num);
  const len = SURAHS.length;
  return {
    prev: SURAHS[(idx - 1 + len) % len],
    next: SURAHS[(idx + 1) % len],
  };
};

export interface SearchResult {
  key: VerseKey;
  /** "Al-Fatihah 1:2" */
  ref: string;
  text: string;
  num: number;
  ayah: number;
}

/** Fuzzy verse search across the selection: matches surah name, number, or any
 *  verse text (Arabic substring). Caps at 24 hits, like the comp. */
export function searchVerses(rawQuery: string): SearchResult[] {
  const q = rawQuery.trim();
  if (!q) return [];
  const qLower = q.toLowerCase();
  const nameOrNumHit = (s: Surah) =>
    s.name.toLowerCase().includes(qLower) || s.arabic.includes(q) || String(s.num) === qLower;

  const out: SearchResult[] = [];
  for (const s of SURAHS) {
    const hitName = nameOrNumHit(s);
    s.verses.forEach((t, i) => {
      if ((hitName || t.includes(q)) && out.length < 24) {
        out.push({
          key: verseKey(s.num, i + 1),
          ref: `${s.name} ${s.num}:${i + 1}`,
          text: t,
          num: s.num,
          ayah: i + 1,
        });
      }
    });
  }
  return out;
}

/** "Meccan · 7 verses" / "Medinan · 5 of 286 verses". */
export const surahMeta = (s: Surah): string =>
  `${s.place} · ${s.verses.length}${s.partial ? " of 286 verses" : " verses"}`;

/** Sample commentary for a verse. In the full app this slot carries a short,
 *  credited tafsir summary. */
export const tafsirFor = (key: VerseKey): string => {
  const { num, n } = parseKey(key);
  const s = surahByNum(num);
  return `Sample commentary for ${s.name} ${key} — in the full app this slot carries a short, credited tafsir summary.`;
};

