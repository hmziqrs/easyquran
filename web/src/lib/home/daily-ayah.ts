import { QURAN } from "#lib/config/site.js";
import { fetchJsonWithTimeout } from "#lib/quran/fetch.js";
import { unwrapEnvelope } from "#lib/quran/wire.js";
import { asNumber, asObject, asString } from "#lib/storage/index.js";

/**
 * Daily-ayah widget fetch (Feature 5.1 / D07): the backend's `/quran/random` is
 * deterministic per UTC date and serves Cache-Control to UTC midnight, so the
 * plain GET (no explicit date) is the canonical call — every visitor on a day
 * sees the same ayah and the browser caches it until the next UTC day.
 */

export interface DailyAyah {
  readonly surah: number;
  readonly ayah: number;
  readonly text: string;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unwrapped `/quran/random` JSON body; this decoder is its parse boundary.
function decodeDailyAyah(raw: unknown): DailyAyah | null {
  const body = asObject(unwrapEnvelope(raw));
  if (!body) return null;
  const ayah = asObject(body.ayah);
  const surah = ayah === undefined ? undefined : asNumber(ayah.surah, 1, 114);
  const num = ayah === undefined ? undefined : asNumber(ayah.ayah, 1, 286);
  const text = ayah === undefined ? undefined : asString(ayah.text);
  if (surah === undefined || num === undefined || text === undefined || text.length === 0)
    return null;
  return { surah, ayah: num, text };
}

export async function fetchDailyAyah(signal?: AbortSignal): Promise<DailyAyah | null> {
  if (!QURAN.apiBase) return null;
  try {
    const body = await fetchJsonWithTimeout(`${QURAN.apiBase}/random`, {
      headers: { accept: "application/json" },
      signal,
    });
    return decodeDailyAyah(body);
  } catch {
    return null;
  }
}
