import { createSyncEngine, type RegisteredSyncDomain, type SyncEngine } from "./engine.svelte";

/**
 * App-wide engine singleton. Domains are not known at module init — features
 * call `registerDomain` from their own boot. Constructing the engine touches no
 * browser API: IndexedDB is opened lazily on the first enqueue/flush/hydrate.
 */
export const syncEngine: SyncEngine = createSyncEngine();

/** Register a domain (idempotent by name) and refresh the pending count. */
export function registerDomain(domain: RegisteredSyncDomain): void {
  syncEngine.register(domain);
  void syncEngine.hydrate();
}

export function unregisterDomain(name: string): void {
  syncEngine.unregister(name);
}
