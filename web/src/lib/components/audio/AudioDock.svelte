<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { audio } from "#lib/audio/engine.svelte.js";
  import { reciterById, type AudioRecording } from "#lib/audio/catalogue.js";
  import type { UiLocale } from "#lib/i18n/locales.js";
  import { removeCachedAudioFiles } from "#lib/audio/offline.js";
  import { OFFLINE_AUDIO_MAX_BYTES, PLAYBACK_RATES } from "#lib/audio/policy.js";
  import { peekQuranData } from "#lib/data/quran-data-client.js";
  import { routeContextFromParams, surahAyahPathFor } from "#lib/data/quran.js";
  import { getAudioCopy } from "#lib/i18n/audio-copy.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import RepeatPanel from "./RepeatPanel.svelte";

  // Mounted by the application layout only when a recording is licensed and only
  // in the browser. The engine is a module singleton, so this component can be
  // remounted on navigation without stopping playback.
  const copy = getAudioCopy();
  const WINDOW = 30;

  // Panel state lives in the engine so reader actions can open it.
  const expanded = $derived(audio.panelOpen);
  let notice = $state<string | null>(null);
  let busy = $state(false);

  const recording = $derived(audio.recording);
  const item = $derived(audio.item);
  const queue = $derived(audio.queue);
  const reciterName = $derived(
    recording ? (reciterById(recording.reciterId)?.name ?? "") : "",
  );
  const attributionText = $derived(
    recording ? attributionIn(copy.locale, recording) : "",
  );
  const playing = $derived(audio.status === "playing" || audio.status === "loading");
  const readerHref = $derived.by(() => {
    if (!item) return null;
    const entry = peekQuranData()?.surahByNum(item.surah);
    if (!entry) return null;
    return surahAyahPathFor(routeContextFromParams(page.params), entry, item.ayah);
  });
  // The repeat panel follows a range the reader requested, else the playing surah.
  const panelSurah = $derived(audio.repeatRequest?.surah ?? item?.surah ?? 0);
  const ayahCount = $derived(peekQuranData()?.surahByNum(panelSurah)?.ayahCount ?? 0);
  const windowItems = $derived.by(() => {
    if (!queue) return [];
    const start = Math.max(0, queue.index - WINDOW);
    const end = Math.min(queue.items.length, queue.index + WINDOW + 1);
    return queue.items.slice(start, end).map((entry, offset) => ({
      entry,
      index: start + offset,
    }));
  });

  function attributionIn(locale: UiLocale, source: AudioRecording): string {
    return locale === "ar" ? source.attribution.ar : source.attribution.en;
  }

  function megabytes(bytes: number): number {
    return Math.round(bytes / (1024 * 1024));
  }

  async function saveOffline(): Promise<void> {
    busy = true;
    const result = await audio.cacheQueue();
    busy = false;
    if (result === "ok") notice = copy.offlineSaved;
    else if (result === "limit") notice = copy.offlineLimit(megabytes(OFFLINE_AUDIO_MAX_BYTES));
    else notice = copy.offlineFailed;
  }

  async function clearOffline(): Promise<void> {
    await removeCachedAudioFiles();
    notice = copy.offlineCleared;
  }

  function statusText(): string | null {
    if (audio.status === "error") return copy.error;
    if (audio.status === "loading" && audio.attempt > 0) return copy.retrying(audio.attempt);
    if (audio.status === "loading") return copy.loading;
    return null;
  }

  onMount(() => {
    audio.hydrate();
  });

  // Published height so bottom-anchored overlays (download progress popup, toaster) stack above the dock.
  let dockHeight = $state(0);
  const docked = $derived(Boolean(queue && recording && item));
  $effect(() => {
    const root = document.documentElement;
    root.style.setProperty("--eq-dock-h", `${docked ? dockHeight : 0}px`);
    return () => root.style.removeProperty("--eq-dock-h");
  });

  $effect(() => {
    let disposed = false;
    const handles: { unregister(): void }[] = [];
    void import("#lib/hotkeys.svelte.js").then(({ registerHotkey }) => {
      if (disposed) return;
      handles.push(
        registerHotkey("Mod+Shift+Space", (event) => {
          if (event.isComposing) return;
          event.preventDefault();
          audio.toggle();
        }),
      );
      handles.push(
        registerHotkey("Mod+Shift+ArrowRight", (event) => {
          if (event.isComposing) return;
          event.preventDefault();
          audio.next();
        }),
      );
      handles.push(
        registerHotkey("Mod+Shift+ArrowLeft", (event) => {
          if (event.isComposing) return;
          event.preventDefault();
          audio.prev();
        }),
      );
    });
    return () => {
      disposed = true;
      for (const handle of handles) handle.unregister();
    };
  });
</script>

{#if queue && recording && item}
  <section class="audio-dock" aria-label={copy.dockLabel} dir={copy.direction} bind:offsetHeight={dockHeight}>
    <div class="audio-dock__bar">
      <Button
        variant="ghost"
        size="icon"
        onclick={() => audio.prevSurah()}
        aria-label={copy.previousSurah}
        title={copy.previousSurah}>&laquo;</Button
      >
      <Button variant="ghost" size="icon" onclick={() => audio.prev()} aria-label={copy.previous}
        >&lsaquo;</Button
      >
      <Button
        variant="primary"
        size="icon"
        onclick={() => audio.toggle()}
        aria-label={playing ? copy.pause : copy.play}>{playing ? "II" : ">"}</Button
      >
      <Button variant="ghost" size="icon" onclick={() => audio.next()} aria-label={copy.next}
        >&rsaquo;</Button
      >
      <Button
        variant="ghost"
        size="icon"
        onclick={() => audio.nextSurah()}
        aria-label={copy.nextSurah}
        title={copy.nextSurah}>&raquo;</Button
      >
      <p class="audio-dock__now" aria-live="polite">
        <span>{copy.nowPlaying(item.surah, item.ayah)}</span>
        {#if statusText()}
          <span class="audio-dock__status">{statusText()}</span>
        {/if}
      </p>
      <Button
        variant="ghost"
        size="sm"
        onclick={() => audio.setPanelOpen(!audio.panelOpen)}
        aria-expanded={expanded}
        aria-controls="audio-full-player">{expanded ? copy.collapse : copy.expand}</Button
      >
    </div>

    {#if audio.status === "error"}
      <div class="audio-dock__error" role="alert">
        <span>{copy.error}</span>
        <Button variant="secondary" size="sm" onclick={() => audio.retry()}>{copy.retry}</Button>
      </div>
    {/if}

    {#if expanded}
      <div id="audio-full-player" class="audio-dock__full">
        <header>
          <h2>{copy.nowPlaying(item.surah, item.ayah)}</h2>
          <p>{reciterName} · {recording.label}</p>
          <p class="audio-dock__attribution">
            {copy.attribution(reciterName, recording.licence.name)}
            {attributionText}
            <a href={recording.attribution.url} rel="noopener noreferrer" target="_blank"
              >{copy.attributionSource}</a
            >
          </p>
        </header>

        <label class="audio-dock__seek">
          <span class="sr-only">{copy.seek}</span>
          <input
            type="range"
            min="0"
            max={audio.durationSec || 0}
            step="0.1"
            value={audio.positionSec}
            oninput={(event) => audio.seek(Number(event.currentTarget.value))}
          />
        </label>

        <div class="audio-dock__speed" role="group" aria-label={copy.speed}>
          {#each PLAYBACK_RATES as rate (rate)}
            <Button
              variant={audio.rate === rate ? "secondary" : "ghost"}
              size="sm"
              aria-pressed={audio.rate === rate}
              onclick={() => audio.setRate(rate)}>{rate}x</Button
            >
          {/each}
        </div>

        {#if readerHref}
          <a class="audio-dock__reader" href={readerHref}>{copy.openAyah(item.surah, item.ayah)}</a>
        {/if}

        <section aria-label={copy.queue} class="audio-dock__queue">
          <h3>{copy.queue}</h3>
          <ol>
            {#each windowItems as row (row.index)}
              <li>
                <Button
                  variant={row.index === queue.index ? "secondary" : "ghost"}
                  size="sm"
                  aria-current={row.index === queue.index ? "true" : undefined}
                  onclick={() => audio.jumpTo(row.index)}>{row.entry.surah}:{row.entry.ayah}</Button
                >
              </li>
            {/each}
          </ol>
        </section>

        <div class="audio-dock__offline">
          <Button variant="secondary" size="sm" onclick={saveOffline} disabled={busy || audio.caching}
            >{copy.offlineSave}</Button
          >
          <Button variant="ghost" size="sm" onclick={clearOffline}>{copy.offlineClear}</Button>
          {#if notice}
            <p role="status">{notice}</p>
          {/if}
        </div>

        {#if ayahCount > 0}
          <RepeatPanel surah={panelSurah} {ayahCount} recordingId={recording.id} />
        {/if}
      </div>
    {/if}
  </section>
{/if}

<style>
  .audio-dock {
    position: fixed;
    inset-inline: 0;
    bottom: 0;
    z-index: 40;
    background: var(--background, #fff);
    border-block-start: 1px solid var(--border, #ddd);
    padding: 0.5rem 1rem;
  }
  .audio-dock__bar,
  .audio-dock__speed,
  .audio-dock__offline,
  .audio-dock__error {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
  }
  .audio-dock__now {
    flex: 1 1 10rem;
    margin: 0;
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .audio-dock__status {
    opacity: 0.8;
  }
  .audio-dock__full {
    max-height: 70vh;
    overflow-y: auto;
    padding-block: 0.75rem;
    display: grid;
    gap: 0.75rem;
  }
  .audio-dock__seek input {
    width: 100%;
  }
  .audio-dock__queue ol {
    list-style: none;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
  }
  .audio-dock__attribution {
    font-size: 0.8125rem;
  }
  @media (max-width: 480px) {
    .audio-dock {
      padding-inline: 16px;
    }
  }
</style>
