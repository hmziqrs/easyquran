import { createSyncEngine, type RegisteredSyncDomain, type SyncEngine } from "./engine.svelte";

/**
 * App-wide engine singleton. Domains are not known at module init — features
 * call `registerDomain` from their own boot. Constructing the engine touches no
 * browser API: IndexedDB is opened lazily on the first enqueue/flush/hydrate.
 */
export const syncEngine: SyncEngine = createSyncEngine();

const registered = new Set<string>();

/** Register a domain (idempotent by name) and refresh the pending count. */
export function registerDomain(domain: RegisteredSyncDomain): void {
  if (registered.has(domain.name)) return;
  registered.add(domain.name);
  syncEngine.register(domain);
  void syncEngine.hydrate();
}
