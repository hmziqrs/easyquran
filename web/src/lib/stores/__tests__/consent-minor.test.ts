import { describe, expect, it } from "vite-plus/test";

import { createConsent } from "../consent.svelte.js";

describe("consent for a minor account", () => {
  it("keeps analytics and ads off while the flag is set", () => {
    const c = createConsent();
    c.markAuthResolved();
    c.allowAll();
    expect(c.analytics).toBe(true);
    expect(c.analyticsAllowed).toBe(true);
    c.setMinorAccount(true);
    expect(c.analytics).toBe(false);
    expect(c.analyticsAllowed).toBe(false);
    expect(c.advertising).toBe(false);
    expect(c.consentSettings.analytics_storage).toBe("denied");
    c.setMinorAccount(false);
    expect(c.analytics).toBe(true);
    expect(c.analyticsAllowed).toBe(true);
  });
});

describe("consent while auth status is unknown (A34)", () => {
  it("holds analytics and ads until auth is resolved", () => {
    const c = createConsent();
    c.allowAll();
    // The stored preference still shows for the Settings toggle...
    expect(c.analytics).toBe(true);
    // ...but nothing may leave the device before the status is known.
    expect(c.analyticsAllowed).toBe(false);
    expect(c.consentSettings.analytics_storage).toBe("denied");
    expect(c.consentSettings.ad_storage).toBe("denied");
    c.markAuthResolved();
    expect(c.analyticsAllowed).toBe(true);
    expect(c.consentSettings.analytics_storage).toBe("granted");
  });

  it("stays resolved once set, so a later unknown status does not re-hold analytics", () => {
    const c = createConsent();
    c.allowAll();
    c.markAuthResolved();
    c.markAuthResolved();
    expect(c.analyticsAllowed).toBe(true);
  });

  it("a minor stays held even after auth resolves", () => {
    const c = createConsent();
    c.allowAll();
    c.setMinorAccount(true);
    c.markAuthResolved();
    expect(c.analyticsAllowed).toBe(false);
    expect(c.consentSettings.analytics_storage).toBe("denied");
  });
});
