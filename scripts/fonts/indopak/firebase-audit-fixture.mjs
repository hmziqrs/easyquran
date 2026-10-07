import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { root } from "./deep-browser-shared.mjs";

export async function installFirebaseAuditFixture(context) {
  const events = [];
  const source = await readFile(path.join(root, "web/src/lib/firebase/index.ts"), "utf8");
  const config = {
    appId: source.match(/appId:\s*"([^"]+)"/u)?.[1],
    measurementId: source.match(/measurementId:\s*"([^"]+)"/u)?.[1],
  };
  assert.ok(config.appId && config.measurementId);
  await context.route("https://www.googletagmanager.com/gtag/js**", (route) =>
    route.fulfill({ status: 200, contentType: "application/javascript", body: "" }),
  );
  for (const domain of ["firebase.googleapis.com", "firebaseinstallations.googleapis.com"])
    await context.route(`https://${domain}/**`, async (route) => {
      const request = route.request();
      const headers = await request.allHeaders();
      const responseHeaders = {
        "access-control-allow-origin": headers.origin ?? new URL(request.frame().url()).origin,
        "access-control-allow-credentials": "true",
        "access-control-allow-methods": "GET,POST,OPTIONS",
        "access-control-allow-headers":
          headers["access-control-request-headers"] ?? Object.keys(headers).join(","),
      };
      events.push({
        url: request.url(),
        method: request.method(),
        response_headers: responseHeaders,
      });
      if (request.method() === "OPTIONS") {
        await route.fulfill({ status: 204, headers: responseHeaders });
        return;
      }
      let body = config;
      if (domain === "firebaseinstallations.googleapis.com") {
        body = {
          fid: "cIndopakAuditFixture001",
          refreshToken: "audit-fixture-refresh",
          authToken: { token: "audit-fixture-token", expiresIn: "604800s" },
        };
      }
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        headers: responseHeaders,
        body: JSON.stringify(body),
      });
    });
  return events;
}
