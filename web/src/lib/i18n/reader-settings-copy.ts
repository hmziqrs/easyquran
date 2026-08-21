import type { OfflinePackCopy } from "$lib/components/status/offline-pack-copy";
import type { UiLocale } from "$lib/i18n/locales";
import {
  tweaks_accent_option,
  tweaks_colour_input,
  tweaks_colour_label,
  tweaks_preset,
  tweaks_reset_to_preset,
  tweaks_toggle_status,
} from "$lib/i18n/m/controls";
import { reader_copied, reader_dark, reader_light, reader_theme } from "$lib/i18n/m/reader";
import {
  reader_accent,
  reader_accent_azure,
  reader_accent_colour,
  reader_accent_gold,
  reader_accent_plum,
  reader_accent_reset,
  reader_accent_teal,
  reader_analytics,
  reader_background,
  reader_clear,
  reader_close_appearance,
  reader_copy_css,
  reader_custom_colours,
  reader_data_privacy,
  reader_mode,
  reader_off,
  reader_on,
  reader_performance,
  reader_performance_reload,
  reader_pop,
  reader_reset,
  reader_settings,
  reader_surface,
  reader_surface_contrast,
  reader_surface_contrast_note,
  reader_surface_ink,
  reader_surface_ink_note,
  reader_surface_mocha,
  reader_surface_mocha_note,
  reader_surface_paper,
  reader_surface_paper_note,
  reader_surface_slate,
  reader_surface_slate_note,
  reader_theme_derived_note,
} from "$lib/i18n/m/reader-settings";
import {
  settings_storage_pack_busy,
  settings_storage_pack_heading,
  settings_storage_pack_retry,
  settings_storage_pack_routes,
  settings_storage_pack_saved,
  settings_storage_pack_toggle_off,
  settings_storage_pack_toggle_on,
  settings_storage_pack_usage,
} from "$lib/i18n/m/settings";
import type { TweaksResolvedCopy } from "$lib/i18n/marketing-copy";

export type ReaderSettingsCopy = TweaksResolvedCopy & {
  readonly offlinePack: OfflinePackCopy;
  readonly background: string;
  readonly pop: string;
  readonly accentColour: (label: string) => string;
  readonly accentReset: (label: string) => string;
  readonly themeDerivedNote: string;
};

/**
 * Reader appearance panel copy. Loaded on first panel open, never eagerly: it is ~34 messages that
 * nothing renders until the user opens the panel. See docs/quran-system.md (Part 2, Message chunking).
 */
export function getReaderSettingsCopy(locale: UiLocale): ReaderSettingsCopy {
  const options = { locale } as const;
  const noArgs = (
    message: (inputs?: undefined, options?: { locale?: UiLocale }) => string,
  ): string => message(undefined, options);
  return {
    settings: noArgs(reader_settings),
    theme: noArgs(reader_theme),
    closePanel: noArgs(reader_close_appearance),
    mode: noArgs(reader_mode),
    surface: noArgs(reader_surface),
    accent: noArgs(reader_accent),
    customColours: noArgs(reader_custom_colours),
    clear: noArgs(reader_clear),
    seedNames: {
      bg: noArgs(reader_background),
      accent: noArgs(reader_accent),
      pop: noArgs(reader_pop),
    },
    colourLabel: noArgs(tweaks_colour_label),
    accentOptionLabel: (name) => tweaks_accent_option({ name }, options),
    colourInputLabel: (name) => tweaks_colour_input({ name }, options),
    preset: noArgs(tweaks_preset),
    resetToPresetLabel: (name) => tweaks_reset_to_preset({ name }, options),
    toggleStatusLabel: (name, status) => tweaks_toggle_status({ name, status }, options),
    derivedColours: noArgs(reader_theme_derived_note),
    copied: noArgs(reader_copied),
    copyCss: noArgs(reader_copy_css),
    reset: noArgs(reader_reset),
    dataPrivacy: noArgs(reader_data_privacy),
    analytics: noArgs(reader_analytics),
    performance: noArgs(reader_performance),
    on: noArgs(reader_on),
    off: noArgs(reader_off),
    performanceReload: noArgs(reader_performance_reload),
    offlinePack: {
      heading: noArgs(settings_storage_pack_heading),
      routes: (entries: number, size: string) =>
        settings_storage_pack_routes({ entries, size }, options),
      saved: (when: string) => settings_storage_pack_saved({ when }, options),
      usage: (used: string, quota: string) => settings_storage_pack_usage({ used, quota }, options),
      toggleOn: noArgs(settings_storage_pack_toggle_on),
      toggleOff: noArgs(settings_storage_pack_toggle_off),
      busy: noArgs(settings_storage_pack_busy),
      retry: noArgs(settings_storage_pack_retry),
    },
    background: noArgs(reader_background),
    pop: noArgs(reader_pop),
    themeNames: {
      dark: noArgs(reader_dark),
      light: noArgs(reader_light),
    },
    surfaces: {
      ink: { label: noArgs(reader_surface_ink), note: noArgs(reader_surface_ink_note) },
      paper: { label: noArgs(reader_surface_paper), note: noArgs(reader_surface_paper_note) },
      slate: { label: noArgs(reader_surface_slate), note: noArgs(reader_surface_slate_note) },
      mocha: { label: noArgs(reader_surface_mocha), note: noArgs(reader_surface_mocha_note) },
      contrast: {
        label: noArgs(reader_surface_contrast),
        note: noArgs(reader_surface_contrast_note),
      },
    },
    accents: {
      emerald: noArgs(reader_accent_teal),
      gold: noArgs(reader_accent_gold),
      azure: noArgs(reader_accent_azure),
      plum: noArgs(reader_accent_plum),
    },
    accentColour: (label) => reader_accent_colour({ label }, options),
    accentReset: (label) => reader_accent_reset({ label }, options),
    themeDerivedNote: noArgs(reader_theme_derived_note),
  };
}
