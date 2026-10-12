import { browser } from "$app/env";

import { TRANSLATION_BY_ID, TRANSLATIONS } from "#lib/data/translations.js";
import { asObject, asString, readJSON, writeJSON } from "#lib/storage/index.js";

/**
 * First-run reading choice (Feature 5.5 / HOME-04): one-line "Read in: Arabic
 * only · Arabic + {language}" persisted under `easyquran.home.reading` (local
 * only — the widget layout key's sibling, §11.3). The choice is a home-surface
 * preference: it labels the Continue card and seeds reader hrefs from /yours;
 * the reader's own translation state stays with the picker.
 */

const STORAGE_KEY = "easyquran.home.reading";
const SCHEMA_VERSION = 1;

export type ReadingChoice = { mode: "arabic" } | { mode: "translation"; sourceId: string };

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON → JSON.parse); this decoder re-validates every field, and a translation id that has since left the catalogue falls back to "no choice" (EC-38 spirit).
function decodeChoice(raw: unknown): ReadingChoice | null {
  const stored = asObject(raw);
  if (!stored || stored.v !== SCHEMA_VERSION) return null;
  const mode = asString(stored.mode);
  if (mode === "arabic") return { mode: "arabic" };
  if (mode !== "translation") return null;
  const sourceId = asString(stored.sourceId);
  if (sourceId === undefined || !TRANSLATION_BY_ID.has(sourceId)) return null;
  return { mode: "translation", sourceId };
}

/** First catalogue translation for a language code — baked order is deterministic. */
export function translationForLanguage(code: string): string | null {
  const hit = TRANSLATIONS.find((entry) => entry.languageCode === code);
  return hit === undefined ? null : hit.id;
}

/** Browser-language suggestion (HOME-04 default); Arabic-only when nothing matches. */
export function suggestedChoice(): ReadingChoice {
  if (!browser) return { mode: "arabic" };
  for (const tag of navigator.languages) {
    const primary = tag.split("-")[0]?.toLowerCase();
    if (primary === undefined) continue;
    if (primary === "ar") return { mode: "arabic" };
    const sourceId = translationForLanguage(primary);
    if (sourceId !== null) return { mode: "translation", sourceId };
  }
  return { mode: "arabic" };
}

/** Language label for the option pill: the catalogue's language name. */
export function languageLabelFor(sourceId: string): string | null {
  return TRANSLATION_BY_ID.get(sourceId)?.language ?? null;
}

class ReadingChoiceStore {
  #chosen = $state.raw<ReadingChoice | null>(null);

  hydrate(): void {
    if (!browser || this.#chosen !== null) return;
    this.#chosen = decodeChoice(readJSON(STORAGE_KEY));
  }

  get chosen(): ReadingChoice | null {
    return this.#chosen;
  }

  choose(choice: ReadingChoice): void {
    this.#chosen = choice;
    if (browser) writeJSON(STORAGE_KEY, { v: SCHEMA_VERSION, ...choice });
  }
}

export const readingChoice = new ReadingChoiceStore();
