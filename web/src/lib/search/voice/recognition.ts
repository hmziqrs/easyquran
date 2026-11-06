/**
 * Voice search input (F15). A thin wrapper over the browser's own speech
 * recognition (`SpeechRecognition`, or `webkitSpeechRecognition` on WebKit and
 * older Blink). EasyQuran never captures, records or uploads audio itself: the
 * browser owns the microphone and its speech service. The final transcript is
 * handed back verbatim; Arabic normalization happens in the existing search
 * pipeline, the same as for typed input.
 *
 * Browsers without the API (Firefox, for one) get `null` from
 * `browserSpeechConstructor()`, and the control stays hidden there.
 */

export interface SpeechAlternativeLike {
  readonly transcript: string;
}

export interface SpeechResultLike {
  readonly isFinal: boolean;
  readonly [index: number]: SpeechAlternativeLike | undefined;
}

export interface SpeechResultListLike {
  readonly length: number;
  readonly [index: number]: SpeechResultLike | undefined;
}

export interface SpeechResultEventLike {
  readonly resultIndex: number;
  readonly results: SpeechResultListLike;
}

export interface SpeechErrorEventLike {
  readonly error: string;
}

export interface SpeechRecognitionLike {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  maxAlternatives: number;
  onresult: ((event: SpeechResultEventLike) => void) | null;
  onerror: ((event: SpeechErrorEventLike) => void) | null;
  onend: (() => void) | null;
  start(): void;
  stop(): void;
  abort(): void;
}

export type SpeechRecognitionCtor = new () => SpeechRecognitionLike;

/** Why a listening session ended without a transcript. `aborted` is not a failure. */
export type VoiceFailure = "denied" | "no-speech" | "failed";

export interface VoiceHandlers {
  readonly onTranscript: (text: string) => void;
  readonly onFailure: (failure: VoiceFailure) => void;
  readonly onEnd: () => void;
}

export interface VoiceRecognizer {
  start(): void;
  stop(): void;
  abort(): void;
}

// The DOM lib has no typings for the speech constructors; declare the runtime
// shape the browser exposes (unprefixed, or WebKit/old Blink prefixed).
declare global {
  interface Window {
    SpeechRecognition?: SpeechRecognitionCtor;
    webkitSpeechRecognition?: SpeechRecognitionCtor;
  }
}

type SpeechGlobals = Pick<Window, "SpeechRecognition" | "webkitSpeechRecognition">;

/** The constructor a scope exposes, unprefixed first. Null when the scope has neither. */
export function resolveSpeechConstructor(scope: SpeechGlobals): SpeechRecognitionCtor | null {
  return scope.SpeechRecognition ?? scope.webkitSpeechRecognition ?? null;
}

/** Null on the server and in browsers without speech recognition. */
export function browserSpeechConstructor(): SpeechRecognitionCtor | null {
  if (globalThis.window === undefined) return null;
  return resolveSpeechConstructor(window);
}

/** Speech language for the UI locale. Arabic UI listens as Saudi Arabic, English as US English. */
export function voiceLang(locale: string): string {
  return locale === "ar" ? "ar-SA" : "en-US";
}

/** Maps a `SpeechRecognitionError.error` code. `aborted` returns null: the page or user stopped it. */
export function failureFor(code: string): VoiceFailure | null {
  if (code === "aborted") return null;
  if (code === "not-allowed" || code === "service-not-allowed") return "denied";
  if (code === "no-speech") return "no-speech";
  return "failed";
}

/** Final transcript pieces from one result event, joined in order. Interim text is ignored. */
export function finalTranscript(event: SpeechResultEventLike): string {
  const parts: string[] = [];
  for (let i = event.resultIndex; i < event.results.length; i += 1) {
    const result = event.results[i];
    if (result === undefined || !result.isFinal) continue;
    const alternative = result[0];
    if (alternative !== undefined) parts.push(alternative.transcript);
  }
  return parts.join(" ").trim();
}

export function createVoiceRecognizer(
  ctor: SpeechRecognitionCtor,
  lang: string,
  handlers: VoiceHandlers,
): VoiceRecognizer {
  const recognition = new ctor();
  recognition.lang = lang;
  recognition.continuous = false;
  recognition.interimResults = false;
  recognition.maxAlternatives = 1;
  recognition.onresult = (event) => {
    const text = finalTranscript(event);
    if (text.length > 0) handlers.onTranscript(text);
  };
  recognition.onerror = (event) => {
    const failure = failureFor(event.error);
    if (failure !== null) handlers.onFailure(failure);
  };
  recognition.onend = () => handlers.onEnd();
  return {
    start() {
      try {
        recognition.start();
      } catch {
        // start() throws when a session is already running; report it as a failed attempt.
        handlers.onFailure("failed");
      }
    },
    stop() {
      recognition.stop();
    },
    abort() {
      recognition.abort();
    },
  };
}
