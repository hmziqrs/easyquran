import { describe, expect, it } from "vite-plus/test";

import { normalizeArabic } from "#lib/quran/search/normalize.js";

import {
  createVoiceRecognizer,
  failureFor,
  finalTranscript,
  resolveSpeechConstructor,
  voiceLang,
  type SpeechAlternativeLike,
  type SpeechRecognitionLike,
  type SpeechResultEventLike,
  type SpeechResultLike,
  type SpeechResultListLike,
  type VoiceFailure,
} from "../recognition";

/** A fake engine that records calls and lets a test fire the browser's events. */
class FakeRecognition implements SpeechRecognitionLike {
  static instances: FakeRecognition[] = [];
  lang = "";
  continuous = true;
  interimResults = true;
  maxAlternatives = 0;
  onresult: ((event: SpeechResultEventLike) => void) | null = null;
  onerror: ((event: { readonly error: string }) => void) | null = null;
  onend: (() => void) | null = null;
  started = 0;
  stopped = 0;
  aborted = 0;
  startThrows = false;

  constructor() {
    FakeRecognition.instances.push(this);
  }

  start(): void {
    if (this.startThrows) throw new Error("InvalidStateError");
    this.started += 1;
  }
  stop(): void {
    this.stopped += 1;
  }
  abort(): void {
    this.aborted += 1;
  }
}

class PlainEngine extends FakeRecognition {}
class PrefixedEngine extends FakeRecognition {}

function resultEvent(
  pieces: readonly { transcript: string; isFinal: boolean }[],
  resultIndex = 0,
): SpeechResultEventLike {
  const items: SpeechResultLike[] = pieces.map((piece) => ({
    isFinal: piece.isFinal,
    0: { transcript: piece.transcript } satisfies SpeechAlternativeLike,
  }));
  const results: SpeechResultListLike = items;
  return { resultIndex, results };
}

function setup() {
  FakeRecognition.instances = [];
  const transcripts: string[] = [];
  const failures: VoiceFailure[] = [];
  const ends: number[] = [];
  createVoiceRecognizer(FakeRecognition, "en-US", {
    onTranscript: (text) => transcripts.push(text),
    onFailure: (failure) => failures.push(failure),
    onEnd: () => ends.push(1),
  });
  const engine = FakeRecognition.instances[0];
  if (engine === undefined) throw new Error("no engine constructed");
  return { engine, transcripts, failures, ends };
}

describe("speech constructor lookup", () => {
  it("prefers the unprefixed constructor and falls back to webkit", () => {
    expect(
      resolveSpeechConstructor({
        SpeechRecognition: PlainEngine,
        webkitSpeechRecognition: PrefixedEngine,
      }),
    ).toBe(PlainEngine);
    expect(resolveSpeechConstructor({ webkitSpeechRecognition: PrefixedEngine })).toBe(
      PrefixedEngine,
    );
  });

  it("returns null where the browser has no speech recognition", () => {
    expect(resolveSpeechConstructor({})).toBeNull();
  });
});

describe("language and error mapping", () => {
  it("listens in Saudi Arabic for the Arabic UI and US English otherwise", () => {
    expect(voiceLang("ar")).toBe("ar-SA");
    expect(voiceLang("en")).toBe("en-US");
  });

  it("maps denied, no-speech and other errors; aborted is not a failure", () => {
    expect(failureFor("not-allowed")).toBe("denied");
    expect(failureFor("service-not-allowed")).toBe("denied");
    expect(failureFor("no-speech")).toBe("no-speech");
    expect(failureFor("network")).toBe("failed");
    expect(failureFor("audio-capture")).toBe("failed");
    expect(failureFor("language-not-supported")).toBe("failed");
    expect(failureFor("aborted")).toBeNull();
  });
});

describe("transcript extraction", () => {
  it("joins only final pieces from the changed index onward", () => {
    const event = resultEvent(
      [
        { transcript: "al fatiha", isFinal: true },
        { transcript: "al fat", isFinal: false },
        { transcript: "surah two", isFinal: true },
      ],
      0,
    );
    expect(finalTranscript(event)).toBe("al fatiha surah two");
  });

  it("returns an empty string when nothing is final", () => {
    expect(finalTranscript(resultEvent([{ transcript: "partial", isFinal: false }]))).toBe("");
  });

  it("passes Arabic transcripts through verbatim and normalizes them like typed input", () => {
    const spoken = "بِسْمِ ٱللَّهِ الرَّحْمَٰنِ";
    const typed = "بسم الله الرحمن";
    const { engine, transcripts } = setup();
    engine.onresult?.(resultEvent([{ transcript: spoken, isFinal: true }]));
    expect(transcripts).toEqual([spoken]);
    expect(normalizeArabic(transcripts[0] ?? "")).toBe(normalizeArabic(typed));
  });
});

describe("createVoiceRecognizer", () => {
  it("configures a single final result per session in the given language", () => {
    const { engine } = setup();
    expect(engine.lang).toBe("en-US");
    expect(engine.continuous).toBe(false);
    expect(engine.interimResults).toBe(false);
    expect(engine.maxAlternatives).toBe(1);
  });

  it("reports a failed start instead of throwing", () => {
    FakeRecognition.instances = [];
    const failures: VoiceFailure[] = [];
    const recognizer = createVoiceRecognizer(FakeRecognition, "en-US", {
      onTranscript: () => {},
      onFailure: (failure) => failures.push(failure),
      onEnd: () => {},
    });
    const engine = FakeRecognition.instances[0];
    if (engine === undefined) throw new Error("no engine constructed");
    engine.startThrows = true;
    expect(() => recognizer.start()).not.toThrow();
    expect(failures).toEqual(["failed"]);
  });

  it("reports denied microphone access and still ends the session", () => {
    const { engine, failures, ends, transcripts } = setup();
    engine.onerror?.({ error: "not-allowed" });
    engine.onend?.();
    expect(failures).toEqual(["denied"]);
    expect(ends).toEqual([1]);
    expect(transcripts).toEqual([]);
  });

  it("reports a silent session as no-speech via the error event", () => {
    const { engine, failures } = setup();
    engine.onerror?.({ error: "no-speech" });
    expect(failures).toEqual(["no-speech"]);
  });

  it("treats an abort as a normal end, not a failure", () => {
    const { engine, failures, ends } = setup();
    engine.onerror?.({ error: "aborted" });
    engine.onend?.();
    expect(failures).toEqual([]);
    expect(ends).toEqual([1]);
  });

  it("forwards start, stop and abort to the engine", () => {
    FakeRecognition.instances = [];
    const recognizer = createVoiceRecognizer(FakeRecognition, "ar-SA", {
      onTranscript: () => {},
      onFailure: () => {},
      onEnd: () => {},
    });
    const engine = FakeRecognition.instances[0];
    if (engine === undefined) throw new Error("no engine constructed");
    recognizer.start();
    recognizer.stop();
    recognizer.abort();
    expect(engine.started).toBe(1);
    expect(engine.stopped).toBe(1);
    expect(engine.aborted).toBe(1);
    expect(engine.lang).toBe("ar-SA");
  });
});
