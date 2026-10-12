import { browser } from "$app/env";

const KEY = "eq:oauth-return";

export function isInternalPath(p: string | null): p is string {
  if (p === null || p.length === 0) return false;
  if (!p.startsWith("/")) return false;
  if (p.startsWith("//")) return false;
  if (p.includes("\\")) return false;
  if (/[\s]/.test(p)) return false;
  return true;
}

// Sign-in surfaces are never valid return targets: landing on them while
// already signed in would loop the user back through the guards.
const AUTH_SURFACE_PATHS = new Set(["/login", "/register", "/forgot-password", "/verify-email"]);

function isAuthSurfacePath(path: string): boolean {
  return AUTH_SURFACE_PATHS.has(path) || path.startsWith("/auth/");
}

function asReturnPath(pathname: string, search: string): string | null {
  if (!isInternalPath(`${pathname}${search}`) || isAuthSurfacePath(pathname)) return null;
  return `${pathname}${search}`;
}

/**
 * The current location as a return target, or null when we are on an auth
 * surface (so a target already stored by /login's ?returnTo is left intact).
 */
export function currentAppReturnPath(): string | null {
  if (!browser) return null;
  return asReturnPath(location.pathname, location.search);
}

/**
 * Same-origin referrer as a return target for guest pages; null when the
 * referrer is missing, cross-origin, or an auth surface.
 */
export function appPathFromReferrer(referrer: string | null): string | null {
  if (!browser || !referrer) return null;
  let ref: URL;
  try {
    ref = new URL(referrer);
  } catch {
    return null;
  }
  if (ref.origin !== location.origin) return null;
  return asReturnPath(ref.pathname, ref.search);
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
