import assert from "node:assert/strict";
import { Readable } from "node:stream";
import test from "node:test";

import { PutObjectCommand, S3Client } from "@aws-sdk/client-s3";

import {
  buildCatalogBody,
  listExisting,
  putObjectInput,
  type UploadItem,
  uploadItems,
} from "./upload.ts";

interface CapturedRequest {
  readonly body?: { destroy?: () => void };
  readonly headers: Record<string, string>;
}

function item(mutable: boolean): UploadItem {
  return {
    absolutePath: null,
    generatedBody: Buffer.from("abc"),
    key: mutable ? "tanzil/translations/index.min.json" : "tanzil/test.sqlite",
    sizeBytes: 3,
    contentType: "application/octet-stream",
    cacheControl: "public",
    mutable,
    sqlite: false,
  };
}

test("catalogue is generated deterministically from the complete baked map", () => {
  const first = buildCatalogBody();
  const second = buildCatalogBody();
  const catalogue = JSON.parse(first.toString("utf8")) as unknown[];

  assert.deepEqual(first, second);
  assert.equal(catalogue.length, 134);
  assert.equal(first.at(-1), 10);
});

test("immutable puts are conditional and mutable catalogue puts are not", () => {
  const immutable = putObjectInput(item(false), Readable.from([Buffer.from("abc")]));
  const mutable = putObjectInput(item(true), Readable.from([Buffer.from("abc")]));

  assert.equal(immutable.IfNoneMatch, "*");
  assert.equal(mutable.IfNoneMatch, undefined);
  assert.equal(immutable.ContentLength, 3);
});

test("stream upload signing uses an unsigned payload and no checksum headers", async () => {
  let captured: CapturedRequest | undefined;
  const requestHandler = {
    handle: async (request: CapturedRequest) => {
      captured = request;
      request.body?.destroy?.();
      return {
        response: { statusCode: 200, headers: {}, body: new Uint8Array() },
      };
    },
    updateHttpClientConfig(): void {},
    httpHandlerConfigs(): Record<string, never> {
      return {};
    },
  };
  const client = new S3Client({
    region: "auto",
    endpoint: "https://example.r2.cloudflarestorage.com",
    credentials: { accessKeyId: "test", secretAccessKey: "test" },
    requestChecksumCalculation: "WHEN_REQUIRED",
    responseChecksumValidation: "WHEN_REQUIRED",
    maxAttempts: 1,
    requestHandler: requestHandler as never,
  });

  try {
    const body = Readable.from([Buffer.from("abc")]);
    await client.send(new PutObjectCommand(putObjectInput(item(false), body)));
  } finally {
    client.destroy();
  }

  assert.ok(captured);
  assert.equal(captured.headers["x-amz-content-sha256"], "UNSIGNED-PAYLOAD");
  assert.equal(captured.headers["if-none-match"], "*");
  assert.equal(captured.headers["content-length"], "3");
  assert.equal(
    Object.keys(captured.headers).some((header) => header.startsWith("x-amz-checksum-")),
    false,
  );
});

test("R2 pagination fails closed on missing or repeated continuation tokens", async () => {
  const missingTokenClient = {
    send: async () => ({ IsTruncated: true }),
  };
  await assert.rejects(
    listExisting(missingTokenClient as never),
    /truncated without a continuation token/,
  );

  let call = 0;
  const repeatedTokenClient = {
    send: async () => {
      call += 1;
      return { IsTruncated: true, NextContinuationToken: "same" };
    },
  };
  await assert.rejects(listExisting(repeatedTokenClient as never), /repeated a continuation token/);
  assert.equal(call, 2);
});

test("R2 inventory rejects missing object sizes", async () => {
  const client = {
    send: async () => ({ IsTruncated: false, Contents: [{ Key: "tanzil/bad.sqlite" }] }),
  };
  await assert.rejects(listExisting(client as never), /invalid object size/);
});

test("catalogue upload is withheld when an immutable artifact fails", async () => {
  const attempted: string[] = [];
  const client = {
    send: async (command: { input?: { Key?: string } }) => {
      const key = command.input?.Key;
      if (!key) return { IsTruncated: false, Contents: [] };
      attempted.push(key);
      throw new Error("simulated immutable failure");
    },
  };
  const immutable = item(false);
  const catalogue = item(true);
  const result = await uploadItems(client as never, [immutable, catalogue]);

  assert.deepEqual(attempted, [immutable.key]);
  assert.equal(result.uploaded, 0);
  assert.equal(result.failures.length, 2);
  assert.match(result.failures.join("\n"), /not uploaded because an immutable artifact failed/);
});
