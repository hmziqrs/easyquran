<script lang="ts">
  // Reader header action: play the whole surah. Loaded only through a dynamic
  // import from ReaderHeader, and only once a recording is licensed.
  import { audio } from "#lib/audio/engine.svelte.js";
  import { defaultRecordingId } from "#lib/audio/catalogue.js";
  import { getAudioCopy } from "#lib/i18n/audio-copy.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Icon } from "#lib/components/icon/index.js";

  let { surah, ayahCount }: { surah: number; ayahCount: number } = $props();

  const copy = getAudioCopy();

  function playSurah(): void {
    const recordingId = defaultRecordingId();
    if (recordingId === undefined || ayahCount < 1) return;
    audio.playQueue(recordingId, [{ surah, from: 1, to: ayahCount }]);
  }
</script>

{#if defaultRecordingId() !== undefined}
  <Button variant="ghost" size="sm" onclick={playSurah} aria-label={copy.playSurah} title={copy.playSurah}>
    <Icon name="play" size={15} />
    <span>{copy.playSurah}</span>
  </Button>
{/if}
