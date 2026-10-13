import { uniq } from "es-toolkit";

import { REGION_LANGUAGES } from "./region-languages";
import { COUNTRY_TIMEZONES } from "./region-timezones";

/**
 * Translation-rail ranking from signals the browser already has: the reader's language
 * preferences and their timezone. No geolocation prompt, no network call — nothing leaves the
 * device, and a wrong guess only reorders the rail.
 */

let zoneCountry: Map<string, string> | null = null;

/** IANA timezone → ISO country, or null when the zone is not in a country the rail ranks for. */
export function countryFromTimeZone(timeZone: string | undefined): string | null {
  if (timeZone === undefined || timeZone === "") return null;
  if (zoneCountry === null) {
    zoneCountry = new Map();
    for (const [country, zones] of Object.entries(COUNTRY_TIMEZONES)) {
      for (const zone of zones.split(" ")) zoneCountry.set(zone, country);
    }
  }
  return zoneCountry.get(timeZone) ?? null;
}

function regionOf(tag: string): string | null {
  try {
    const region = new Intl.Locale(tag).region;
    // Numeric UN M.49 regions ("es-419" = Latin America) name no single country.
    if (region !== undefined && /^[A-Z]{2}$/.test(region)) return region;
    return null;
  } catch {
    // Malformed tags carry no region.
    return null;
  }
}

/** Country regions of BCP 47 tags in preference order ("ur-PK" → "PK"), deduped. */
export function regionsFromLanguageTags(tags: readonly string[]): string[] {
  return uniq(
    tags.map((tag) => regionOf(tag)).filter((region): region is string => region !== null),
  );
}

/** Base language codes in preference order ("ur-PK" → "ur"), deduped. */
export function baseLanguageCodes(tags: readonly string[]): string[] {
  return uniq(
    tags.map((tag) => (tag.split("-")[0] ?? "").toLowerCase()).filter((code) => code !== ""),
  );
}

function currentTimeZone(): string | undefined {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone;
  } catch {
    return undefined;
  }
}

/**
 * Where the reader probably is, best guess first: the timezone's country, then the regions of
 * their language tags. Client-only (reads navigator).
 */
export function detectCountries(): string[] {
  const fromZone = countryFromTimeZone(currentTimeZone());
  return uniq([
    ...(fromZone === null ? [] : [fromZone]),
    ...regionsFromLanguageTags(navigator.languages),
  ]);
}

/**
 * Rail priority, as language codes: the reader's first browser language, then English, then
 * their other browser languages, then the languages of the countries they are probably in,
 * then Arabic. Everything else follows alphabetically.
 */
export function languagePriority(
  browserTags: readonly string[],
  countries: readonly string[],
): string[] {
  const browser = baseLanguageCodes(browserTags);
  return uniq([
    ...browser.slice(0, 1),
    "en",
    ...browser.slice(1),
    ...countries.flatMap((country) => REGION_LANGUAGES[country] ?? []),
    "ar",
  ]);
}
