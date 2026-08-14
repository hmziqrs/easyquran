import {
  OpenerKind,
  OpenerPackaging,
  QuranScript,
  QuranSourceId,
  type Ayah,
  type SurahNormalization,
} from "$lib/data/quran-types";
import { displayVerses, groupRangeAyahs } from "$lib/quran/view/presentation";
import { describe, expect, it } from "vite-plus/test";

const normalization: SurahNormalization = {
  surah: 2,
  sourceId: QuranSourceId.Uthmani,
  script: QuranScript.Uthmani,
  sourceProfile: "fixture",
  packaging: OpenerPackaging.EmbeddedPrefix,
  openerKind: OpenerKind.Header,
  openerText: "opener",
  openerEndScalar: 6,
  bodyStartScalar: 7,
};

const first: Ayah = {
  key: "2:1",
  surah: 2,
  ayah: 1,
  globalIndex: 8,
  text: "opener body",
};

const second: Ayah = {
  key: "2:2",
  surah: 2,
  ayah: 2,
  globalIndex: 9,
  text: "second",
};

describe("canonical reader presentation", () => {
  it("uses the same descriptor for a full surah body and header", () => {
    expect(
      displayVerses({
        sourceId: QuranSourceId.Uthmani,
        script: QuranScript.Uthmani,
        verses: [first.text, second.text],
        normalization,
      }),
    ).toEqual(["body", "second"]);
  });

  it("renders one opener only when a range contains ayah 1", () => {
    expect(groupRangeAyahs([first, second], [normalization])).toMatchObject([
      { surah: 2, opener: "opener", ayahs: [first, second] },
    ]);
    expect(groupRangeAyahs([second], [normalization])).toMatchObject([
      { surah: 2, opener: null, ayahs: [second] },
    ]);
  });
});
