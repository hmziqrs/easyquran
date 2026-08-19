import { createHash } from "node:crypto";

const scriptPattern = /<script>([\s\S]*?)<\/script>/gu;
const staticCspPattern = /^[ \t]*Content-Security-Policy:[ \t]*(.+)$/mu;

export function scriptHashTokens(html: string): string[] {
  const tokens: string[] = [];
  for (const match of html.matchAll(scriptPattern)) {
    tokens.push(
      `'sha256-${createHash("sha256")
        .update(match[1] ?? "")
        .digest("base64")}'`,
    );
  }
  return tokens;
}

export function parseStaticCsp(headersFile: string): string | undefined {
  const value = staticCspPattern.exec(headersFile)?.[1];
  return value?.trim();
}

export function cspWithScriptTokens(csp: string, tokens: readonly string[]): string {
  if (tokens.length === 0) return csp;
  return csp.replace("script-src 'self'", `script-src 'self' ${tokens.join(" ")}`);
}

export function pageCandidates(pathname: string): string[] {
  if (pathname.endsWith(".html")) return [pathname];
  if (pathname.endsWith("/")) return [`${pathname}index.html`];
  return [`${pathname}.html`, `${pathname}/index.html`];
}
