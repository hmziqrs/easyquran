<script lang="ts">
  // Reader verse-toolbar actions for the audio engine. Loaded only through a
  // dynamic import from VerseTools, and only once a recording is licensed.
  import { audio } from "#lib/audio/engine.svelte.js";
  import { defaultRecordingId } from "#lib/audio/catalogue.js";
  import { getAudioCopy } from "#lib/i18n/audio-copy.js";
  import { Icon } from "#lib/components/icon/index.js";

  let {
    surah,
    ayah,
    ayahCount,
  }: { surah: number; ayah: number; ayahCount: number } = $props();

  const copy = getAudioCopy();
  const actionClass =
    "flex size-[30px] touch-manipulation items-center justify-center rounded-md text-foreground-secondary transition-colors hover:bg-surface-hover hover:text-foreground [@media(pointer:coarse)]:size-10";

  function playFromHere(): void {
    const recordingId = defaultRecordingId();
    if (recordingId === undefined || ayahCount < ayah) return;
    audio.playQueue(recordingId, [{ surah, from: ayah, to: ayahCount }]);
  }

  function repeatVerse(): void {
    audio.requestRepeat(surah, ayah, ayah);
  }
</script>

{#if defaultRecordingId() !== undefined}
  <button
    type="button"
    class={actionClass}
    title={copy.playFromHere}
    aria-label={copy.playFromHere}
    onclick={playFromHere}
  >
    <Icon name="play" size={15} />
  </button>
  <button
    type="button"
    class={actionClass}
    title={copy.repeatVerse}
    aria-label={copy.repeatVerse}
    onclick={repeatVerse}
  >
    <Icon name="arrows-clockwise" size={15} />
  </button>
{/if}
