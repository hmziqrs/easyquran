/**
 * Per-page metadata for the auth form pages, resolved in the layout because the
 * `(auth)` layout gates children behind the session probe — a title living in
 * `+page.svelte` would be absent from the prerendered HTML (verified: reader
 * pages render titles, gated auth pages did not).
 */

export type AuthFooterLinkId = "privacy" | "terms" | "contact" | "faq";

const LINKS_BY_PATH = {
  "/login": ["privacy", "terms"],
  "/register": ["privacy", "terms"],
  "/forgot-password": ["contact", "faq"],
  "/verify-email": ["contact", "faq"],
} satisfies Record<string, readonly AuthFooterLinkId[]>;

const TITLES_BY_PATH = {
  "/login": "Sign in · EasyQuran",
  "/register": "Create account · EasyQuran",
  "/forgot-password": "Reset your password · EasyQuran",
  "/verify-email": "Verify your email · EasyQuran",
} satisfies Record<string, string>;

function normalize(pathname: string): string {
  return pathname.replace(/\/+$/u, "") || "/";
}

/**
 * Quiet footer links for the auth form pages. Each page answers a question the
 * user has on that page: account creation shows the legal pair, recovery and
 * verification show the support pair.
 */
export function authFooterLinkIds(pathname: string): readonly AuthFooterLinkId[] {
  const normalized = normalize(pathname);
  for (const [path, ids] of Object.entries(LINKS_BY_PATH)) {
    if (path === normalized) return ids;
  }
  return [];
}

export function authPageTitle(pathname: string): string | null {
  const normalized = normalize(pathname);
  for (const [path, title] of Object.entries(TITLES_BY_PATH)) {
    if (path === normalized) return title;
  }
  return null;
}
