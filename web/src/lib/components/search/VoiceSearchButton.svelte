<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { UiLocale } from "#lib/i18n/locales.js";
  import {
    search_voice_denied,
    search_voice_failed,
    search_voice_listening,
    search_voice_no_speech,
    search_voice_privacy,
    search_voice_start,
    search_voice_stop,
  } from "#lib/i18n/m/search.js";
  import { getLocale } from "#lib/paraglide/runtime.js";
  import { Icon } from "#lib/components/icon/index.js";
  import {
    browserSpeechConstructor,
    createVoiceRecognizer,
    voiceLang,
    type SpeechRecognitionCtor,
    type VoiceFailure,
    type VoiceRecognizer,
  } from "#lib/search/voice/recognition.js";

  /**
   * Voice input for a search box (F15). Renders nothing where the browser has no
   * speech recognition. The transcript goes to `onTranscript` as plain text, the
   * same path typed input takes, so Arabic normalization is unchanged.
   */
  let { onTranscript }: { onTranscript: (text: string) => void } = $props();

  type Status = "idle" | "listening" | VoiceFailure;

  // SAFETY: paraglide is compiled for exactly the UI locales (en/ar in messages/), so getLocale() only ever returns a UiLocale at runtime.
  const locale = getLocale() as UiLocale;
  const text = (message: (inputs?: undefined, options?: { locale?: UiLocale }) => string): string =>
    message(undefined, { locale });
  const copy = {
    start: text(search_voice_start),
    stop: text(search_voice_stop),
    listening: text(search_voice_listening),
    denied: text(search_voice_denied),
    noSpeech: text(search_voice_no_speech),
    failed: text(search_voice_failed),
    privacy: text(search_voice_privacy),
  };

  let supported = $state(false);
  let status = $state<Status>("idle");
  const listening = $derived(status === "listening");

  let ctor: SpeechRecognitionCtor | null = null;
  let recognizer: VoiceRecognizer | null = null;

  onMount(() => {
    ctor = browserSpeechConstructor();
    supported = ctor !== null;
  });

  onDestroy(() => {
    recognizer?.abort();
    recognizer = null;
  });

  function statusText(current: Status): string {
    if (current === "listening") return copy.listening;
    if (current === "denied") return copy.denied;
    if (current === "no-speech") return copy.noSpeech;
    if (current === "failed") return copy.failed;
    return "";
  }

  function start(): void {
    if (ctor === null) return;
    // Events from a session that was replaced must not change the current one's status.
    recognizer?.abort();
    status = "listening";
    const active: VoiceRecognizer = createVoiceRecognizer(ctor, voiceLang(locale), {
      onTranscript: (transcript) => {
        if (recognizer !== active) return;
        status = "idle";
        onTranscript(transcript);
      },
      onFailure: (failure) => {
        if (recognizer !== active) return;
        status = failure;
      },
      onEnd: () => {
        if (recognizer !== active) return;
        if (status === "listening") status = "no-speech";
      },
    });
    recognizer = active;
    active.start();
  }

  function toggle(): void {
    if (listening) {
      recognizer?.stop();
      return;
    }
    start();
  }
</script>

{#if supported}
  <div class="flex flex-col gap-1.5">
    <div class="flex flex-wrap items-center gap-2">
      <button
        type="button"
        onclick={toggle}
        class="flex h-8 items-center gap-1.5 rounded-pill border border-border-strong bg-surface px-3 text-[12.5px] text-foreground-secondary transition-colors hover:border-border hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
      >
        <Icon name="mic" size={14} class={listening ? "text-primary" : ""} />
        <span>{listening ? copy.stop : copy.start}</span>
      </button>
      <p role="status" aria-live="polite" class="min-w-0 text-[12.5px] text-foreground-secondary">
        {statusText(status)}
      </p>
    </div>
    <p class="text-[11.5px] leading-normal text-muted">{copy.privacy}</p>
  </div>
{/if}
