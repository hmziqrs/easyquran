import { safeParse, type BaseIssue, type GenericSchema } from "valibot";

export type FieldErrorMap = Readonly<Record<string, string>>;

type Issue = BaseIssue<unknown>;

/**
 * First message per top-level field, in schema order. Nested paths collapse onto
 * their root key because every auth form is flat.
 */
export function issuesToFieldErrors(issues: ReadonlyArray<Issue>) {
  const out: Record<string, string> = {};
  for (const issue of issues) {
    const key = issue.path?.[0]?.key;
    // eslint-disable-next-line anti-slop/no-runtime-typeof -- valibot types a path segment key as `unknown` (it can be an array index); this narrows it to an object key before use.
    if (typeof key !== "string") continue;
    if (out[key] === undefined) out[key] = issue.message;
  }
  return out satisfies FieldErrorMap;
}

/**
 * Field errors the API returned for the last submit. They are shown alongside
 * schema errors and dropped per field as soon as the user edits that field, so
 * "email already registered" disappears the moment the email is changed.
 */
export class ServerFieldErrors {
  #errors = $state<Record<string, string>>({});

  get current(): FieldErrorMap {
    return this.#errors;
  }

  adopt(errors: FieldErrorMap): void {
    this.#errors = { ...errors };
  }

  clearField(name: string): void {
    if (this.#errors[name] === undefined) return;
    const next = { ...this.#errors };
    delete next[name];
    this.#errors = next;
  }

  clearAll(): void {
    if (Object.keys(this.#errors).length === 0) return;
    this.#errors = {};
  }
}

/**
 * Form-level validator for `revalidateLogic`: runs the schema and reports every
 * field at once in TanStack's `{ fields }` shape.
 */
export function dynamicValidator<TValues>(
  schema: GenericSchema<TValues>,
): (args: { value: TValues }) => { fields: Record<string, string> } | undefined {
  return ({ value }) => {
    const result = safeParse(schema, value);
    if (result.success) return undefined;
    return { fields: { ...issuesToFieldErrors(result.issues) } };
  };
}

/** First error message on a TanStack field, or null when the field is clean. */
export function fieldError(errors: ReadonlyArray<unknown>): string | null {
  for (const error of errors) {
    // eslint-disable-next-line anti-slop/no-runtime-typeof -- TanStack types validation errors as `unknown`; this narrows the values our validators produce (plain strings) at the boundary.
    if (typeof error === "string" && error) return error;
    if (!error) continue;
    // eslint-disable-next-line anti-slop/no-runtime-typeof -- same boundary: a standard-schema issue object carries the message under `.message`.
    if (typeof error === "object" && "message" in error) {
      // SAFETY: `message` was just confirmed present on the object; validated as a non-empty string below.
      const message = (error as { message: unknown }).message;
      // eslint-disable-next-line anti-slop/no-runtime-typeof -- narrowing an opaque error payload field to string before display.
      if (typeof message === "string" && message) return message;
    }
  }
  return null;
}
