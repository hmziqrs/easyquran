/**
 * Appeal intake pages (A35). `/appeal` takes a token from the notice email;
 * `/appeal/<token>` is the form. Both stay outside the app shell's third-party
 * surface: no analytics, and the response carries no-referrer and noindex.
 */

export const APPEAL_PATH = "/appeal";

/** A token is 32 random bytes in hex (see the API's appeal_v1 controller). */
export const APPEAL_TOKEN_PATTERN = /^[0-9a-f]{64}$/u;

/** Headers stamped on every appeal response (server-side, see hooks.server.ts). */
export const APPEAL_HEADERS = {
  "X-Robots-Tag": "noindex",
  "Referrer-Policy": "no-referrer",
} as const;

export function isAppealPath(pathname: string): boolean {
  return pathname === APPEAL_PATH || pathname.startsWith(`${APPEAL_PATH}/`);
}

/**
 * Reads a token from what the person pasted: the bare token, or the full link
 * from the notice email. Returns null for anything else, and nothing is sent.
 */
export function appealTokenFromInput(input: string): string | null {
  const trimmed = input.trim();
  const candidate = trimmed.includes("/appeal/")
    ? trimmed.slice(trimmed.lastIndexOf("/appeal/") + "/appeal/".length)
    : trimmed;
  const token = candidate.replace(/[/?#].*$/u, "");
  return APPEAL_TOKEN_PATTERN.test(token) ? token : null;
}
