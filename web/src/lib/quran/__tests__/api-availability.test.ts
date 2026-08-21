import { QuranApiAvailability, QuranApiUnavailableError } from "$lib/quran/api-availability";
import { FetchHttpError, FetchTimeoutError, MalformedDataError } from "$lib/quran/fetch";
import { describe, expect, it, vi } from "vite-plus/test";

describe("QuranApiAvailability", () => {
  it("opens immediately after transport failure or timeout and skips requests during cooldown", async () => {
    const availability = new QuranApiAvailability({ cooldownMs: 100 });
    const transport = vi.fn().mockRejectedValue(new Error("offline"));
    await expect(availability.run(transport)).rejects.toThrow("offline");

    const skipped = vi.fn().mockResolvedValue("unexpected");
    await expect(availability.run(skipped)).rejects.toBeInstanceOf(QuranApiUnavailableError);
    expect(skipped).not.toHaveBeenCalled();

    availability.reset();
    await expect(
      availability.run(() => Promise.reject(new FetchTimeoutError())),
    ).rejects.toBeInstanceOf(FetchTimeoutError);
    await expect(availability.run(skipped)).rejects.toBeInstanceOf(QuranApiUnavailableError);
  });

  it("requires repeated 5xx failures but never opens for 4xx or malformed data", async () => {
    const availability = new QuranApiAvailability({ serverFailureThreshold: 2 });
    const serviceFailure = (): Promise<never> => Promise.reject(new FetchHttpError(503));

    await expect(availability.run(serviceFailure)).rejects.toBeInstanceOf(FetchHttpError);
    const ordinaryRequest = vi.fn().mockRejectedValue(new FetchHttpError(404));
    await expect(availability.run(ordinaryRequest)).rejects.toBeInstanceOf(FetchHttpError);
    expect(ordinaryRequest).toHaveBeenCalledTimes(1);

    await expect(availability.run(serviceFailure)).rejects.toBeInstanceOf(FetchHttpError);
    const malformed = vi.fn().mockRejectedValue(new MalformedDataError());
    await expect(availability.run(malformed)).rejects.toBeInstanceOf(MalformedDataError);
    expect(malformed).toHaveBeenCalledTimes(1);

    await expect(availability.run(serviceFailure)).rejects.toBeInstanceOf(FetchHttpError);
    await expect(availability.run(serviceFailure)).rejects.toBeInstanceOf(FetchHttpError);
    const skipped = vi.fn().mockResolvedValue("unexpected");
    await expect(availability.run(skipped)).rejects.toBeInstanceOf(QuranApiUnavailableError);
    expect(skipped).not.toHaveBeenCalled();
  });

  it("allows one half-open real request and coalesces competing probes", async () => {
    let now = 0;
    const availability = new QuranApiAvailability({ cooldownMs: 100, now: () => now });
    await expect(availability.run(() => Promise.reject(new Error("offline")))).rejects.toThrow(
      "offline",
    );

    now = 100;
    let resolveProbe: ((value: string) => void) | undefined;
    const probe = availability.run(
      () =>
        new Promise<string>((resolve) => {
          resolveProbe = resolve;
        }),
    );
    const competing = vi.fn().mockResolvedValue("duplicate");
    await expect(availability.run(competing)).rejects.toBeInstanceOf(QuranApiUnavailableError);
    expect(competing).not.toHaveBeenCalled();

    resolveProbe?.("healthy");
    await expect(probe).resolves.toBe("healthy");
    await expect(availability.run(() => Promise.resolve("next"))).resolves.toBe("next");
  });

  it("does not mark caller aborts as outages", async () => {
    const availability = new QuranApiAvailability();
    const controller = new AbortController();
    controller.abort();
    await expect(
      availability.run(
        () => Promise.reject(new DOMException("aborted", "AbortError")),
        controller.signal,
      ),
    ).rejects.toMatchObject({ name: "AbortError" });

    await expect(availability.run(() => Promise.resolve("healthy"))).resolves.toBe("healthy");
  });
});
