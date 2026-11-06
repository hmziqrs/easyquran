import { describe, expect, it } from "vite-plus/test";

import { createConsent } from "../consent.svelte.js";

describe("consent for a minor account", () => {
  it("keeps analytics and ads off while the flag is set", () => {
    const c = createConsent();
    c.allowAll();
    expect(c.analytics).toBe(true);
    c.setMinorAccount(true);
    expect(c.analytics).toBe(false);
    expect(c.advertising).toBe(false);
    expect(c.consentSettings.analytics_storage).toBe("denied");
    c.setMinorAccount(false);
    expect(c.analytics).toBe(true);
  });
});
