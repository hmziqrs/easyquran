import type { AuthStatus } from "$lib/auth/auth-state.svelte";

export type AccountView = "loading" | "anonymous" | "authenticated";

export const ACCOUNT_PATH = "/account";
export const ANONYMOUS_CTA_HREF = "/login";
export const REGISTER_HREF = "/register";

export function resolveAccountView(status: AuthStatus): AccountView {
  if (status === "authenticated") return "authenticated";
  if (status === "anonymous") return "anonymous";
  return "loading";
}

export function sessionDeviceLabel(userAgent?: string): string {
  if (!userAgent) return "This device";
  if (/mobile|android|iphone|ipod/i.test(userAgent)) return "Mobile device";
  if (/ipad|tablet/i.test(userAgent)) return "Tablet";
  return "This device";
}
