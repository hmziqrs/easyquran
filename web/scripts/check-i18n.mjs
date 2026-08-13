import { readFile } from "node:fs/promises";

const LOCALES = ["en", "ar"];
const DOMAINS = ["messages", "messages/reader", "messages/auth"];

const readCatalog = async (domain, locale) => {
  const path = new URL(`../${domain}/${locale}.json`, import.meta.url);
  return JSON.parse(await readFile(path, "utf8"));
};

const flatten = (value, prefix = "", out = new Map()) => {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- value is JSON.parse'd catalog content; this is the boundary discrimination
  if (value && typeof value === "object" && !Array.isArray(value)) {
    for (const [key, child] of Object.entries(value)) {
      flatten(child, prefix ? `${prefix}.${key}` : key, out);
    }
  } else {
    out.set(prefix, value);
  }
  return out;
};

const signature = (value, structural = false, complex = false) => {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- catalog leaf discrimination on untyped JSON.parse output
  if (typeof value === "string") {
    if (structural) return ["literal", value];
    if (complex) return ["text"];
    const parameters = [...value.matchAll(/\{\s*([A-Za-z][A-Za-z0-9_]*)\s*\}/gu)]
      .map((match) => match[1])
      .sort();
    return ["text", parameters];
  }
  if (Array.isArray(value)) {
    return ["array", value.map((item) => signature(item, structural, true))];
  }
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- nested catalog object discrimination on untyped JSON.parse output
  if (value && typeof value === "object") {
    return [
      "object",
      Object.entries(value)
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([key, child]) => [
          key,
          signature(child, key === "declarations" || key === "selectors", complex),
        ]),
    ];
  }
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- terminal primitive tag for a JSON.parse leaf; no schema layer in plain JS
  return [typeof value];
};

const fail = (message) => {
  throw new Error(`[i18n] ${message}`);
};

const catalogs = Object.fromEntries(
  await Promise.all(
    LOCALES.map(async (locale) => {
      const domains = new Map();
      for (const domain of DOMAINS) {
        domains.set(domain, flatten(await readCatalog(domain, locale)));
      }
      return [locale, domains];
    }),
  ),
);

const compareCatalog = (source, candidate, label) => {
  const missing = [...source.keys()].filter((key) => !candidate.has(key));
  const unknown = [...candidate.keys()].filter((key) => !source.has(key));
  if (missing.length) fail(`${label} missing: ${missing.join(", ")}`);
  if (unknown.length) fail(`${label} unknown: ${unknown.join(", ")}`);
  for (const [key, sourceValue] of source) {
    const targetValue = candidate.get(key);
    if (JSON.stringify(signature(sourceValue)) !== JSON.stringify(signature(targetValue))) {
      fail(`${label} shape/parameter mismatch: ${key}`);
    }
  }
};

for (const domain of DOMAINS) {
  const source = catalogs.en.get(domain);
  for (const locale of LOCALES.slice(1)) {
    compareCatalog(source, catalogs[locale].get(domain), `${domain}/${locale}`);
  }
}

const byLocale = Object.fromEntries(
  LOCALES.map((locale) => {
    const merged = new Map();
    for (const domain of DOMAINS) {
      for (const [key, value] of catalogs[locale].get(domain)) {
        if (merged.has(key)) fail(`duplicate ${locale} key across domains: ${key}`);
        // eslint-disable-next-line anti-slop/no-runtime-typeof -- message value is untyped JSON.parse output; string check is the emptiness parse
        if (typeof value === "string" && value.trim() === "") {
          fail(`empty ${locale} message: ${key}`);
        }
        merged.set(key, value);
      }
    }
    return [locale, merged];
  }),
);

const base = byLocale.en;
for (const locale of LOCALES.slice(1)) {
  compareCatalog(base, byLocale[locale], locale);
}

console.log(`[i18n] ${base.size} messages valid across ${LOCALES.join(", ")}`);
