const RESOLVE_BASE = "http://easyquran.local";

export function normalizeDataKey(url: string | URL): string {
  const u = new URL(url, RESOLVE_BASE);
  const params = new URLSearchParams();
  for (const [key, value] of u.searchParams) {
    if (key.startsWith("x-sveltekit-")) continue;
    if (key === "mode") continue;
    params.append(key, value);
  }
  const search = params.size > 0 ? `?${params.toString()}` : "";
  return `${u.pathname}${search}`;
}
