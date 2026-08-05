export const QuranDataEnvironment = {
  Local: "local",
  Production: "prod",
} as const;

export type QuranDataEnvironment = (typeof QuranDataEnvironment)[keyof typeof QuranDataEnvironment];

export const LOCAL_QURAN_ARTIFACT_BASE = "/_quran";
export const QURAN_ARTIFACT_DELIVERY_BASE = "/_quran";
export const QURAN_R2_UPSTREAM_BASE = "https://r2.easyquran.fyi";

export function resolveQuranDataEnvironment(
  value: string | undefined,
  fallback: QuranDataEnvironment,
): QuranDataEnvironment {
  const environment = value?.trim() || fallback;
  if (
    environment === QuranDataEnvironment.Local ||
    environment === QuranDataEnvironment.Production
  ) {
    return environment;
  }
  throw new Error(
    `[quran] PUBLIC_ENV must be "${QuranDataEnvironment.Local}" or "${QuranDataEnvironment.Production}", received "${environment}"`,
  );
}

export function resolveQuranArtifactBase(environment: QuranDataEnvironment): string {
  return environment === QuranDataEnvironment.Local
    ? LOCAL_QURAN_ARTIFACT_BASE
    : QURAN_ARTIFACT_DELIVERY_BASE;
}
