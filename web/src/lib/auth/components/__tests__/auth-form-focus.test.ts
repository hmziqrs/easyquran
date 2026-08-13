import { focusFirstInvalid } from "$lib/auth/components/auth-form-focus";
import { afterEach, describe, expect, it } from "vite-plus/test";

afterEach(() => {
  document.body.innerHTML = "";
});

describe("focusFirstInvalid", () => {
  it("focuses the field marked data-invalid='true'", () => {
    document.body.innerHTML = `
      <form>
        <input name="email" />
        <input name="password" data-invalid="true" />
      </form>
    `;
    const password = document.body.querySelector<HTMLInputElement>("[name='password']")!;

    expect(focusFirstInvalid(document.body)).toBe(true);
    expect(document.activeElement).toBe(password);
  });

  it("focuses the first aria-invalid='true' field when no data-invalid present", () => {
    document.body.innerHTML = `
      <form>
        <input name="code" aria-invalid="true" />
        <input name="other" aria-invalid="true" />
      </form>
    `;
    const code = document.body.querySelector<HTMLInputElement>("[name='code']")!;

    expect(focusFirstInvalid(document.body)).toBe(true);
    expect(document.activeElement).toBe(code);
  });

  it("prefers the first match in document order (data-invalid before aria-invalid)", () => {
    document.body.innerHTML = `
      <form>
        <input name="first" data-invalid="true" />
        <input name="second" aria-invalid="true" />
      </form>
    `;
    const first = document.body.querySelector<HTMLInputElement>("[name='first']")!;

    focusFirstInvalid(document.body);
    expect(document.activeElement).toBe(first);
  });

  it("returns false and focuses nothing when all fields are valid", () => {
    document.body.innerHTML = `
      <form>
        <input name="email" />
        <input name="password" />
      </form>
    `;
    const before = document.activeElement;

    expect(focusFirstInvalid(document.body)).toBe(false);
    expect(document.activeElement).toBe(before);
  });

  it("scopes the query to the given root (ignores invalid fields outside)", () => {
    document.body.innerHTML = `
      <div id="other"><input name="outside" data-invalid="true" /></div>
      <form id="form"><input name="inside" /></form>
    `;
    const form = document.body.querySelector("#form")!;
    const outside = document.body.querySelector<HTMLInputElement>("[name='outside']")!;

    expect(focusFirstInvalid(form)).toBe(false);
    expect(document.activeElement).not.toBe(outside);
  });
});
