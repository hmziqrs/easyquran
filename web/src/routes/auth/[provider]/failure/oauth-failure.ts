export const OAUTH_FAILURE_MESSAGE =
  "We couldn't complete sign-in. Please try again or choose another method.";
export const OAUTH_FAILURE_HEADING = "Sign-in didn't complete";

export function failureMessage(_errorCode: string | null | undefined): string {
  return OAUTH_FAILURE_MESSAGE;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- exported type guard; callers pass raw URL/error payloads whose only contract is "any value" (see failure __tests__).
export function isOpaqueFailureCode(v: unknown): v is string {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- string-ness plus the regex below IS the whole contract; no schema exists upstream of this guard.
  if (typeof v !== "string") return false;
  if (v.length === 0 || v.length > 48) return false;
  return /^[a-z][a-z0-9_]*$/.test(v);
}
