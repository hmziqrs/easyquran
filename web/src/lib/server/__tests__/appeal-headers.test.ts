import { describe, expect, it } from "vite-plus/test";

import { applyHeaders } from "../../../../server";

const TOKEN = "b".repeat(64);

function htmlResponse(): Response {
  return new Response("<!doctype html>", {
    status: 200,
    headers: { "content-type": "text/html; charset=utf-8" },
  });
}

describe("appeal page response headers", () => {
  it("stamps noindex and no-referrer on the intake and token pages", () => {
    for (const path of ["/appeal", `/appeal/${TOKEN}`]) {
      const response = htmlResponse();
      applyHeaders(response, path);
      expect(response.headers.get("x-robots-tag")).toBe("noindex");
      expect(response.headers.get("referrer-policy")).toBe("no-referrer");
    }
  });

  it("leaves other pages on the site-wide referrer policy", () => {
    const response = htmlResponse();
    applyHeaders(response, "/bookmarks");
    expect(response.headers.get("referrer-policy")).toBe("strict-origin-when-cross-origin");
    expect(response.headers.get("x-robots-tag")).toBeNull();
  });
});
