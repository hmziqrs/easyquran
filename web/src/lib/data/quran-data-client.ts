import { asset } from "$app/paths";

import { createQuranData, type QuranData } from "./quran-data";

const QURAN_DATA_URL = "/quran-meta/quran-data.json" as const;

let dataPromise: Promise<QuranData> | undefined;
let loadedData: QuranData | undefined;

export function loadQuranData(): Promise<QuranData> {
  if (dataPromise) return dataPromise;
  const url = asset(QURAN_DATA_URL);
  const request = fetch(url, { headers: { accept: "application/json" } })
    .then(async (response) => {
      if (!response.ok) throw new Error(`[quran-data] ${url} returned ${response.status}`);
      loadedData = createQuranData(await response.json());
      return loadedData;
    })
    .catch((error: unknown) => {
      if (dataPromise === request) dataPromise = undefined;
      throw error;
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
