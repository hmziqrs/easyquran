import { afterEach, describe, expect, it, vi } from "vite-plus/test";

import {
  STORAGE_STATS,
  STORAGE_STATS_ACK,
  VERSION_QUERY,
  VERSION_RESULT,
  requestStorageStats,
  requestWorkerVersion,
} from "$lib/offline/messages";

interface FakePort {
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- FakePort mirrors the MessagePort surface the SUT exercises; postMessage payloads are the same untyped SW acks requestStorageStats validates, heterogeneous by design
  postMessage: (msg: unknown) => void;
  close(): void;
  onmessage: ((ev: { data: unknown }) => void) | null;
  start(): void;
}

function installFakeMessageChannel(): void {
  vi.stubGlobal("MessageChannel", function MessageChannel() {
    const port1: FakePort = {
      postMessage: () => {},
      close: () => {},
      onmessage: null,
      start: () => {},
    };
    const port2: FakePort = {
      // eslint-disable-next-line anti-slop/no-unknown-parameters -- the fake port forwards raw ack payloads verbatim; the SUT's decoder is the parser under test, so the double stays opaque here
      postMessage: (msg: unknown) => {
        port1.onmessage?.({ data: msg });
      },
      close: () => {},
      onmessage: null,
      start: () => {},
    };
    return { port1, port2 };
  });
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- stubs navigator.serviceWorker.controller for varied fake-controller shapes (postMessage arity differs per test); the SUT reads it via the real ServiceWorker type, not this local annotation.
function stubController(controller: unknown): void {
  vi.stubGlobal("navigator", { serviceWorker: { controller } });
}

function flush(ms = 10): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("requestStorageStats client helper", () => {
  it("posts STORAGE_STATS on the controller with a transferred port and resolves the decoded ack", async () => {
    installFakeMessageChannel();
    let sent: { msg: { type: string }; port: FakePort | null } | null = null;
    const controller = {
      postMessage(msg: { type: string }, transfer: unknown[]): void {
        // SAFETY: requestStorageStats transfers port2 from installFakeMessageChannel, which is a FakePort instance in this test.
        sent = { msg, port: (transfer[0] as FakePort) ?? null };
      },
    };
    stubController(controller);

    const promise = requestStorageStats(5000);
    await flush(5);

    expect(sent).not.toBeNull();
    expect(sent!.msg.type).toBe(STORAGE_STATS);
    expect(sent!.port).not.toBeNull();

    sent!.port!.postMessage({
      type: STORAGE_STATS_ACK,
      pages: { entries: 2, bytes: 128 },
      data: { entries: 5, bytes: 512 },
    });

    await expect(promise).resolves.toEqual({
      pages: { entries: 2, bytes: 128 },
      data: { entries: 5, bytes: 512 },
    });
  });

  it("ignores acks with a wrong type or invalid layer stats, then resolves on a valid ack", async () => {
    installFakeMessageChannel();
    let sent: { msg: { type: string }; port: FakePort | null } | null = null;
    stubController({
      postMessage(msg: { type: string }, transfer: unknown[]): void {
        // SAFETY: requestStorageStats transfers port2 from installFakeMessageChannel, which is a FakePort instance in this test.
        sent = { msg, port: (transfer[0] as FakePort) ?? null };
      },
    });

    const promise = requestStorageStats(1000);
    await flush(5);

    const port = sent!.port!;
    port.postMessage({ type: "UNRELATED" });
    port.postMessage({
      type: STORAGE_STATS_ACK,
      pages: { entries: "two", bytes: 128 },
      data: { entries: 5, bytes: 512 },
    });
    port.postMessage({
      type: STORAGE_STATS_ACK,
      pages: { entries: 2, bytes: Number.NaN },
      data: { entries: 5, bytes: 512 },
    });
    port.postMessage({ type: STORAGE_STATS_ACK, pages: { entries: 2, bytes: 128 } });
    port.postMessage("not an object");
    port.postMessage({
      type: STORAGE_STATS_ACK,
      pages: { entries: 2, bytes: 128 },
      data: { entries: 5, bytes: 512 },
    });

    const stats = await promise;
    expect(stats).toEqual({ pages: { entries: 2, bytes: 128 }, data: { entries: 5, bytes: 512 } });
  });

  it("resolves null on its own when the worker never acks (timeout fallback)", async () => {
    installFakeMessageChannel();
    let posted = false;
    stubController({
      postMessage(): void {
        posted = true;
      },
    });

    await expect(requestStorageStats(20)).resolves.toBeNull();
    expect(posted).toBe(true);
  });

  it("resolves null without posting when no controller is attached", async () => {
    installFakeMessageChannel();
    let posted = false;
    stubController({
      postMessage(): void {
        posted = true;
      },
    });
    vi.stubGlobal("navigator", { serviceWorker: { controller: null } });

    await expect(requestStorageStats(20)).resolves.toBeNull();
    expect(posted).toBe(false);
  });

  it("resolves null when navigator.serviceWorker is undefined", async () => {
    vi.stubGlobal("navigator", {});
    await expect(requestStorageStats(20)).resolves.toBeNull();
  });
});

describe("version handshake protocol constants", () => {
  it("keeps the VERSION_QUERY / VERSION_RESULT wire names stable", () => {
    expect(VERSION_QUERY).toBe("VERSION_QUERY");
    expect(VERSION_RESULT).toBe("VERSION_RESULT");
  });
});

describe("requestWorkerVersion client helper", () => {
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- fake worker double; the SUT's postMessage call is the I/O boundary and this test drives it directly
  function asWorker(fake: unknown): ServiceWorker {
    // SAFETY: the fake implements exactly the postMessage(message, transfer) surface requestWorkerVersion exercises; ServiceWorker is a DOM interface whose full member/overload set no single assertion could satisfy structurally.
    // eslint-disable-next-line anti-slop/no-chained-type-assertions, anti-slop/require-safety-comment-for-type-assertion -- widening the minimal fake through unknown because the full ServiceWorker DOM interface cannot be replicated by the double
    return fake as unknown as ServiceWorker;
  }

  it("posts VERSION_QUERY with a transferred port and resolves the answered version", async () => {
    installFakeMessageChannel();
    let sent: { msg: { type: string }; port: FakePort | null } | null = null;
    const worker = {
      postMessage(msg: { type: string }, transfer: unknown[]): void {
        // SAFETY: requestWorkerVersion transfers port2 from installFakeMessageChannel, which is a FakePort instance in this test.
        sent = { msg, port: (transfer[0] as FakePort) ?? null };
      },
    };

    const promise = requestWorkerVersion(asWorker(worker));
    await flush(5);

    expect(sent).not.toBeNull();
    expect(sent!.msg.type).toBe(VERSION_QUERY);
    expect(sent!.port).not.toBeNull();

    sent!.port!.postMessage({ type: VERSION_RESULT, version: "test-v1" });

    await expect(promise).resolves.toBe("test-v1");
  });

  it("ignores wrong-type and malformed answers, then resolves on a valid VERSION_RESULT", async () => {
    installFakeMessageChannel();
    let sent: { msg: { type: string }; port: FakePort | null } | null = null;
    const worker = {
      postMessage(msg: { type: string }, transfer: unknown[]): void {
        // SAFETY: requestWorkerVersion transfers port2 from installFakeMessageChannel, which is a FakePort instance in this test.
        sent = { msg, port: (transfer[0] as FakePort) ?? null };
      },
    };

    const promise = requestWorkerVersion(asWorker(worker));
    await flush(5);

    const port = sent!.port!;
    port.postMessage({ type: "UNRELATED", version: "nope" });
    port.postMessage({ type: VERSION_RESULT });
    port.postMessage({ type: VERSION_RESULT, version: "" });
    port.postMessage({ type: VERSION_RESULT, version: 42 });
    port.postMessage("not an object");
    port.postMessage({ type: VERSION_RESULT, version: "test-v2" });

    await expect(promise).resolves.toBe("test-v2");
  });

  it("resolves null when the worker never answers (timeout fallback)", async () => {
    installFakeMessageChannel();
    let posted = false;
    const worker = {
      postMessage(): void {
        posted = true;
      },
    };

    await expect(requestWorkerVersion(asWorker(worker), 20)).resolves.toBeNull();
    expect(posted).toBe(true);
  });

  it("resolves null without posting when the target worker is missing", async () => {
    await expect(requestWorkerVersion(null, 20)).resolves.toBeNull();
    await expect(requestWorkerVersion(undefined, 20)).resolves.toBeNull();
  });

  it("resolves null when postMessage throws on the target worker", async () => {
    installFakeMessageChannel();
    const worker = {
      postMessage(): void {
        throw new Error("target gone");
      },
    };

    await expect(requestWorkerVersion(asWorker(worker), 20)).resolves.toBeNull();
  });
});
