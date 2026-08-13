import type { UserProfile } from "$lib/auth/auth-client";

/**
 * Where to send the browser right after a successful login/passkey sign-in:
 * unverified users go finish email verification first, everyone else goes to the app.
 */
export function postAuthPath(user: UserProfile | null): "/verify-email" | "/app" {
  return user && !user.is_verified ? "/verify-email" : "/app";
}
