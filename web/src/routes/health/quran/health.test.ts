import { describe, expect, it } from "vite-plus/test";

import { readinessResponse } from "./readiness";

describe("web quran health readiness", () => {
  it("is ready (200, no-store) only when the manifest exists and the cache is writable", async () => {
    const response = readinessResponse(true, true);

    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ ready: true });
    expect(response.headers.get("cache-control")).toBe("no-store");
  });

  it("is not ready (503) when the build manifest is missing", async () => {
    const response = readinessResponse(false, true);

    expect(response.status).toBe(503);
    expect(await response.json()).toEqual({ ready: false });
  });

  it("is not ready (503) when the disk cache is not writable", async () => {
    const response = readinessResponse(true, false);

    expect(response.status).toBe(503);
    expect(await response.json()).toEqual({ ready: false });
  });
});
