import { describe, expect, it } from "vite-plus/test";

import { buildReportPayload, REPORT_DETAILS_MAX, type ReportFormInput } from "../payload";

const base: ReportFormInput = {
  targetKind: "post",
  targetId: "12",
  reason: "spam",
  details: "",
  contactEmail: "",
};

describe("buildReportPayload", () => {
  it("omits blank optional fields", () => {
    const built = buildReportPayload(base);
    expect(built).toEqual({
      ok: true,
      body: { target_kind: "post", target_id: "12", reason: "spam" },
    });
  });

  it("trims details and lowercases the contact address", () => {
    const built = buildReportPayload({
      ...base,
      details: "  broken verse  ",
      contactEmail: "  Reader@Example.COM ",
    });
    expect(built).toEqual({
      ok: true,
      body: {
        target_kind: "post",
        target_id: "12",
        reason: "spam",
        details: "broken verse",
        contact_email: "reader@example.com",
      },
    });
  });

  it("refuses ids that are not positive integers", () => {
    for (const targetId of ["", "0", "abc", "-3", "1.5", "12345678901"]) {
      expect(buildReportPayload({ ...base, targetId })).toEqual({
        ok: false,
        error: "invalid_target",
      });
    }
  });

  it("refuses details over the server limit", () => {
    const details = "x".repeat(REPORT_DETAILS_MAX + 1);
    expect(buildReportPayload({ ...base, details })).toEqual({
      ok: false,
      error: "details_too_long",
    });
  });

  it("accepts details at exactly the limit", () => {
    const details = "x".repeat(REPORT_DETAILS_MAX);
    expect(buildReportPayload({ ...base, details }).ok).toBe(true);
  });

  it("sends a translation target as source id and verse key", () => {
    const built = buildReportPayload({
      ...base,
      targetKind: "translation",
      targetId: "ar.jalalayn:2:255",
      reason: "other",
    });
    expect(built).toEqual({
      ok: true,
      body: {
        target_kind: "translation",
        target_id: "ar.jalalayn:2:255",
        reason: "other",
      },
    });
  });

  it("refuses translation targets outside the surah and ayah bounds", () => {
    for (const targetId of [
      "ar.jalalayn",
      "ar.jalalayn:0:1",
      "ar.jalalayn:115:1",
      "ar.jalalayn:2:0",
      "ar.jalalayn:2:287",
      "ar.jalalayn:2:255:1",
      "2:255",
      "AR.jalalayn:2:255",
    ]) {
      expect(buildReportPayload({ ...base, targetKind: "translation", targetId })).toEqual({
        ok: false,
        error: "invalid_target",
      });
    }
  });
});
