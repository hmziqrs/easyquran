import type { Component } from "svelte";

let pending: Promise<Component> | null = null;

export function loadAuthModal(): Promise<Component> {
  // SAFETY: the Svelte compiler emits AuthModal.svelte's default export as a Component.
  pending ??= import("./AuthModal.svelte").then((m) => m.default as Component);
  return pending;
}
