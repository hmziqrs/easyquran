import { createReadStream, existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import type { Plugin } from "vite";

import rawTranslations from "./src/lib/data/translations.json";
import { QuranDataEnvironment, resolveQuranDataEnvironment } from "./src/lib/quran/environment";
import { registeredSourceProfiles } from "./src/lib/quran/view/source-profiles";

const WEB_ROOT = path.dirname(fileURLToPath(import.meta.url));
const LOCAL_ARTIFACT_PREFIX = "/_quran/";
const LOCAL_TRANSLATION_DIR = "db/quran/translations";
const TRANSLATION_FILE_PATH = 6;

// SAFETY: source profiles carry string artifact paths, and translations.json rows are positional
// arrays whose TRANSLATION_FILE_PATH slot holds the translation file name.
const LOCAL_ARTIFACT_ENTRIES: [string, string][] = [
  ...registeredSourceProfiles().map(
    (profile) =>
      [profile.artifact.r2Path, path.resolve(WEB_ROOT, "..", profile.artifact.repositoryPath)] as [
        string,
        string,
      ],
  ),
  ...(rawTranslations as readonly unknown[]).map((row) => {
    // SAFETY: translations.json rows are positional arrays; the file name sits at
    // TRANSLATION_FILE_PATH and is a string in every tracked row.
    const filePath = (row as readonly unknown[])[TRANSLATION_FILE_PATH] as string;
    // SAFETY: both entries are strings — the URL key template and path.resolve both yield strings.
    return [
      `translations/${filePath}`,
      path.resolve(WEB_ROOT, "..", LOCAL_TRANSLATION_DIR, filePath),
    ] as [string, string];
  }),
];

const LOCAL_ARTIFACTS = new Map<string, string>(LOCAL_ARTIFACT_ENTRIES);

function localArtifactPath(rawUrl: string): string | undefined {
  try {
    const pathname = new URL(rawUrl, "http://vite.local").pathname;
    if (!pathname.startsWith(LOCAL_ARTIFACT_PREFIX)) return undefined;
    return LOCAL_ARTIFACTS.get(decodeURIComponent(pathname.slice(LOCAL_ARTIFACT_PREFIX.length)));
  } catch {
    return undefined;
  }
}

export function quranArtifacts(): Plugin {
  let dataEnvironment: QuranDataEnvironment = QuranDataEnvironment.Production;
  return {
    name: "easyquran:quran-artifacts",
    enforce: "pre",
    configResolved(config) {
      dataEnvironment = resolveQuranDataEnvironment(
        process.env.PUBLIC_ENV ?? config.env.PUBLIC_ENV,
        config.command === "serve" ? QuranDataEnvironment.Local : QuranDataEnvironment.Production,
      );
    },
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        if (dataEnvironment !== QuranDataEnvironment.Local || !req.url) return next();
        const localPath = localArtifactPath(req.url);
        if (!localPath || !existsSync(localPath)) return next();

        const size = statSync(localPath).size;
        res.statusCode = 200;
        res.setHeader("Content-Type", "application/vnd.sqlite3");
        res.setHeader("Content-Length", size);
        res.setHeader("Cache-Control", "no-store");
        createReadStream(localPath).on("error", next).pipe(res);
      });
    },
    generateBundle() {
      if (dataEnvironment !== QuranDataEnvironment.Local) return;
      for (const [artifactPath, localPath] of LOCAL_ARTIFACTS) {
        if (!artifactPath.startsWith("arabic/")) continue;
        this.emitFile({
          type: "asset",
          fileName: `${LOCAL_ARTIFACT_PREFIX.slice(1)}${artifactPath}`,
          source: readFileSync(localPath),
        });
      }
    },
  };
}
