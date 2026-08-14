import { mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
vi.mock("$env/dynamic/public", () => ({ env: { PUBLIC_API_BASE_URL: "https://eq.test/api" } }));

import type { AuthClient } from "$lib/auth/auth-client";
import { createRegisterFlow } from "$lib/auth/flows.svelte";
import { getAuthValidationCopy } from "$lib/i18n/auth-validation-copy";
import RegisterForm from "../RegisterForm.svelte";

const copy = getAuthValidationCopy("en");
const PASSWORD = "correct-horse-battery";

function mockClient(unsafeRequest: ReturnType<typeof vi.fn>) {
  // SAFETY: hand-built AuthClient test double — the class's #private csrf members are never
  // touched by the auth flows, and every member they invoke is stubbed as a vi.fn().
  return {
    unsafeRequest,
    refreshCsrf: vi.fn().mockResolvedValue(undefined),
    clearCsrf: vi.fn(),
    getUser: vi.fn(),
  } as AuthClient & {
    unsafeRequest: ReturnType<typeof vi.fn>;
    refreshCsrf: ReturnType<typeof vi.fn>;
    clearCsrf: ReturnType<typeof vi.fn>;
    getUser: ReturnType<typeof vi.fn>;
  };
}

function mockState() {
  return {
    transition: vi.fn().mockResolvedValue(undefined),
    setUser: vi.fn(),
    setTwoFaPending: vi.fn(),
    reset: vi.fn(),
    probe: vi.fn().mockResolvedValue({ kind: "anonymous" }),
  };
}

let target: HTMLElement;
let unmountForm: () => void = () => {};

async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function errorTexts(): Array<string> {
  return [...target.querySelectorAll("[role='alert']")].map((el) => el.textContent?.trim() ?? "");
}

async function type(id: string, value: string): Promise<void> {
  const el = target.querySelector<HTMLInputElement>(`#${id}`);
  if (!el) throw new Error(`missing input #${id}`);
  el.value = value;
  el.dispatchEvent(new Event("input", { bubbles: true }));
  await settle();
}

async function submit(): Promise<void> {
  const form = target.querySelector("form");
  if (!form) throw new Error("missing form");
  form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  await settle();
}

function mountForm(unsafeRequest = vi.fn()) {
  const flow = createRegisterFlow({ client: mockClient(unsafeRequest), state: mockState() });
  const onsuccess = vi.fn();
  const instance = mount(RegisterForm, { target, props: { flow, onsuccess } });
  unmountForm = () => {
    void unmount(instance);
  };
  return { flow, onsuccess };
}

beforeEach(() => {
  target = document.createElement("div");
  document.body.appendChild(target);
});

afterEach(() => {
  unmountForm();
  target.remove();
});

describe("RegisterForm validation UX", () => {
  it("stays quiet until the first submit, then reports every field", async () => {
    const request = vi.fn();
    mountForm(request);

    await type("register-email", "nope");
    expect(errorTexts()).toEqual([]);

    await submit();
    expect(errorTexts()).toEqual([
      copy.nameRequired,
      copy.emailInvalid,
      copy.passwordRequired,
      copy.confirmRequired,
    ]);
    expect(request).not.toHaveBeenCalled();
  });

  it("blames confirm_password for a mismatch and clears it live", async () => {
    mountForm();

    await type("register-name", "Sara");
    await type("register-email", "sara@eq.test");
    await type("register-password", PASSWORD);
    await type("register-confirm", `${PASSWORD}!`);
    await submit();

    expect(errorTexts()).toEqual([copy.confirmMismatch]);

    await type("register-confirm", PASSWORD);
    expect(errorTexts()).toEqual([]);
  });

  it("rejects a password under the server floor of 12", async () => {
    mountForm();

    await type("register-name", "Sara");
    await type("register-email", "sara@eq.test");
    await type("register-password", "short");
    await type("register-confirm", "short");
    await submit();

    expect(errorTexts()).toEqual([copy.passwordMin]);
  });

  it("posts a trimmed payload once valid", async () => {
    const request = vi.fn().mockResolvedValue({
      ok: false,
      status: 400,
      rotated: false,
      data: null,
      error: { context: { errors: { email: "That email is already registered." } } },
    });
    mountForm(request);

    await type("register-name", "  Sara  ");
    await type("register-email", " sara@eq.test ");
    await type("register-password", PASSWORD);
    await type("register-confirm", PASSWORD);
    await submit();

    expect(request).toHaveBeenCalledWith("/auth/v1/register", {
      method: "POST",
      body: {
        name: "Sara",
        email: "sara@eq.test",
        password: PASSWORD,
        confirm_password: PASSWORD,
      },
    });
    expect(errorTexts()).toContain("That email is already registered.");
  });
});
