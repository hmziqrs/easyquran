import { publicHref } from "#lib/i18n/public-href.js";

import { createQuranData, type QuranData } from "./quran-data";

// quran-meta/quran-data.json is emitted by the quran vite plugin at build
// time, so it never appears in the static-dir scan behind kit 3's AssetPath
// union and cannot ride asset(). publicHref applies the same base-path
// prefix asset() would (paths.assets is unset in this app's kit config).
export const QURAN_DATA_URL = publicHref("/quran-meta/quran-data.json");

let dataPromise: Promise<QuranData> | undefined;
let loadedData: QuranData | undefined;

export function loadQuranData(): Promise<QuranData> {
  if (dataPromise) return dataPromise;
  const url = QURAN_DATA_URL;
  const request = fetch(url, { headers: { accept: "application/json" } })
    .then(async (response) => {
      if (!response.ok) throw new Error(`[quran-data] ${url} returned ${response.status}`);
      loadedData = createQuranData(await response.json());
      return loadedData;
    })
    .catch((cause: unknown) => {
      if (dataPromise === request) dataPromise = undefined;
      throw cause;
    });
  dataPromise = request;
  return request;
}

export function peekQuranData(): QuranData | undefined {
  return loadedData;
}

export function resetQuranDataForTests(): void {
  dataPromise = undefined;
  loadedData = undefined;
}
