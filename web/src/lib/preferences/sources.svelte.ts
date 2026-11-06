import { homeLayout } from "#lib/home/layout.svelte.js";
import { applySyncedSettings, toSettingsDoc } from "#lib/settings/settings-document.js";
import type { JsonValue } from "#lib/storage/decoders.js";

import { preferences, type PreferenceSource } from "./store.svelte";

const SETTINGS_PREFIX = "settings.";
const LAYOUT_KEY = "home.layout";

// eslint-disable-next-line anti-slop/no-unknown-parameters -- values of a settings document; the JSON round-trip is what makes them storable.
function plain(value: unknown): JsonValue {
  return JSON.parse(JSON.stringify(value));
}

/**
 * One key per setting, so two settings changed apart both survive. Consent flags
 * stay on the device; they never sync.
 */
export const settingsSource: PreferenceSource = {
  read() {
    const doc = toSettingsDoc();
    const entries = [
      ...Object.entries(doc.appearance).map(([k, v]) => [`${SETTINGS_PREFIX}appearance.${k}`, plain(v)] as const),
      ...Object.entries(doc.reading).map(([k, v]) => [`${SETTINGS_PREFIX}reading.${k}`, plain(v)] as const),
    ];
    return new Map(entries);
  },
  apply(values) {
    const relevant = [...values.keys()].some((key) => key.startsWith(SETTINGS_PREFIX));
    if (!relevant) return;
    const doc = JSON.parse(JSON.stringify(toSettingsDoc()));
    for (const [key, value] of values) {
      if (!key.startsWith(SETTINGS_PREFIX)) continue;
      const [section, field] = key.slice(SETTINGS_PREFIX.length).split(".");
      if (section !== "appearance" && section !== "reading") continue;
      if (field !== undefined) doc[section][field] = value;
    }
    applySyncedSettings(doc);
  },
};

export const layoutSource: PreferenceSource = {
  read() {
    return new Map([[LAYOUT_KEY, plain({ widgets: homeLayout.entries })]]);
  },
  apply(values) {
    const value = values.get(LAYOUT_KEY);
    if (value !== undefined) homeLayout.replace(value);
  },
};

/** Boot once: wire the sources and re-capture whenever their values change. */
export function startPreferenceSync(): () => void {
  preferences.addSource(settingsSource);
  preferences.addSource(layoutSource);
  preferences.hydrate();
  return $effect.root(() => {
    $effect(() => {
      settingsSource.read();
      layoutSource.read();
      preferences.noteChanged();
    });
  });
}
