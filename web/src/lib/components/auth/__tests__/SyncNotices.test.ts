import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

// freshHarness() re-imports the whole graph per test (see comment below); the
// first test pays the cold transform/import cost, which busts the default 5s
// under CI load. Same ceiling LocaleSuggest.test.ts uses.
vi.setConfig({ testTimeout: 15_000 });

// The component binds the real auth-state singleton; only `sessionExpired` is
// needed, so a plain object stands in (each case sets the flag before mount —
// a plain property is not a signal, so it is never flipped mid-test).
const authStateMock = vi.hoisted(() => ({ sessionExpired: false }));

vi.mock("#lib/auth/auth-state.svelte.js", () => ({ authState: authStateMock }));
vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

// Each test re-imports the whole graph (svelte runtime included, so the component
// and mount() share one runtime): the component closes over singletons, and the
// sign-out notice state must be the same instance the component consumed.
async function freshHarness() {
  vi.resetModules();
  const [svelte, notice, authModalModule, component] = await Promise.all([
    import("svelte"),
    import("#lib/auth/sign-out-notice.svelte.js"),
    import("#lib/auth/auth-modal.svelte.js"),
    import("#lib/components/auth/SyncNotices.svelte"),
  ]);
  const target = document.createElement("div");
  document.body.appendChild(target);
  const instance = svelte.mount(component.default, { target });
  return {
    notice,
    authModal: authModalModule.authModal,
    target,
    dispose: () => {
      void svelte.unmount(instance);
    },
  };
}

async function settle(): Promise<void> {
  for (let i = 0; i < 10; i++) await Promise.resolve();
}

let target: HTMLElement | null = null;
let dispose: (() => void) | null = null;

beforeEach(() => {
  authStateMock.sessionExpired = false;
  localStorage.clear();
});

afterEach(() => {
  dispose?.();
  target?.remove();
  target = null;
  document.body.innerHTML = "";
  vi.useRealTimers();
  vi.restoreAllMocks();
  localStorage.clear();
});

describe("SyncNotices (§9.5 sync UI)", () => {
  it("renders nothing while the session is live and no notice is armed", async () => {
    const rig = await freshHarness();
    target = rig.target;
    dispose = rig.dispose;
    await settle();
    expect(rig.target.textContent).toBe("");
  });

  it("shows the session-expired banner and its sign-in action opens the auth modal (EC-15/16/19)", async () => {
    authStateMock.sessionExpired = true;
    const rig = await freshHarness();
    target = rig.target;
    dispose = rig.dispose;
    await settle();
    expect(rig.target.textContent).toContain("Session expired — sign in to resume sync");
    const button = rig.target.querySelector<HTMLButtonElement>("button");
    expect(button).not.toBeNull();
    button?.click();
    await settle();
    expect(rig.authModal.open).toBe(true);
    expect(rig.authModal.mode).toBe("login");
  });

  it("shows the one-shot sign-out notice, consumes it once, and lets it lapse (EC-14 notice)", async () => {
    vi.useFakeTimers();
    const rig = await freshHarness();
    target = rig.target;
    dispose = rig.dispose;
    // Arm after mount: the armed flip is what triggers the render.
    rig.notice.armSignOutNotice(7, (domain) => (domain.endsWith(":7") ? 3 : 0));
    await settle();
    expect(rig.target.textContent).toContain("Your unsynced changes are kept on this device");
    await vi.advanceTimersByTimeAsync(8001);
    await settle();
    expect(rig.target.textContent).not.toContain("Your unsynced changes");
    // Consumed once the line lapses — it can never re-show for this sign-out.
    expect(rig.notice.signOutNoticeArmed()).toBe(false);
  });
});
