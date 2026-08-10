import type { AuthErrorEnvelope } from "$lib/auth/auth-client";

export const CREDENTIAL_FAILURE = "Email or password is incorrect.";
export const ACCOUNT_EXISTS_RESET =
  "If an account exists for that email, a password reset code is on its way.";
export const ACCOUNT_EXISTS_RESEND =
  "If that account exists and is unverified, a new verification code has been sent.";
export const VERIFY_EMAIL_NEXT =
  "Verify your email to continue. Check your inbox for the code we just sent.";
export const TWO_FA_NEXT =
  "Enter the 6-digit code from your authenticator app to finish signing in.";
export const RESET_SUCCESS = "Your password has been reset. You can sign in now.";
export const GENERIC_TRY_AGAIN = "Something went wrong. Please try again.";
export const NETWORK_ERROR = "Network error. Check your connection and try again.";
export const RATE_LIMIT = "Too many attempts. Please wait a moment and try again.";

export type ErrorKind =
  | "field"
  | "credential"
  | "verified-only"
  | "rate-limit"
  | "transport"
  | "server";

export interface ClassifiedError {
  readonly kind: ErrorKind;
  readonly fieldErrors: Readonly<Record<string, string>>;
  readonly message: string;
}

type FieldErrorContext = Record<string, unknown>;

function pickFieldMessage(value: unknown): string | null {
  if (typeof value === "string" && value.trim()) return value;
  if (Array.isArray(value)) {
    for (const entry of value) {
      if (typeof entry === "string" && entry.trim()) return entry;
      if (entry && typeof entry === "object") {
        const obj = entry as Record<string, unknown>;
        const msg = obj.message;
        if (typeof msg === "string" && msg.trim()) return msg;
      }
    }
  } else if (value && typeof value === "object") {
    const obj = value as Record<string, unknown>;
    const msg = obj.message;
    if (typeof msg === "string" && msg.trim()) return msg;
  }
  return null;
}

export function extractFieldErrors(
  error: AuthErrorEnvelope | null,
  fields: ReadonlyArray<string>,
): Record<string, string> {
  const out: Record<string, string> = {};
  if (!error?.context) return out;
  const root = error.context as FieldErrorContext;
  const bucket = (
    root.errors && typeof root.errors === "object" ? root.errors : root
  ) as FieldErrorContext;
  for (const field of fields) {
    const msg = pickFieldMessage(bucket[field]);
    if (msg) out[field] = msg;
  }
  return out;
}

const CREDENTIAL_STATUSES = new Set([401, 403, 404]);
const CREDENTIAL_TYPES = new Set([
  "AUTH_001",
  "AUTH_002",
  "AUTH_003",
  "AUTH_004",
  "AUTH_006",
  "AUTH_009",
]);
const RATE_LIMIT_TYPES = new Set(["AUTH_007", "SRV_004"]);
export const VERIFIED_ONLY_TYPES = new Set(["AUTH_008"]);

export function isVerifiedOnlyError(error: AuthErrorEnvelope | null): boolean {
  if (!error?.type) return false;
  return VERIFIED_ONLY_TYPES.has(error.type);
}

export function isTransportFailure(status: number): boolean {
  return status === 0 || status >= 500;
}

export function isRateLimited(error: AuthErrorEnvelope | null, status: number): boolean {
  if (status === 429) return true;
  return (
    (error?.type != null && RATE_LIMIT_TYPES.has(error.type)) ||
    typeof error?.retry_after === "number"
  );
}

export function classifyAuthError(
  status: number,
  error: AuthErrorEnvelope | null,
  fields: ReadonlyArray<string>,
): ClassifiedError {
  if (status === 0) {
    return { kind: "transport", fieldErrors: {}, message: NETWORK_ERROR };
  }
  if (isRateLimited(error, status)) {
    const seconds = typeof error?.retry_after === "number" ? error.retry_after : undefined;
    return {
      kind: "rate-limit",
      fieldErrors: {},
      message: seconds ? `Too many attempts. Please try again in ${seconds}s.` : RATE_LIMIT,
    };
  }
  if (status >= 500) {
    return { kind: "server", fieldErrors: {}, message: GENERIC_TRY_AGAIN };
  }
  const fieldErrors = extractFieldErrors(error, fields);
  const isValidationError = status === 400 && Object.keys(fieldErrors).length > 0;
  if (isValidationError) {
    const first = Object.values(fieldErrors)[0];
    return { kind: "field", fieldErrors, message: first ?? error?.message ?? GENERIC_TRY_AGAIN };
  }
  const type = error?.type;
  if (type && VERIFIED_ONLY_TYPES.has(type)) {
    return { kind: "verified-only", fieldErrors: {}, message: VERIFY_EMAIL_NEXT };
  }
  if (CREDENTIAL_STATUSES.has(status) || (type && CREDENTIAL_TYPES.has(type))) {
    return { kind: "credential", fieldErrors: {}, message: CREDENTIAL_FAILURE };
  }
  return {
    kind: "server",
    fieldErrors: {},
    message: error?.message ?? GENERIC_TRY_AGAIN,
  };
}
