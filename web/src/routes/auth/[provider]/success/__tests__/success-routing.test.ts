import { describe, expect, it } from "vite-plus/test";

import { successDestination, type SuccessRouteUser } from "../success-destination";

const VERIFIED: SuccessRouteUser = { is_verified: true };
const UNVERIFIED: SuccessRouteUser = { is_verified: false };

describe("OAuth success route destination decision", () => {
  it("ok + verified user + returnTarget -> goto(returnTarget)", () => {
    expect(successDestination("/surah/2/255", VERIFIED)).toBe("/surah/2/255");
  });

  it("ok + unverified user + no returnTarget -> goto('/verify-email')", () => {
    expect(successDestination(null, UNVERIFIED)).toBe("/verify-email");
  });

  it("ok + verified user + no returnTarget -> goto('/surah')", () => {
    expect(successDestination(null, VERIFIED)).toBe("/surah");
  });

  it("returnTarget takes precedence over the unverified fallback", () => {
    expect(successDestination("/settings", UNVERIFIED)).toBe("/settings");
  });

  it("no user resolved + no returnTarget -> goto('/surah') (unverified defaults false)", () => {
    expect(successDestination(null, null)).toBe("/surah");
  });

  it("returnTarget used verbatim once; decision is pure (consume lives in return-target)", () => {
    const dest = successDestination("/yours", VERIFIED);
    expect(dest).toBe("/yours");
    expect(successDestination("/yours", VERIFIED)).toBe(dest);
  });
});
