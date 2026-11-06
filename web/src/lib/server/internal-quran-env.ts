// Production refuses to boot without the internal Quran API pair. Without it,
// SSR would fall back to the public CF-fronted base, which lands in the
// escalatable external bucket: one IP could earn a ban that hits every reader.
// Pure over the env map so the boot check is unit-testable without binding a port.
export function assertInternalQuranEnv(env: Readonly<Record<string, string | undefined>>): void {
  if (env.NODE_ENV !== "production") return;
  const missing: string[] = [];
  if (!env.INTERNAL_QURAN_API_BASE?.trim()) missing.push("INTERNAL_QURAN_API_BASE");
  if (!env.INTERNAL_QURAN_API_TOKEN?.trim()) missing.push("INTERNAL_QURAN_API_TOKEN");
  if (missing.length > 0) {
    throw new Error(`[server] production refuses to boot without ${missing.join(", ")}`);
  }
}
