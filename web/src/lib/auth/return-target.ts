import { browser } from "$app/environment";

const KEY = "eq:oauth-return";

function isInternalPath(p: string | null): p is string {
  if (p === null || p.length === 0) return false;
  if (!p.startsWith("/")) return false;
  if (p.startsWith("//")) return false;
  if (p.includes("\\")) return false;
  if (/[\s]/.test(p)) return false;
  return true;
}

export function getReturnTarget(): string | null {
  if (!browser) return null;
  let raw: string | null = null;
  try {
    raw = sessionStorage.getItem(KEY);
  } catch {
    return null;
  }
  if (!isInternalPath(raw)) {
    if (raw !== null) clearReturnTarget();
    return null;
  }
  return raw;
}

export function setReturnTarget(path: string): boolean {
  if (!browser) return false;
  if (!isInternalPath(path)) return false;
  try {
    sessionStorage.setItem(KEY, path);
    return true;
  } catch {
    return false;
  }
}

export function clearReturnTarget(): void {
  if (!browser) return;
  try {
    sessionStorage.removeItem(KEY);
  } catch {}
}

export function consumeReturnTarget(): string | null {
  const v = getReturnTarget();
  clearReturnTarget();
  return v;
}
