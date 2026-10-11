/**
 * quran.com reference mirror catalogue — script-level assets only.
 *
 * The mirror is an audit reference: parity checks compare our data and rendering against
 * quran.com's exact upstream assets. Nothing here is app-referenced; every entry keeps
 * productionEnabled = false until a licence clears and an env-gated consumer exists.
 *
 * Identity is id + baked byte size, never a hash (AGENTS.MD hard rule). QCF page fonts and
 * the code_v1/code_v2 page-glyph scripts are deliberately out of scope.
 */

import path from "node:path";
import { fileURLToPath } from "node:url";

export type QurancomAssetKind = "script" | "font";
export type QurancomLicenseStatus = "cleared" | "restricted" | "unknown";

export interface QurancomLicense {
  readonly name: string;
  readonly status: QurancomLicenseStatus;
  readonly notice?: string;
}

export interface QurancomAsset {
  readonly id: string;
  readonly kind: QurancomAssetKind;
  readonly url: string;
  readonly targetPath: string;
  readonly sizeBytes: number;
  readonly license: QurancomLicense;
  readonly provenance: string;
  readonly productionEnabled: boolean;
}

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

/** Committed mirror root: web/static/qurancom/ (resolved from this file, like upload.ts). */
export const QURANCOM_MIRROR_ROOT = path.join(REPO_ROOT, "web", "static", "qurancom");

/** Every mirror asset path relative to web/static/. */
export const QURANCOM_STATIC_PREFIX = "qurancom/";

const API_VERSES = "https://api.quran.com/api/v4/quran/verses";
const FONT_CDN = "https://quran.com/fonts";
const PROBED = "2026-10-11";

/** Verbatim QuranWBW distribution notice; do not paraphrase. */
export const QURANWBW_NOTICE =
  "NOT FOR SALE, NOT FOR MODIFICATION, NOT FOR DISTRIBUTION OR NOT FOR DEVELOPMENT WITHOUT WRITTEN NOTICE BY QURANWBW.COM";

/** Baked byte sizes recorded by `bun run quran:mirror --record` (2026-10-10). Identity, not hash. */
const RECORDED_SIZES: Readonly<Record<string, number>> = {
  "script-uthmani": 1654246,
  "script-indopak": 1721053,
  "script-indopak-nastaleeq": 1774400,
  "script-imlaei": 1602614,
  "script-imlaei-simple": 1079848,
  "script-uthmani-tajweed": 4061465,
  "font-uthmanic-hafs-woff2": 87760,
  "font-uthmanic-hafs-ttf": 242368,
  "font-indopak-nastaleeq-waqf-lazim-v4.2.1-woff2": 83560,
  "font-indopak-nastaleeq-waqf-lazim-v4.2.1-woff": 113472,
  "font-indopak-nastaleeq-waqf-lazim-v4.2.1-ttf": 335984,
  "font-sura-names-v1-woff2": 88560,
  "font-sura-names-v1-woff": 154980,
  "font-noto-naskh-arabic-regular-woff2": 49236,
  "font-noto-naskh-arabic-regular-ttf": 132340,
  "font-mehr-nastaliq-web-woff2": 41824,
  "font-mehr-nastaliq-web-woff": 52232,
  "font-mehr-nastaliq-web-ttf": 104752,
  "font-noto-serif-bengali-regular-woff2": 57656,
  "font-droid-naskh-regular-woff2": 38644,
  "font-droid-naskh-regular-ttf": 91340,
  "font-figtree-ttf": 62868,
  "font-playfair-display-black-ttf": 193608,
  "font-dhivehi-akuru-woff2": 16040,
};

function recordedSize(id: string): number {
  return RECORDED_SIZES[id] ?? 0;
}

const API_TEXT_LICENSE: QurancomLicense = {
  name: "quran.com API v4 script text",
  status: "unknown",
  notice: "No explicit per-asset license published; verify before production use.",
};

const KFGQPC_LICENSE: QurancomLicense = {
  name: "KFGQPC Uthmanic Script HAFS (UthmanicHafs1Ver18)",
  status: "restricted",
  notice:
    "KFGQPC asset; repo policy treats KFGQPC as restricted (docs/indopak-v4-verification-plan.md R12). Written permission needed before production use.",
};

const QURANWBW_LICENSE: QurancomLicense = {
  name: "AlQuran IndoPak by QuranWBW 4.2.1-WL",
  status: "restricted",
  notice: QURANWBW_NOTICE,
};

const OFL_LICENSE: QurancomLicense = {
  name: "SIL Open Font License 1.1",
  status: "cleared",
  notice: "Upstream font is licensed under SIL OFL 1.1.",
};

const APACHE_LICENSE: QurancomLicense = {
  name: "Apache License 2.0",
  status: "cleared",
  notice: "Google Droid fonts ship under Apache License 2.0.",
};

function apiScriptAsset(slug: string, fileName: string): QurancomAsset {
  const url = `${API_VERSES}/${slug}`;
  const id = `script-${slug.replace(/_/g, "-")}`;
  return {
    id,
    kind: "script",
    url,
    targetPath: `scripts/${fileName}`,
    sizeBytes: recordedSize(id),
    license: API_TEXT_LICENSE,
    provenance: `GET ${url} raw JSON body, mirrored as-is; live probe ${PROBED}.`,
    productionEnabled: false,
  };
}

function fontAsset(
  id: string,
  subpath: string,
  license: QurancomLicense,
  provenance: string = `quran.com font CDN; declared in quran.com fonts.scss; live probe ${PROBED}.`,
): QurancomAsset {
  return {
    id,
    kind: "font",
    url: `${FONT_CDN}/${subpath}`,
    targetPath: `fonts/${subpath}`,
    sizeBytes: recordedSize(id),
    license,
    provenance,
    productionEnabled: false,
  };
}

export const QURANCOM_ASSETS = [
  // Script texts: raw API v4 bodies, stored as-is.
  apiScriptAsset("uthmani", "uthmani.json"),
  apiScriptAsset("indopak", "indopak.json"),
  apiScriptAsset("indopak_nastaleeq", "indopak_nastaleeq.json"),
  apiScriptAsset("imlaei", "imlaei.json"),
  apiScriptAsset("imlaei_simple", "imlaei_simple.json"),
  apiScriptAsset("uthmani_tajweed", "uthmani_tajweed.json"),

  // Uthmani Hafs (KFGQPC).
  fontAsset(
    "font-uthmanic-hafs-woff2",
    "quran/hafs/uthmanic_hafs/UthmanicHafs1Ver18.woff2",
    KFGQPC_LICENSE,
  ),
  fontAsset(
    "font-uthmanic-hafs-ttf",
    "quran/hafs/uthmanic_hafs/UthmanicHafs1Ver18.ttf",
    KFGQPC_LICENSE,
  ),

  // IndoPak Nastaleeq (QuranWBW) — restricted, see docs/qurancom-mirror.md.
  fontAsset(
    "font-indopak-nastaleeq-waqf-lazim-v4.2.1-woff2",
    "quran/hafs/nastaleeq/indopak/indopak-nastaleeq-waqf-lazim-v4.2.1.woff2",
    QURANWBW_LICENSE,
  ),
  fontAsset(
    "font-indopak-nastaleeq-waqf-lazim-v4.2.1-woff",
    "quran/hafs/nastaleeq/indopak/indopak-nastaleeq-waqf-lazim-v4.2.1.woff",
    QURANWBW_LICENSE,
  ),
  fontAsset(
    "font-indopak-nastaleeq-waqf-lazim-v4.2.1-ttf",
    "quran/hafs/nastaleeq/indopak/indopak-nastaleeq-waqf-lazim-v4.2.1.ttf",
    QURANWBW_LICENSE,
  ),

  // Surah-name font.
  fontAsset("font-sura-names-v1-woff2", "quran/surah-names/v1/sura_names.woff2", {
    name: "quran.com surah-name font v1",
    status: "unknown",
    notice: "quran.com-authored asset with no published terms; verify before production use.",
  }),
  fontAsset("font-sura-names-v1-woff", "quran/surah-names/v1/sura_names.woff", {
    name: "quran.com surah-name font v1",
    status: "unknown",
    notice: "quran.com-authored asset with no published terms; verify before production use.",
  }),

  // Language fonts.
  fontAsset(
    "font-noto-naskh-arabic-regular-woff2",
    "lang/arabic/NotoNaskhArabic-Regular.woff2",
    OFL_LICENSE,
  ),
  fontAsset(
    "font-noto-naskh-arabic-regular-ttf",
    "lang/arabic/NotoNaskhArabic-Regular.ttf",
    OFL_LICENSE,
  ),
  fontAsset("font-mehr-nastaliq-web-woff2", "lang/urdu/MehrNastaliqWeb.woff2", {
    name: "Mehr Nastaliq Web",
    status: "unknown",
    notice: "No published redistribution terms found; verify before production use.",
  }),
  fontAsset("font-mehr-nastaliq-web-woff", "lang/urdu/MehrNastaliqWeb.woff", {
    name: "Mehr Nastaliq Web",
    status: "unknown",
    notice: "No published redistribution terms found; verify before production use.",
  }),
  fontAsset("font-mehr-nastaliq-web-ttf", "lang/urdu/MehrNastaliqWeb.ttf", {
    name: "Mehr Nastaliq Web",
    status: "unknown",
    notice: "No published redistribution terms found; verify before production use.",
  }),
  fontAsset(
    "font-noto-serif-bengali-regular-woff2",
    "lang/bengali/NotoSerifBengali-Regular.woff2",
    OFL_LICENSE,
  ),
  fontAsset(
    "font-droid-naskh-regular-woff2",
    "lang/kurdish/droid-naskh-regular.woff2",
    APACHE_LICENSE,
  ),
  fontAsset("font-droid-naskh-regular-ttf", "lang/kurdish/droid-naskh-regular.ttf", APACHE_LICENSE),
  fontAsset("font-figtree-ttf", "lang/Figtree/Figtree.ttf", OFL_LICENSE),
  fontAsset(
    "font-playfair-display-black-ttf",
    "lang/PlayfairDisplay/PlayfairDisplay-Black.ttf",
    OFL_LICENSE,
  ),

  // Dhivehi: fonts.scss path /fonts/lang/dhivehi/dhivehi/ 404s; live alternate mirrored.
  fontAsset(
    "font-dhivehi-akuru-woff2",
    "lang/dhivehi/DhivehiAkuru.woff2",
    {
      name: "Dhivehi Akuru",
      status: "unknown",
      notice:
        "fonts.scss path /fonts/lang/dhivehi/dhivehi/ returns 404; live alternate mirrored. No published terms; verify before production use.",
    },
    `quran.com font CDN; fonts.scss path /fonts/lang/dhivehi/dhivehi/DhivehiAkuru.{woff2,woff,otf} 404s, mirrored from live alternate; live probe ${PROBED}.`,
  ),
] satisfies readonly QurancomAsset[];

export function assertMirrorTargetPath(asset: Pick<QurancomAsset, "id" | "targetPath">): void {
  const target = asset.targetPath;
  const invalid =
    target.length === 0 ||
    path.posix.isAbsolute(target) ||
    target.includes("\\") ||
    path.posix.normalize(target) !== target;
  if (invalid) {
    throw new Error(`invalid mirror target path for ${asset.id}: ${JSON.stringify(target)}`);
  }
  if (target === ".." || target.startsWith("../")) {
    throw new Error(`mirror target path escapes web/static/qurancom for ${asset.id}: ${target}`);
  }
}

/** Mirror asset path relative to web/static/ (always under qurancom/). */
export function mirrorStaticPath(asset: Pick<QurancomAsset, "id" | "targetPath">): string {
  assertMirrorTargetPath(asset);
  return `${QURANCOM_STATIC_PREFIX}${asset.targetPath}`;
}

/** Absolute on-disk path, resolved from the repo root like upload.ts. */
export function mirrorAbsolutePath(asset: Pick<QurancomAsset, "id" | "targetPath">): string {
  assertMirrorTargetPath(asset);
  const absolute = path.resolve(QURANCOM_MIRROR_ROOT, asset.targetPath);
  if (!absolute.startsWith(`${QURANCOM_MIRROR_ROOT}${path.sep}`)) {
    throw new Error(
      `mirror target path escapes web/static/qurancom for ${asset.id}: ${asset.targetPath}`,
    );
  }
  return absolute;
}

/**
 * Size assertion for baked registry entries. sizeBytes = 0 means not yet recorded (skip).
 * Mismatch messages list id/expected/actual so the mirror CLI can fail non-zero verbatim.
 */
export function assertAssetSize(
  asset: Pick<QurancomAsset, "id" | "sizeBytes">,
  actualBytes: number,
): void {
  if (asset.sizeBytes === 0) return;
  if (asset.sizeBytes !== actualBytes) {
    throw new Error(
      `size mismatch for ${asset.id}: expected ${asset.sizeBytes}, actual ${actualBytes}`,
    );
  }
}
