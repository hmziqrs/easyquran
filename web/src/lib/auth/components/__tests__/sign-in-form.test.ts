import { mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
vi.mock("$env/dynamic/public", () => ({ env: { PUBLIC_API_BASE_URL: "https://eq.test/api" } }));

import type { AuthClient } from "$lib/auth/auth-client";
import { createLoginFlow } from "$lib/auth/flows.svelte";
import { getAuthValidationCopy } from "$lib/i18n/auth-validation-copy";
import SignInForm from "../SignInForm.svelte";

const copy = getAuthValidationCopy("en");

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

function input(id: string): HTMLInputElement {
  const el = target.querySelector<HTMLInputElement>(`#${id}`);
  if (!el) throw new Error(`missing input #${id}`);
  return el;
}

async function type(id: string, value: string): Promise<void> {
  const el = input(id);
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
  const flow = createLoginFlow({ client: mockClient(unsafeRequest), state: mockState() });
  const onsuccess = vi.fn();
  const instance = mount(SignInForm, { target, props: { flow, onsuccess } });
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

describe("SignInForm validation UX", () => {
  it("shows no errors before the first submit, even while typing", async () => {
    mountForm();
    await type("signin-email", "not-an-email");
    await type("signin-password", "x");

    expect(errorTexts()).toEqual([]);
    expect(target.querySelector("[data-invalid='true']")).toBeNull();
  });

  it("reports every invalid field on submit and does not call the API", async () => {
    const request = vi.fn();
    mountForm(request);

    await submit();

    expect(errorTexts()).toEqual([copy.emailRequired, copy.passwordRequired]);
    expect(request).not.toHaveBeenCalled();
  });

  it("revalidates on change once submitted, clearing each field as it is fixed", async () => {
    mountForm();
    await submit();
    expect(errorTexts()).toEqual([copy.emailRequired, copy.passwordRequired]);

    await type("signin-email", "sara@eq.test");
    expect(errorTexts()).toEqual([copy.passwordRequired]);

    await type("signin-password", "correct-horse-battery");
    expect(errorTexts()).toEqual([]);
  });

  it("moves focus to the first invalid field on a failed submit", async () => {
    mountForm();
    await submit();

    expect(document.activeElement).toBe(input("signin-email"));
  });

  it("submits trimmed credentials once the form is valid", async () => {
    const request = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      rotated: true,
      error: null,
      data: {
        id: 7,
        name: "Sara",
        email: "sara@eq.test",
        avatar_id: null,
        is_verified: true,
        role: "user",
        two_fa_enabled: false,
        oauth_provider: null,
      },
    });
    const { onsuccess } = mountForm(request);

    await type("signin-email", "  sara@eq.test  ");
    await type("signin-password", "correct-horse-battery");
    await submit();

    expect(request).toHaveBeenCalledWith("/auth/v1/log_in", {
      method: "POST",
      body: { email: "sara@eq.test", password: "correct-horse-battery" },
    });
    expect(onsuccess).toHaveBeenCalledTimes(1);
  });

  it("keeps a server field error until that field is edited", async () => {
    const request = vi.fn().mockResolvedValue({
      ok: false,
      status: 400,
      rotated: false,
      data: null,
      error: { context: { errors: { email: "That email is not registered." } } },
    });
    mountForm(request);

    await type("signin-email", "sara@eq.test");
    await type("signin-password", "correct-horse-battery");
    await submit();

    expect(errorTexts()).toContain("That email is not registered.");

    await type("signin-password", "correct-horse-batteryy");
    expect(errorTexts()).toContain("That email is not registered.");

    await type("signin-email", "sara2@eq.test");
    expect(errorTexts()).toEqual([]);
  });
});
