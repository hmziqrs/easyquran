import { browser } from "$app/env";

import { isIdentityProvider, type IdentityProvider } from "#lib/auth/account-client.js";

// The link flow leaves the app for the provider and returns via a server redirect
// (/account?linked=… | /account?link_error=…). Error redirects carry no provider,
// so the attempted provider is stashed before navigation and consumed on return
// to name it in the banner. Same cross-roundtrip pattern as return-target.ts.
const KEY = "eq:link-provider";

export function rememberLinkProvider(provider: IdentityProvider): void {
  if (!browser) return;
  try {
    sessionStorage.setItem(KEY, provider);
  } catch {}
}

export function consumeLinkProvider(): IdentityProvider | null {
  if (!browser) return null;
  let raw: string | null = null;
  try {
    raw = sessionStorage.getItem(KEY);
    sessionStorage.removeItem(KEY);
  } catch {
    return null;
  }
  if (raw === null || !isIdentityProvider(raw)) return null;
  return raw;
}
