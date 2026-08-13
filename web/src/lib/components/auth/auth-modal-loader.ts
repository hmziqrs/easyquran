import type { Component } from "svelte";

let pending: Promise<Component> | null = null;

export function loadAuthModal(): Promise<Component> {
  pending ??= import("./AuthModal.svelte").then((m) => m.default as Component);
  return pending;
}
