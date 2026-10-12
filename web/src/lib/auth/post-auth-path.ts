import type { UserProfile } from "#lib/auth/auth-client.js";

/**
 * Where to send the browser right after a successful login/passkey sign-in:
 * unverified users go finish email verification first, everyone else lands on
 * `returnTarget` (the page they signed in from) or falls back to the app root.
 */
export function postAuthPath(user: UserProfile | null, returnTarget?: string | null): string {
  if (user && !user.is_verified) return "/verify-email";
  return returnTarget || "/surah";
}
