import {
  downloads_app_active,
  downloads_app_heading,
  downloads_app_inactive,
  downloads_app_offline,
  downloads_intro,
  downloads_offline,
  downloads_online,
  downloads_overall,
  downloads_overall_label,
  downloads_pack_heading,
  downloads_popup_hidden,
  downloads_popup_show,
  downloads_progress,
  downloads_quran_error,
  downloads_quran_heading,
  downloads_quran_idle,
  downloads_quran_note,
  downloads_state_checking,
  downloads_state_downloading,
  downloads_state_missing,
  downloads_state_queued,
  downloads_state_ready,
  downloads_storage_unknown,
  downloads_title,
  downloads_translations_empty,
  downloads_translations_heading,
} from "#lib/i18n/m/downloads.js";
import type { UiDirection, UiLocale } from "#lib/i18n/locales.js";
import { uiDirection } from "#lib/i18n/locales.js";
import { getLocale } from "#lib/paraglide/runtime.js";

export type DownloadRowState = "queued" | "downloading" | "ready" | "missing" | "checking";

export interface DownloadsCopy {
  readonly locale: UiLocale;
  readonly direction: UiDirection;
  readonly title: string;
  readonly intro: string;
  readonly online: string;
  readonly offline: string;
  readonly quranHeading: string;
  readonly quranNote: string;
  readonly overall: (ready: number, total: number) => string;
  readonly overallLabel: string;
  readonly states: Readonly<Record<DownloadRowState, string>>;
  readonly progress: (pct: number, loaded: string, total: string) => string;
  readonly quranError: string;
  readonly quranIdle: string;
  readonly translationsHeading: string;
  readonly translationsEmpty: string;
  readonly packHeading: string;
  readonly appHeading: string;
  readonly appOffline: string;
  readonly appActive: string;
  readonly appInactive: string;
  readonly storageUnknown: string;
  readonly popupHidden: string;
  readonly popupShow: string;
}

/** Downloads page copy. Loaded only by the downloads route: the `downloads` namespace is a lazy chunk. */
// SAFETY: paraglide is compiled for exactly the UI locales (en/ar in messages/), so getLocale() only ever returns a UiLocale at runtime.
export function getDownloadsCopy(locale: UiLocale = getLocale() as UiLocale): DownloadsCopy {
  const options = { locale } as const;
  const noArgs = (
    message: (inputs?: undefined, options?: { locale?: UiLocale }) => string,
  ): string => message(undefined, options);
  return {
    locale,
    direction: uiDirection(locale),
    title: noArgs(downloads_title),
    intro: noArgs(downloads_intro),
    online: noArgs(downloads_online),
    offline: noArgs(downloads_offline),
    quranHeading: noArgs(downloads_quran_heading),
    quranNote: noArgs(downloads_quran_note),
    overall: (ready, total) => downloads_overall({ ready, total }, options),
    overallLabel: noArgs(downloads_overall_label),
    states: {
      queued: noArgs(downloads_state_queued),
      downloading: noArgs(downloads_state_downloading),
      ready: noArgs(downloads_state_ready),
      missing: noArgs(downloads_state_missing),
      checking: noArgs(downloads_state_checking),
    },
    progress: (pct, loaded, total) => downloads_progress({ pct, loaded, total }, options),
    quranError: noArgs(downloads_quran_error),
    quranIdle: noArgs(downloads_quran_idle),
    translationsHeading: noArgs(downloads_translations_heading),
    translationsEmpty: noArgs(downloads_translations_empty),
    packHeading: noArgs(downloads_pack_heading),
    appHeading: noArgs(downloads_app_heading),
    appOffline: noArgs(downloads_app_offline),
    appActive: noArgs(downloads_app_active),
    appInactive: noArgs(downloads_app_inactive),
    storageUnknown: noArgs(downloads_storage_unknown),
    popupHidden: noArgs(downloads_popup_hidden),
    popupShow: noArgs(downloads_popup_show),
  };
}
