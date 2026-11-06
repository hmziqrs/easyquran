<script lang="ts">
  import { audio } from "#lib/audio/engine.svelte.js";
  import { REPEAT_LIMITS } from "#lib/audio/policy.js";
  import {
    MAX_REPEAT_PRESETS,
    addPreset,
    loadRepeatPresets,
    normalizePlan,
    removePreset,
    saveRepeatPresets,
    type RepeatPreset,
  } from "#lib/audio/repeat.js";
  import { getAudioCopy } from "#lib/i18n/audio-copy.js";
  import { newEntityId } from "#lib/sync/ids.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import { Label } from "#lib/components/ui/label/index.js";

  let {
    surah,
    ayahCount,
    recordingId,
  }: { surah: number; ayahCount: number; recordingId: string } = $props();

  const copy = getAudioCopy();
  let from = $state(1);
  let to = $state(3);
  let ayahRepeat = $state(1);
  let setRepeat = $state(1);
  let delayMs = $state(500);
  let presetName = $state("");
  let presets = $state<RepeatPreset[]>(loadRepeatPresets());
  let message = $state<string | null>(null);

  const plan = $derived(
    normalizePlan({ surah, from, to, ayahRepeat, setRepeat, delayMs }, ayahCount),
  );
  const surahPresets = $derived(presets.filter((preset) => preset.surah === surah));
  const running = $derived(audio.repeatStep !== null);
  const progress = $derived(
    audio.repeatStep === null
      ? null
      : copy.repeatProgress(audio.repeatStep.ayah, audio.repeatStep.pass, audio.repeatStep.passes),
  );

  // A reader's "Repeat this verse" fills the range here. Runs on mount too, so a
  // request made while the panel was closed still applies.
  $effect(() => {
    const request = audio.repeatRequest;
    if (request === null) return;
    from = request.from;
    to = request.to;
  });

  function start(): void {
    if (plan === null) {
      message = copy.repeatInvalid;
      return;
    }
    message = audio.startRepeat(recordingId, plan, ayahCount) ? null : copy.repeatInvalid;
  }

  function savePreset(): void {
    const name = presetName.trim();
    if (name.length === 0) return;
    if (presets.length >= MAX_REPEAT_PRESETS) {
      message = copy.presetFull;
      return;
    }
    const preset: RepeatPreset = {
      id: newEntityId(),
      name,
      surah,
      from,
      to,
      ayahRepeat,
      setRepeat,
      delayMs,
    };
    presets = addPreset(presets, preset);
    presetName = "";
    message = saveRepeatPresets(presets) ? null : copy.presetFailed;
  }

  function loadPreset(preset: RepeatPreset): void {
    from = preset.from;
    to = preset.to;
    ayahRepeat = preset.ayahRepeat;
    setRepeat = preset.setRepeat;
    delayMs = preset.delayMs;
  }

  function deletePreset(id: string): void {
    presets = removePreset(presets, id);
    message = saveRepeatPresets(presets) ? null : copy.presetFailed;
  }
</script>

<section class="audio-repeat" aria-labelledby="audio-repeat-title">
  <h3 id="audio-repeat-title" class="audio-repeat__title">{copy.repeatTitle}</h3>

  <div class="audio-repeat__grid">
    <div>
      <Label for="audio-repeat-from">{copy.repeatFrom}</Label>
      <Input
        id="audio-repeat-from"
        type="number"
        min={1}
        max={ayahCount}
        step={1}
        bind:value={from}
      />
    </div>
    <div>
      <Label for="audio-repeat-to">{copy.repeatTo}</Label>
      <Input id="audio-repeat-to" type="number" min={1} max={ayahCount} step={1} bind:value={to} />
    </div>
    <div>
      <Label for="audio-repeat-ayah">{copy.repeatAyahCount}</Label>
      <Input
        id="audio-repeat-ayah"
        type="number"
        min={REPEAT_LIMITS.ayahRepeat.min}
        max={REPEAT_LIMITS.ayahRepeat.max}
        step={1}
        bind:value={ayahRepeat}
      />
    </div>
    <div>
      <Label for="audio-repeat-set">{copy.repeatSetCount}</Label>
      <Input
        id="audio-repeat-set"
        type="number"
        min={REPEAT_LIMITS.setRepeat.min}
        max={REPEAT_LIMITS.setRepeat.max}
        step={1}
        bind:value={setRepeat}
      />
    </div>
    <div>
      <Label for="audio-repeat-delay">{copy.repeatDelay}</Label>
      <Input
        id="audio-repeat-delay"
        type="number"
        min={REPEAT_LIMITS.delayMs.min}
        max={REPEAT_LIMITS.delayMs.max}
        step={100}
        bind:value={delayMs}
      />
    </div>
  </div>

  <div class="audio-repeat__actions">
    {#if running}
      <Button variant="secondary" onclick={() => audio.stopRepeat()}>{copy.repeatStop}</Button>
    {:else}
      <Button onclick={start} disabled={plan === null}>{copy.repeatStart}</Button>
    {/if}
  </div>

  {#if progress}
    <p class="audio-repeat__progress" aria-live="polite">{progress}</p>
  {/if}
  {#if message}
    <p class="audio-repeat__message" role="status">{message}</p>
  {/if}
  <p class="audio-repeat__note">{copy.repeatAwake}</p>

  <div class="audio-repeat__presets">
    <h4>{copy.presets}</h4>
    {#if surahPresets.length > 0}
      <ul>
        {#each surahPresets as preset (preset.id)}
          <li>
            <span>{preset.name}</span>
            <Button variant="ghost" size="sm" onclick={() => loadPreset(preset)}
              >{copy.presetLoad}</Button
            >
            <Button variant="ghost" size="sm" onclick={() => deletePreset(preset.id)}
              >{copy.presetDelete}</Button
            >
          </li>
        {/each}
      </ul>
    {/if}
    <form
      class="audio-repeat__save"
      onsubmit={(event) => {
        event.preventDefault();
        savePreset();
      }}
    >
      <Label for="audio-preset-name" class="sr-only">{copy.presetName}</Label>
      <Input id="audio-preset-name" bind:value={presetName} maxlength={60} placeholder={copy.presetName} />
      <Button type="submit" variant="secondary" disabled={presetName.trim().length === 0}
        >{copy.presetSave}</Button
      >
    </form>
  </div>
</section>

<style>
  .audio-repeat__grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(8rem, 1fr));
    gap: 0.75rem;
  }
  .audio-repeat__actions,
  .audio-repeat__save,
  .audio-repeat__presets li {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
  }
  .audio-repeat__presets ul {
    list-style: none;
    padding: 0;
    margin: 0.5rem 0;
  }
  .audio-repeat__note,
  .audio-repeat__progress,
  .audio-repeat__message {
    font-size: 0.875rem;
    margin: 0.5rem 0;
  }
</style>
