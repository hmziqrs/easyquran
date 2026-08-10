export const OAUTH_FAILURE_MESSAGE =
  "We couldn't complete sign-in. Please try again or choose another method.";
export const OAUTH_FAILURE_HEADING = "Sign-in didn't complete";

export function failureMessage(_errorCode: string | null | undefined): string {
  return OAUTH_FAILURE_MESSAGE;
}

export function isOpaqueFailureCode(v: unknown): v is string {
  if (typeof v !== "string") return false;
  if (v.length === 0 || v.length > 48) return false;
  return /^[a-z][a-z0-9_]*$/.test(v);
}
