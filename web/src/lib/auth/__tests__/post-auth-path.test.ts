import { describe, expect, it } from "vite-plus/test";

import type { UserProfile } from "#lib/auth/auth-client.js";
import { postAuthPath } from "#lib/auth/post-auth-path.js";

const PROFILE: UserProfile = {
  id: 1,
  name: "Alina",
  email: "alina@eq.test",
  avatar_id: null,
  is_verified: true,
  role: "user",
  two_fa_enabled: false,
  oauth_provider: null,
};

const verified: UserProfile = PROFILE;
const unverified: UserProfile = { ...PROFILE, id: 2, is_verified: false };

describe("postAuthPath", () => {
  it("unverified users finish verification first, even with a return target", () => {
    expect(postAuthPath(unverified, "/bookmarks")).toBe("/verify-email");
    expect(postAuthPath(unverified)).toBe("/verify-email");
  });

  it("verified users land on the return target", () => {
    expect(postAuthPath(verified, "/bookmarks?filter=all")).toBe("/bookmarks?filter=all");
    expect(postAuthPath(null, "/surah/1")).toBe("/surah/1");
  });

  it("falls back to /surah when there is no target", () => {
    expect(postAuthPath(verified)).toBe("/surah");
    expect(postAuthPath(verified, null)).toBe("/surah");
    expect(postAuthPath(verified, "")).toBe("/surah");
    expect(postAuthPath(null)).toBe("/surah");
  });
});
