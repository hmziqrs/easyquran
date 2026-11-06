import type { UiDirection, UiLocale } from "#lib/i18n/locales.js";
import { uiDirection } from "#lib/i18n/locales.js";
import {
  audio_attribution,
  audio_attribution_source,
  audio_blocked,
  audio_collapse,
  audio_dock_label,
  audio_error,
  audio_expand,
  audio_loading,
  audio_next,
  audio_next_surah,
  audio_now_playing,
  audio_offline_cleared,
  audio_offline_clear,
  audio_offline_failed,
  audio_offline_limit,
  audio_offline_save,
  audio_offline_saved,
  audio_open_ayah,
  audio_pause,
  audio_play,
  audio_preset_delete,
  audio_preset_failed,
  audio_preset_full,
  audio_preset_load,
  audio_preset_name,
  audio_preset_save,
  audio_presets,
  audio_previous,
  audio_previous_surah,
  audio_queue,
  audio_repeat_ayah_count,
  audio_repeat_awake,
  audio_repeat_delay,
  audio_repeat_from,
  audio_repeat_invalid,
  audio_repeat_progress,
  audio_repeat_set_count,
  audio_repeat_start,
  audio_repeat_stop,
  audio_repeat_title,
  audio_repeat_to,
  audio_retry,
  audio_retrying,
  audio_seek,
  audio_speed,
  audio_play_from_here,
  audio_play_surah,
  audio_repeat_verse,
} from "#lib/i18n/m/audio.js";
import { getLocale } from "#lib/paraglide/runtime.js";

export interface AudioCopy {
  readonly locale: UiLocale;
  readonly direction: UiDirection;
  readonly dockLabel: string;
  readonly play: string;
  readonly pause: string;
  readonly previous: string;
  readonly next: string;
  readonly previousSurah: string;
  readonly nextSurah: string;
  readonly expand: string;
  readonly collapse: string;
  readonly speed: string;
  readonly seek: string;
  readonly queue: string;
  nowPlaying(surah: number, ayah: number): string;
  openAyah(surah: number, ayah: number): string;
  readonly loading: string;
  retrying(attempt: number): string;
  readonly error: string;
  readonly retry: string;
  readonly blocked: string;
  attribution(reciter: string, licence: string): string;
  readonly attributionSource: string;
  readonly offlineSave: string;
  readonly offlineSaved: string;
  offlineLimit(megabytes: number): string;
  readonly offlineFailed: string;
  readonly offlineClear: string;
  readonly offlineCleared: string;
  readonly repeatTitle: string;
  readonly repeatFrom: string;
  readonly repeatTo: string;
  readonly repeatAyahCount: string;
  readonly repeatSetCount: string;
  readonly repeatDelay: string;
  readonly repeatStart: string;
  readonly repeatStop: string;
  repeatProgress(ayah: number, pass: number, passes: number): string;
  readonly repeatInvalid: string;
  readonly repeatAwake: string;
  readonly presetName: string;
  readonly presetSave: string;
  readonly presetLoad: string;
  readonly presetDelete: string;
  readonly presets: string;
  readonly presetFull: string;
  readonly presetFailed: string;
  readonly playFromHere: string;
  readonly playSurah: string;
  readonly repeatVerse: string;
}

// SAFETY: paraglide is compiled for exactly the UI locales (en/ar in messages/), so getLocale() only ever returns a UiLocale at runtime.
export function getAudioCopy(locale: UiLocale = getLocale() as UiLocale): AudioCopy {
  const options = { locale };
  return {
    locale,
    direction: uiDirection(locale),
    dockLabel: audio_dock_label(undefined, options),
    play: audio_play(undefined, options),
    pause: audio_pause(undefined, options),
    previous: audio_previous(undefined, options),
    next: audio_next(undefined, options),
    previousSurah: audio_previous_surah(undefined, options),
    nextSurah: audio_next_surah(undefined, options),
    expand: audio_expand(undefined, options),
    collapse: audio_collapse(undefined, options),
    speed: audio_speed(undefined, options),
    seek: audio_seek(undefined, options),
    queue: audio_queue(undefined, options),
    nowPlaying: (surah, ayah) => audio_now_playing({ surah, ayah }, options),
    openAyah: (surah, ayah) => audio_open_ayah({ surah, ayah }, options),
    loading: audio_loading(undefined, options),
    retrying: (attempt) => audio_retrying({ attempt }, options),
    error: audio_error(undefined, options),
    retry: audio_retry(undefined, options),
    blocked: audio_blocked(undefined, options),
    attribution: (reciter, licence) => audio_attribution({ reciter, licence }, options),
    attributionSource: audio_attribution_source(undefined, options),
    offlineSave: audio_offline_save(undefined, options),
    offlineSaved: audio_offline_saved(undefined, options),
    offlineLimit: (megabytes) => audio_offline_limit({ mb: megabytes }, options),
    offlineFailed: audio_offline_failed(undefined, options),
    offlineClear: audio_offline_clear(undefined, options),
    offlineCleared: audio_offline_cleared(undefined, options),
    repeatTitle: audio_repeat_title(undefined, options),
    repeatFrom: audio_repeat_from(undefined, options),
    repeatTo: audio_repeat_to(undefined, options),
    repeatAyahCount: audio_repeat_ayah_count(undefined, options),
    repeatSetCount: audio_repeat_set_count(undefined, options),
    repeatDelay: audio_repeat_delay(undefined, options),
    repeatStart: audio_repeat_start(undefined, options),
    repeatStop: audio_repeat_stop(undefined, options),
    repeatProgress: (ayah, pass, passes) => audio_repeat_progress({ ayah, pass, passes }, options),
    repeatInvalid: audio_repeat_invalid(undefined, options),
    repeatAwake: audio_repeat_awake(undefined, options),
    presetName: audio_preset_name(undefined, options),
    presetSave: audio_preset_save(undefined, options),
    presetLoad: audio_preset_load(undefined, options),
    presetDelete: audio_preset_delete(undefined, options),
    presets: audio_presets(undefined, options),
    presetFull: audio_preset_full(undefined, options),
    presetFailed: audio_preset_failed(undefined, options),
    playFromHere: audio_play_from_here(undefined, options),
    playSurah: audio_play_surah(undefined, options),
    repeatVerse: audio_repeat_verse(undefined, options),
  };
}
