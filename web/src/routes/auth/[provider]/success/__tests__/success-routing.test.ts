import { describe, expect, it } from "vite-plus/test";

// Mirrors routes/auth/[provider]/success/+page.svelte complete() routing decision.
export interface SuccessRouteUser {
  readonly is_verified: boolean;
}

export function successDestination(
  returnTarget: string | null,
  user: SuccessRouteUser | null,
): string {
  const unverified = user ? !user.is_verified : false;
  return returnTarget ?? (unverified ? "/verify-email" : "/app");
}

const VERIFIED: SuccessRouteUser = { is_verified: true };
const UNVERIFIED: SuccessRouteUser = { is_verified: false };

describe("OAuth success route destination decision", () => {
  it("ok + verified user + returnTarget -> goto(returnTarget)", () => {
    expect(successDestination("/surah/2/255", VERIFIED)).toBe("/surah/2/255");
  });

  it("ok + unverified user + no returnTarget -> goto('/verify-email')", () => {
    expect(successDestination(null, UNVERIFIED)).toBe("/verify-email");
  });

  it("ok + verified user + no returnTarget -> goto('/app')", () => {
    expect(successDestination(null, VERIFIED)).toBe("/app");
  });

  it("returnTarget takes precedence over the unverified fallback", () => {
    expect(successDestination("/app/settings", UNVERIFIED)).toBe("/app/settings");
  });

  it("no user resolved + no returnTarget -> goto('/app') (unverified defaults false)", () => {
    expect(successDestination(null, null)).toBe("/app");
  });

  it("returnTarget used verbatim once; decision is pure (consume lives in return-target)", () => {
    const dest = successDestination("/app/library/last", VERIFIED);
    expect(dest).toBe("/app/library/last");
    expect(successDestination("/app/library/last", VERIFIED)).toBe(dest);
  });
});
