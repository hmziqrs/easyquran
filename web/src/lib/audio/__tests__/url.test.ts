import { describe, expect, it } from "vite-plus/test";

import type { AudioRecording } from "../catalogue";
import { isAllowedAudioUrl, resolveAyahAudio } from "../url";

// Fake catalogue entries, test-only. Hosts use the reserved .test TLD.
const FAKE_PER_AYAH: AudioRecording = {
  id: "fake-per-ayah",
  reciterId: "fake-reciter",
  label: "Test per-ayah",
  urlTemplate: "https://audio.example.test/{surah}{ayah}.mp3",
  timing: { kind: "file-per-ayah" },
  licence: { name: "Test licence", url: "https://licence.example.test/" },
  attribution: { en: "Test", ar: "اختبار", url: "https://source.example.test/" },
};

const FAKE_SURAH_FILE: AudioRecording = {
  ...FAKE_PER_AYAH,
  id: "fake-surah-file",
  urlTemplate: "https://audio.example.test/{surah}.mp3",
  timing: { kind: "surah-file", ayahStartMs: { 1: 0, 2: 1500, 3: 4000 } },
};

describe("resolveAyahAudio", () => {
  it("pads surah and ayah to three digits", () => {
    expect(resolveAyahAudio(FAKE_PER_AYAH, { surah: 2, ayah: 255 })).toEqual({
      url: "https://audio.example.test/002255.mp3",
      startSec: 0,
      endSec: null,
    });
  });

  it("rejects surahs outside 1..114 and ayahs below 1", () => {
    expect(resolveAyahAudio(FAKE_PER_AYAH, { surah: 0, ayah: 1 })).toBeNull();
    expect(resolveAyahAudio(FAKE_PER_AYAH, { surah: 115, ayah: 1 })).toBeNull();
    expect(resolveAyahAudio(FAKE_PER_AYAH, { surah: 1, ayah: 0 })).toBeNull();
    expect(resolveAyahAudio(FAKE_PER_AYAH, { surah: 1.5, ayah: 1 })).toBeNull();
  });

  it("rejects an ayah past the surah's ayah count", () => {
    expect(resolveAyahAudio(FAKE_PER_AYAH, { surah: 1, ayah: 8 }, 7)).toBeNull();
    expect(resolveAyahAudio(FAKE_PER_AYAH, { surah: 1, ayah: 7 }, 7)).not.toBeNull();
  });

  it("reads verse offsets from a surah file, converting ms to seconds", () => {
    expect(resolveAyahAudio(FAKE_SURAH_FILE, { surah: 1, ayah: 2 })).toEqual({
      url: "https://audio.example.test/001.mp3",
      startSec: 1.5,
      endSec: 4,
    });
  });

  it("plays the last timed ayah to the end of the file", () => {
    expect(resolveAyahAudio(FAKE_SURAH_FILE, { surah: 1, ayah: 3 })?.endSec).toBeNull();
  });

  it("returns null for an ayah with no offset", () => {
    expect(resolveAyahAudio(FAKE_SURAH_FILE, { surah: 1, ayah: 9 })).toBeNull();
  });
});

describe("isAllowedAudioUrl", () => {
  it("allows only https", () => {
    expect(isAllowedAudioUrl("https://audio.example.test/001001.mp3")).toBe(true);
    expect(isAllowedAudioUrl("http://audio.example.test/001001.mp3")).toBe(false);
    expect(isAllowedAudioUrl("javascript:alert(1)")).toBe(false);
    expect(isAllowedAudioUrl("not a url")).toBe(false);
  });
});
