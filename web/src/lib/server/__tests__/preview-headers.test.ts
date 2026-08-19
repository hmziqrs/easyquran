import {
  cspWithScriptTokens,
  pageCandidates,
  parseStaticCsp,
  scriptHashTokens,
} from "$lib/server/preview-headers";
import { describe, expect, it } from "vite-plus/test";

const CSP = "default-src 'self'; script-src 'self' https://example.com; object-src 'none'";

const SHA_A = "'sha256-OVLEFZR89oYYkV3AI9/M2qwUUdT/ymKO659RpVAhzEk='";
const SHA_THEME = "'sha256-CzHvFNhLvXK4TfIMNWD3vTOcY6SiqnWiqQTttD8arGk='";

describe("scriptHashTokens", () => {
  it("hashes each plain inline script body, in document order", () => {
    const html =
      "<html><head><script>a();</script></head><body><script>theme();</script></body></html>";
    expect(scriptHashTokens(html)).toEqual([SHA_A, SHA_THEME]);
  });

  it("ignores scripts carrying attributes (nonce/type/src)", () => {
    const html = '<script type="module">a();</script><script src="/x.js"></script>';
    expect(scriptHashTokens(html)).toEqual([]);
  });
});

describe("parseStaticCsp", () => {
  it("extracts the CSP from a generated _headers file", () => {
    const headersFile = [
      "# comment line",
      "/*",
      "  Content-Security-Policy: default-src 'self'; script-src 'self' https://a",
      "  Strict-Transport-Security: max-age=31536000; includeSubDomains",
      "",
    ].join("\n");
    expect(parseStaticCsp(headersFile)).toBe("default-src 'self'; script-src 'self' https://a");
  });

  it("returns undefined when the file carries no CSP", () => {
    expect(parseStaticCsp("/*\n  Cache-Control: no-cache\n")).toBeUndefined();
  });
});

describe("cspWithScriptTokens", () => {
  it("inserts page script hashes right after the script-src origin", () => {
    expect(cspWithScriptTokens(CSP, [SHA_A, SHA_THEME])).toBe(
      "default-src 'self'; script-src 'self' " +
        `${SHA_A} ${SHA_THEME} https://example.com; object-src 'none'`,
    );
  });

  it("keeps the policy byte-identical when there are no page hashes", () => {
    expect(cspWithScriptTokens(CSP, [])).toBe(CSP);
  });
});

describe("pageCandidates", () => {
  it("mirrors kit's prerendered-page file resolution", () => {
    expect(pageCandidates("/app")).toEqual(["/app.html", "/app/index.html"]);
    expect(pageCandidates("/app/")).toEqual(["/app/index.html"]);
    expect(pageCandidates("/")).toEqual(["/index.html"]);
    expect(pageCandidates("/app.html")).toEqual(["/app.html"]);
  });
});
