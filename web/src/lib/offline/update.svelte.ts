import { browser } from "$app/environment";
import { updated } from "$app/state";
import { registerServiceWorker } from "$lib/boot/service-worker";
import {
  PREPARE_RELOAD,
  PREPARE_RELOAD_EVENT,
  requestWorkerVersion,
  SKIP_WAITING,
  SW_BROADCAST_CHANNEL,
  UPDATE_BROADCAST_CHANNEL,
  UPDATE_TAKEOVER,
} from "$lib/offline/messages";
import { readRaw, removeRaw, writeRaw } from "$lib/storage";

const RELOAD_GUARD = "easyquran.reload-guard";
// Legacy paint key from the pre-rev3 design: nothing reads it anymore; boot
// deletes it once so a stale "waiting" flag can never force a banner (I2).
const LEGACY_PAINT_KEY = "easyquran.update.waiting";
const DISMISS_KEY = "easyquran.update.dismissed";
// Sentinel used when the waiting worker's version is unknown (handshake
// pending/timed out/pre-feature SW that never answers).
const DISMISS_UNKNOWN = "*";
const UPDATE_CHECK_INTERVAL_MS = 5 * 60_000;
const INTERACTION_EVENTS = [
  "pointerdown",
  "keydown",
  "pointermove",
  "wheel",
  "touchstart",
] as const;

class UpdateStore {
  // Reactive graph state: the banner derives everything from these (I1).
  #waiting = $state(false);
  #controllerVersion = $state<string | null>(null);
  #waitingVersion = $state<string | null>(null);
  #dismissalRecord = $state<string | null>(null);

  #hydrated = false;
  #reloadArmed = false;
  // Live worker identities (not reactive; versions above are).
  #waitingWorker: ServiceWorker | null = null;
  // Suppression scoped to a pending fresh-tab silent adoption for THIS exact
  // worker identity; released on abort and on any reconcile where the
  // registration's waiting worker no longer matches (I4/R6). Reactive (.raw:
  // reassigned by identity, never mutated) because `available` reads it — a
  // plain field would be invisible to Svelte's dependency tracking and the
  // banner would stay hidden after an abort nulls it.
  #silentTarget = $state.raw<ServiceWorker | null>(null);
  #silentToken = 0;
  #interacted = false;
  #registration: ServiceWorkerRegistration | null = null;
  #registrationResolved = false;
  #channel: BroadcastChannel | null = null;
  #cleanups: Array<() => void> = [];
  #lastUpdateCheckAt: number | null = null;
  #updateCheckInFlight: Promise<void> | null = null;
  #versionFallbackWired = false;
  // True once the no-SW fallback owns availability (SvelteKit `updated`).
  // Reactive because the `dismissed` getter reads it alongside the fallback
  // dismissal key — the banner's derived visibility must invalidate when the
  // fallback wires up (I8: the fallback path keeps full dismissal UX).
  #fallbackMode = $state(false);

  get waiting(): boolean {
    return this.#waiting;
  }

  get available(): boolean {
    return this.#computeAvailable();
  }

  // True when the CURRENT waiting worker's dismissal record matches: either
  // `v:<version>` for a known waiting version or the `*` sentinel when the
  // version is unknown (I3). In no-SW fallback mode the dismissal is keyed to
  // the SvelteKit build version (`updated.version`) — a different version
  // after the next deploy re-prompts (I3 reset-on-new-deploy).
  get dismissed(): boolean {
    if (this.#fallbackMode) return this.#dismissalRecord === this.#fallbackDismissalKey();
    if (!this.#waiting) return false;
    const version = this.#waitingVersion;
    if (version !== null) return this.#dismissalRecord === `v:${version}`;
    return this.#dismissalRecord === DISMISS_UNKNOWN;
  }

  // I3 key for the no-SW fallback: the current SvelteKit build id when
  // accessible, else the unknown-version sentinel. Newer SvelteKit exposes
  // `updated.version` at runtime; the pinned type does not declare it, so
  // widen structurally and treat any absent/empty value as unknown.
  #fallbackDismissalKey(): string {
    // SAFETY: optional-property widening only — every field read afterwards
    // re-checks for absent/empty, so a missing or non-string runtime value
    // degrades to the `*` sentinel instead of trusting the assertion.
    const withVersion = updated as { readonly version?: string };
    const version = withVersion.version;
    if (version !== undefined && version.length > 0) return `v:${version}`;
    return DISMISS_UNKNOWN;
  }

  // Availability is live-derived only (I1): persisted flags can never set it.
  #computeAvailable(): boolean {
    if (!browser) return false;
    if (!("serviceWorker" in navigator)) return updated.current;
    if (!this.#waiting) return false;
    // Pending fresh-tab silent adoption for this exact worker: hide banner.
    if (this.#silentTarget !== null) return false;
    // Both versions known AND equal -> the waiting worker is not an update.
    // Any unknown (handshake pending/timed out, uncontrolled first load)
    // stays indeterminate -> banner eligibility per I1 fallback.
    const controller = this.#controllerVersion;
    const candidate = this.#waitingVersion;
    if (controller === null || candidate === null) return true;
    return controller !== candidate;
  }

  hydrate(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;

    if (readRaw("session", RELOAD_GUARD) === "1") this.#reloadArmed = true;
    const storedDismissal = readRaw("session", DISMISS_KEY);
    this.#dismissalRecord = storedDismissal;

    if ("BroadcastChannel" in globalThis) {
      this.#channel = new BroadcastChannel(UPDATE_BROADCAST_CHANNEL);
      this.#channel.addEventListener("message", (event) => {
        if (event.data?.type === PREPARE_RELOAD) this.#armReloadGuard();
      });
      this.#cleanups.push(() => this.#channel?.close());

      const swChannel = new BroadcastChannel(SW_BROADCAST_CHANNEL);
      swChannel.addEventListener("message", (event) => {
        if (event.data?.type === UPDATE_TAKEOVER) this.#evaluateReload();
      });
      this.#cleanups.push(() => swChannel.close());
    }

    // I2 legacy cleanup: the old sticky paint flag is dead state; a stale
    // value from the previous design must never survive boot.
    removeRaw("local", LEGACY_PAINT_KEY);

    this.#wireInteractionListeners();

    if ("serviceWorker" in navigator) {
      void this.#wireServiceWorker();
    } else {
      this.#wireVersionFallback();
    }
  }

  #wireInteractionListeners(): void {
    const onInteract = (): void => {
      this.#interacted = true;
    };
    for (const type of INTERACTION_EVENTS) {
      window.addEventListener(type, onInteract, { passive: true });
      this.#cleanups.push(() => window.removeEventListener(type, onInteract));
    }
  }

  async #wireServiceWorker(): Promise<void> {
    const reg = await this.#getRegistration();
    if (!reg) {
      this.#wireVersionFallback();
      return;
    }
    // Initial sync: keep the dismissal read from sessionStorage (not a new
    // waiting transition this session). Later transitions reset it (I3).
    this.#syncWaiting(reg, true);
    this.#lastUpdateCheckAt = Date.now();
    reg.addEventListener("updatefound", this.#onUpdateFound);

    const onControllerChange = (): void => {
      // New controller = new waiting-worker transition for dismissal (I3);
      // re-query the active side under its own identity guard (N14).
      this.#resetDismissal();
      this.#controllerVersion = null;
      const reg2 = this.#registration;
      const active = navigator.serviceWorker.controller ?? reg2?.active ?? null;
      if (active) this.#queryControllerVersion(active);
      this.#syncWaiting(reg2);
      const ctrl = navigator.serviceWorker.controller;
      if (ctrl) this.#evaluateReload();
    };
    navigator.serviceWorker.addEventListener("controllerchange", onControllerChange);

    const onVisibility = (): void => {
      if (document.visibilityState !== "visible") return;
      // Sync the local graph BEFORE the throttle gate (N13): a throttled
      // focus-return must still repaint/clear the banner from live state.
      this.#syncWaiting(this.#registration);
      void this.#checkServiceWorkerUpdate();
    };
    document.addEventListener("visibilitychange", onVisibility);

    this.#cleanups.push(
      () => reg.removeEventListener("updatefound", this.#onUpdateFound),
      () => navigator.serviceWorker.removeEventListener("controllerchange", onControllerChange),
      () => document.removeEventListener("visibilitychange", onVisibility),
    );

    // Handshake: controller first (uncontrolled first-load -> controller null
    // -> indeterminate per I1), then the waiting worker. Never installing.
    const controller = navigator.serviceWorker.controller ?? reg.active ?? null;
    if (controller) this.#queryControllerVersion(controller);

    // Fresh-tab silent adoption (I4): waiting-exists alone, before any user
    // interaction, THIS tab only. Guard armed at hydrate means a silent (or
    // consented) flow already happened -> never silent again this session.
    const waiting = reg.waiting;
    if (waiting && !this.#reloadArmed && !this.#interacted) {
      await this.#attemptSilentAdoption(waiting);
    }
  }

  #wireVersionFallback(): void {
    if (this.#versionFallbackWired) return;
    this.#versionFallbackWired = true;
    this.#fallbackMode = true;
    const check = (): void => {
      void updated.check().catch(() => {});
    };
    const onVisibility = (): void => {
      if (document.visibilityState === "visible") check();
    };
    // No-SW silent rule (I4/F6): SvelteKit updated.current true before any
    // interaction -> arm the same one-shot guard and reload directly. Guard
    // armed at hydrate -> never silent-reload again this session.
    const attemptSilent = (): void => {
      if (!updated.current) return;
      if (this.#reloadArmed) return;
      if (this.#interacted) return;
      this.#armReloadGuard();
      if (this.#interacted) {
        this.#reloadArmed = false;
        removeRaw("session", RELOAD_GUARD);
        return;
      }
      window.location.reload();
    };
    void updated
      .check()
      .catch(() => {})
      .then(attemptSilent);
    document.addEventListener("visibilitychange", onVisibility);
    this.#cleanups.push(() => document.removeEventListener("visibilitychange", onVisibility));
  }

  #checkServiceWorkerUpdate(): Promise<void> {
    if (this.#updateCheckInFlight) return this.#updateCheckInFlight;
    const now = Date.now();
    if (
      this.#lastUpdateCheckAt !== null &&
      now - this.#lastUpdateCheckAt < UPDATE_CHECK_INTERVAL_MS
    ) {
      return Promise.resolve();
    }
    this.#lastUpdateCheckAt = now;
    const run = (async (): Promise<void> => {
      const reg = await this.#getRegistration();
      if (!reg) return;
      try {
        await reg.update();
      } catch {}
      this.#syncWaiting(reg);
    })();
    this.#updateCheckInFlight = run;
    const clearInFlight = (): void => {
      if (this.#updateCheckInFlight === run) this.#updateCheckInFlight = null;
    };
    void run.then(clearInFlight, clearInFlight);
    return run;
  }

  readonly #onUpdateFound = (): void => {
    const installing = this.#registration?.installing;
    if (!installing) {
      this.#syncWaiting(this.#registration);
      return;
    }
    installing.addEventListener("statechange", () => {
      this.#syncWaiting(this.#registration);
    });
  };

  // Live-graph reconcile. `initial` marks the first sync after hydrate: the
  // dismissal just read from sessionStorage still applies; any LATER change
  // of the waiting worker identity is a new transition and resets it (I3).
  #syncWaiting(reg: ServiceWorkerRegistration | null | undefined, initial = false): void {
    const next = reg?.waiting ?? null;
    const changed = this.#waitingWorker !== next;
    this.#waitingWorker = next;
    this.#waiting = next !== null;
    if (changed) {
      this.#waitingVersion = null;
      if (!initial) this.#resetDismissal();
    }
    // Suppression is scoped to the targeted worker identity (R6): if the
    // waiting worker moved on, the pending silent attempt is moot.
    if (this.#silentTarget !== null && next !== this.#silentTarget) this.#silentTarget = null;
    if (next !== null && changed) this.#queryWaitingVersion(next);
  }

  // Active-side handshake with its own identity guard (N14): a late answer
  // only applies while the answered worker is still the active/controller one.
  #queryControllerVersion(worker: ServiceWorker): void {
    void requestWorkerVersion(worker).then((version) => {
      const current = navigator.serviceWorker.controller ?? this.#registration?.active ?? null;
      if (current !== worker) return;
      if (version === null) return;
      this.#controllerVersion = version;
    });
  }

  // Waiting-side handshake under the R4 identity guard: the answer applies
  // ONLY while reg.waiting === the worker that answered; stale answers from a
  // superseded waiting worker are discarded and availability recomputes from
  // the live graph.
  #queryWaitingVersion(worker: ServiceWorker): void {
    void requestWorkerVersion(worker).then((version) => {
      if (this.#waitingWorker !== worker) return;
      if (version === null) return;
      this.#waitingVersion = version;
      // Late same-version answer while still applicable: not an update after
      // all -> drop availability and any dismissal pinned to it (I1).
      if (version === this.#controllerVersion && this.#controllerVersion !== null) {
        this.#resetDismissal();
      }
    });
  }

  async #attemptSilentAdoption(waiting: ServiceWorker): Promise<void> {
    const token = ++this.#silentToken;
    this.#armReloadGuard();
    this.#silentTarget = waiting;
    // Recheck 1 (R3): abort before sending anything.
    if (this.#interacted) {
      this.#abortSilentAdoption(token);
      return;
    }
    try {
      waiting.postMessage({ type: SKIP_WAITING });
    } catch {
      this.#abortSilentAdoption(token);
      return;
    }
    // Yield one macrotask: the final interaction recheck must be a real
    // window, not a formality (F2).
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    // dispose() (or a superseding attempt) invalidates the token.
    if (token !== this.#silentToken) return;
    // Recheck 2 (F2/R3): ghost click -> disarm so the later controllerchange
    // cannot reload a tab the user already touched.
    if (this.#interacted || this.#silentTarget !== waiting) {
      this.#abortSilentAdoption(token);
      return;
    }
    window.location.reload();
    // Suppression is only needed until the reload lands; if navigation is
    // somehow blocked the truthful banner returns (bounded suppression, R6).
    this.#silentTarget = null;
  }

  #abortSilentAdoption(token: number): void {
    if (token !== this.#silentToken) return;
    this.#silentTarget = null;
    // Synchronous disarm (R3): memory + sessionStorage, so a subsequent
    // controllerchange finds no armed guard and never ghost-reloads.
    if (this.#reloadArmed) {
      this.#reloadArmed = false;
      removeRaw("session", RELOAD_GUARD);
    }
  }

  async #getRegistration(): Promise<ServiceWorkerRegistration | null> {
    if (this.#registrationResolved) return this.#registration;
    this.#registrationResolved = true;
    this.#registration = await registerServiceWorker();
    return this.#registration;
  }

  #armReloadGuard(): void {
    this.#reloadArmed = true;
    writeRaw("session", RELOAD_GUARD, "1");
  }

  // Interaction-blind by design (R5): only the silent path re-checks
  // interaction; consented coordinated reloads must never refuse.
  #evaluateReload(): void {
    if (!this.#reloadArmed) return;
    this.#reloadArmed = false;
    removeRaw("session", RELOAD_GUARD);
    this.#waiting = false;
    this.#waitingWorker = null;
    this.#waitingVersion = null;
    window.location.reload();
  }

  #resetDismissal(): void {
    this.#dismissalRecord = null;
    removeRaw("session", DISMISS_KEY);
  }

  dismiss(): void {
    // No-SW fallback (I8): the banner must be dismissable there too. Record
    // per I3 under the SvelteKit build version (or the `*` sentinel when it
    // is not accessible) so a new deploy re-prompts while the same build
    // stays dismissed for this tab session.
    if (this.#fallbackMode) {
      const fallbackRecord = this.#fallbackDismissalKey();
      this.#dismissalRecord = fallbackRecord;
      writeRaw("session", DISMISS_KEY, fallbackRecord);
      return;
    }
    if (!this.#waiting) return;
    const version = this.#waitingVersion;
    const record = version !== null ? `v:${version}` : DISMISS_UNKNOWN;
    this.#dismissalRecord = record;
    writeRaw("session", DISMISS_KEY, record);
  }

  apply(): void {
    const waiting = this.#registration?.waiting;
    if (waiting) {
      this.#armReloadGuard();
      this.#channel?.postMessage({ type: PREPARE_RELOAD });
      try {
        window.dispatchEvent(new CustomEvent(PREPARE_RELOAD_EVENT));
      } catch {}
      waiting.postMessage({ type: SKIP_WAITING });
      return;
    }
    if (!("serviceWorker" in navigator) && updated.current) window.location.reload();
  }

  dispose(): void {
    // Abort any in-flight silent attempt (N8): HMR remount must not fire a
    // pending reload. The token invalidates the awaiting continuation.
    this.#silentToken++;
    if (this.#silentTarget !== null) {
      this.#silentTarget = null;
      if (this.#reloadArmed) {
        this.#reloadArmed = false;
        removeRaw("session", RELOAD_GUARD);
      }
    }
    for (const cleanup of this.#cleanups) cleanup();
    this.#cleanups = [];
    this.#channel = null;
    this.#registration = null;
    this.#registrationResolved = false;
    this.#lastUpdateCheckAt = null;
    this.#updateCheckInFlight = null;
    this.#versionFallbackWired = false;
    this.#fallbackMode = false;
    this.#waitingWorker = null;
    this.#waitingVersion = null;
    this.#controllerVersion = null;
    this.#waiting = false;
    this.#hydrated = false;
  }
}

export function createUpdate(): UpdateStore {
  return new UpdateStore();
}

export const update = createUpdate();
