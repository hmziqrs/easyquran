/* ════════════════════════════════════════════════════════════════════════
   reader-context.svelte.ts — a SELECTIVE context binding for the reader.

   Per docs/svelte-improvements.md §4.C, Svelte's type-safe `createContext` is
   introduced ONLY where it buys something concrete. The reader qualifies: it is
   scoped to the /app subtree, and a context lets a component test inject a
   fresh `createReader()` instance (or a stub) without reaching into the module
   singleton. prefs/consent/notifications are whole-browser-session state and
   stay singletons; the Worker and FCM lifecycles stay explicit services. Do
   not migrate every singleton.

   Adoption is opt-in: `useReader()` falls back to the app-wide `reader`
   singleton when no provider is mounted, so existing direct imports
   (`import { reader } from "$lib/stores/reader.svelte"`) keep working until a
   subtree chooses to provide its own.
   ════════════════════════════════════════════════════════════════════════ */

import { createContext } from "svelte";
import { reader as defaultReader, type ReaderApi } from "./reader.svelte";

const [useReaderCtx, setReaderCtx] = createContext<ReaderApi | undefined>();

/**
 * Provide a reader instance to the current subtree. Call during a component's
 * init (the same rule as Svelte's setContext). Defaults to the app singleton so
 * a bare `<ReaderProvider/>` is a no-op compatibility layer.
 */
export function setReaderContext(instance: ReaderApi = defaultReader): ReaderApi {
  setReaderCtx(instance);
  return instance;
}

export function useReader(): ReaderApi {
  return useReaderCtx() ?? defaultReader;
}
