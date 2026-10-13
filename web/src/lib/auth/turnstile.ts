import { dev } from "$app/env";
import { PUBLIC_TURNSTILE_SITE_KEY } from "$app/env/public";

/** Public sitekey; empty disables the widget (server gate stays off unless its secret is set). */
export const TURNSTILE_SITE_KEY = (PUBLIC_TURNSTILE_SITE_KEY ?? "").trim();

// Dev also disables the widget even with a sitekey: Cloudflare's challenge
// script runs debugger-based bot traps that pause DevTools and floods the
// console from its challenge iframe (XHR churn, OTS font errors), while the
// local API's Turnstile gate is off without a secret — so the widget only
// tests noise in development.
export function turnstileEnabled(): boolean {
  return !dev && TURNSTILE_SITE_KEY.length > 0;
}

export interface TurnstileRenderOptions {
  sitekey: string;
  theme: "auto" | "light" | "dark";
  callback: (token: string) => void;
  "error-callback": () => void;
  "expired-callback": () => void;
}

export interface TurnstileApi {
  render(container: HTMLElement, options: TurnstileRenderOptions): string;
  reset(widgetId?: string): void;
  remove(widgetId: string): void;
}

declare global {
  interface Window {
    turnstile?: TurnstileApi;
  }
}

const SCRIPT_SRC = "https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit";

let loader: Promise<TurnstileApi> | null = null;

/** Loads the Turnstile script once; every caller shares the same promise. */
export function loadTurnstile(): Promise<TurnstileApi> {
  if (window.turnstile) return Promise.resolve(window.turnstile);
  if (loader) return loader;
  loader = new Promise<TurnstileApi>((resolve, reject) => {
    const existing = document.querySelector<HTMLScriptElement>(`script[src="${SCRIPT_SRC}"]`);
    const script = existing ?? document.createElement("script");
    script.addEventListener(
      "load",
      () => {
        if (window.turnstile) resolve(window.turnstile);
        else reject(new Error("turnstile unavailable after load"));
      },
      { once: true },
    );
    script.addEventListener(
      "error",
      () => {
        loader = null;
        reject(new Error("turnstile script failed"));
      },
      { once: true },
    );
    if (!existing) {
      script.src = SCRIPT_SRC;
      script.async = true;
      script.defer = true;
      document.head.append(script);
    }
  });
  return loader;
}
