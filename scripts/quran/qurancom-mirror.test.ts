import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  assertAssetSize,
  assertMirrorTargetPath,
  mirrorAbsolutePath,
  mirrorStaticPath,
  QURANCOM_ASSETS,
  QURANCOM_MIRROR_ROOT,
  QURANCOM_STATIC_PREFIX,
} from "./qurancom-assets.ts";

const DOCS_PATH = fileURLToPath(new URL("../../docs/qurancom-mirror.md", import.meta.url));
const LICENSE_STATUSES = ["cleared", "restricted", "unknown"];

test("mirror asset ids are unique", () => {
  const ids = QURANCOM_ASSETS.map((asset) => asset.id);
  assert.equal(new Set(ids).size, ids.length);
});

test("every mirror entry carries provenance, license, and stays production-disabled", () => {
  for (const asset of QURANCOM_ASSETS) {
    assert.ok(asset.provenance.trim().length > 0, `${asset.id}: missing provenance`);
    assert.ok(asset.license.name.trim().length > 0, `${asset.id}: missing license name`);
    assert.ok(
      LICENSE_STATUSES.includes(asset.license.status),
      `${asset.id}: invalid license status ${asset.license.status}`,
    );
    assert.equal(asset.productionEnabled, false, `${asset.id}: must be production-disabled`);
    assert.ok(asset.sizeBytes > 0, `${asset.id}: sizeBytes not baked`);
    assert.match(asset.url, /^https:\/\//, `${asset.id}: url must be https`);
  }
});

test("every target path resolves under web/static/qurancom/", () => {
  for (const asset of QURANCOM_ASSETS) {
    assert.doesNotThrow(() => assertMirrorTargetPath(asset), asset.id);
    assert.ok(mirrorStaticPath(asset).startsWith(QURANCOM_STATIC_PREFIX), asset.id);
    const absolute = mirrorAbsolutePath(asset);
    assert.ok(
      absolute.startsWith(`${QURANCOM_MIRROR_ROOT}${path.sep}`),
      `${asset.id}: ${absolute}`,
    );
  }
});

test("target path guards reject traversal and non-canonical paths", () => {
  assert.throws(() => mirrorAbsolutePath({ id: "bad", targetPath: "../escape" }), /escapes/);
  assert.throws(() => mirrorAbsolutePath({ id: "bad", targetPath: "a/../b" }), /invalid/);
  assert.throws(() => mirrorAbsolutePath({ id: "bad", targetPath: "/etc/passwd" }), /invalid/);
});

test("size assertions compare baked sizes and skip unrecorded entries", () => {
  assert.doesNotThrow(() => assertAssetSize({ id: "x", sizeBytes: 3 }, 3));
  assert.doesNotThrow(() => assertAssetSize({ id: "x", sizeBytes: 0 }, 999));
  assert.throws(
    () => assertAssetSize({ id: "x", sizeBytes: 3 }, 4),
    /size mismatch for x: expected 3, actual 4/,
  );
});

test("register doc mentions every restricted asset id and every catalogue id", async () => {
  const doc = await readFile(DOCS_PATH, "utf8");
  for (const asset of QURANCOM_ASSETS) {
    assert.ok(doc.includes(asset.id), `docs/qurancom-mirror.md missing ${asset.id}`);
  }
  const restricted = QURANCOM_ASSETS.filter((asset) => asset.license.status === "restricted");
  assert.ok(restricted.length > 0, "expected at least one restricted asset");
  for (const asset of restricted) {
    assert.ok(doc.includes(asset.id), `docs/qurancom-mirror.md missing restricted ${asset.id}`);
  }
});
