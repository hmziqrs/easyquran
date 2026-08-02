import { asset } from "$app/paths";
import { createQuranData, type QuranData } from "./quran-data";

const QURAN_DATA_URL = "/quran-meta/quran-data.json" as const;

let dataPromise: Promise<QuranData> | undefined;
let loadedData: QuranData | undefined;

export function loadQuranData(): Promise<QuranData> {
  dataPromise ??= fetch(asset(QURAN_DATA_URL), {
    headers: { accept: "application/json" },
  }).then(async (response) => {
    if (!response.ok) throw new Error(`[quran-data] ${QURAN_DATA_URL} returned ${response.status}`);
    loadedData = createQuranData(await response.json());
    return loadedData;
  });
  return dataPromise;
}

export function peekQuranData(): QuranData | undefined {
  return loadedData;
}

export function resetQuranDataForTests(): void {
  dataPromise = undefined;
  loadedData = undefined;
}
