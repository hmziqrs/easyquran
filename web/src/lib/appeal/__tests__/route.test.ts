import { describe, expect, it } from "vite-plus/test";

import { APPEAL_HEADERS, appealTokenFromInput, isAppealPath } from "../route";

const TOKEN = "a".repeat(64);

describe("isAppealPath", () => {
  it("matches the intake page and its token pages only", () => {
    expect(isAppealPath("/appeal")).toBe(true);
    expect(isAppealPath(`/appeal/${TOKEN}`)).toBe(true);
    expect(isAppealPath("/appealing")).toBe(false);
    expect(isAppealPath("/account")).toBe(false);
  });
});

describe("appealTokenFromInput", () => {
  it("accepts a bare token and the full link from the notice email", () => {
    expect(appealTokenFromInput(TOKEN)).toBe(TOKEN);
    expect(appealTokenFromInput(`  https://example.test/appeal/${TOKEN}  `)).toBe(TOKEN);
    expect(appealTokenFromInput(`https://example.test/appeal/${TOKEN}?x=1`)).toBe(TOKEN);
  });

  it("refuses anything that is not 64 lowercase hex characters", () => {
    expect(appealTokenFromInput("")).toBeNull();
    expect(appealTokenFromInput("not-a-token")).toBeNull();
    expect(appealTokenFromInput("A".repeat(64))).toBeNull();
    expect(appealTokenFromInput("a".repeat(63))).toBeNull();
  });
});

describe("APPEAL_HEADERS", () => {
  it("asks for no indexing and no referrer", () => {
    expect(APPEAL_HEADERS).toEqual({
      "X-Robots-Tag": "noindex",
      "Referrer-Policy": "no-referrer",
    });
  });
});
