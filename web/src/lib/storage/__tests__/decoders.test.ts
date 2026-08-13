import { describe, it, expect } from "vite-plus/test";

import {
  asBooleanRecord,
  asLiteral,
  asNullableObject,
  asNumber,
  asObject,
  asString,
  asStringRecord,
} from "../decoders";

describe("asObject", () => {
  it("returns the object for plain objects", () => {
    expect(asObject({ a: 1 })).toEqual({ a: 1 });
  });
  it("returns undefined for arrays, null, and primitives", () => {
    expect(asObject([1, 2])).toBeUndefined();
    expect(asObject(null)).toBeUndefined();
    expect(asObject("x")).toBeUndefined();
    expect(asObject(3)).toBeUndefined();
    expect(asObject(undefined)).toBeUndefined();
  });
});

describe("asNumber", () => {
  it("keeps finite numbers within range", () => {
    expect(asNumber(5, 1, 10)).toBe(5);
    expect(asNumber(1, 1, 10)).toBe(1);
    expect(asNumber(10, 1, 10)).toBe(10);
  });
  it("rejects out-of-range, non-finite, and non-numbers", () => {
    expect(asNumber(0, 1, 10)).toBeUndefined();
    expect(asNumber(11, 1, 10)).toBeUndefined();
    expect(asNumber(Number.NaN, 1, 10)).toBeUndefined();
    expect(asNumber(Infinity, 1, 10)).toBeUndefined();
    expect(asNumber("3", 1, 10)).toBeUndefined();
  });
  it("accepts any finite number with ±Infinity bounds", () => {
    expect(asNumber(-1000, -Infinity, Infinity)).toBe(-1000);
    expect(asNumber(0, -Infinity, Infinity)).toBe(0);
  });
});

describe("asString / asLiteral", () => {
  it("asString keeps strings only", () => {
    expect(asString("hi")).toBe("hi");
    expect(asString(3)).toBeUndefined();
    expect(asString(null)).toBeUndefined();
  });
  it("asLiteral narrows to the allowed set", () => {
    const modes = ["verse", "reading"] as const;
    expect(asLiteral("verse", modes)).toBe("verse");
    expect(asLiteral("reading", modes)).toBe("reading");
    expect(asLiteral("scrolled", modes)).toBeUndefined();
    expect(asLiteral(2, modes)).toBeUndefined();
  });
});

describe("asNullableObject", () => {
  it("classifies null vs object vs other", () => {
    expect(asNullableObject(null)).toBe("null");
    expect(asNullableObject({ num: 1 })).toBe("object");
    expect(asNullableObject([1])).toBeUndefined();
    expect(asNullableObject(3)).toBeUndefined();
    expect(asNullableObject(undefined)).toBeUndefined();
  });
});

describe("asBooleanRecord", () => {
  it("keeps only entries whose value is exactly true", () => {
    expect(asBooleanRecord({ "1:1": true, "2:2": false, "3:3": 1 })).toEqual({ "1:1": true });
  });
  it("returns {} for non-objects", () => {
    expect(asBooleanRecord(null)).toEqual({});
    expect(asBooleanRecord("x")).toEqual({});
    expect(asBooleanRecord([true])).toEqual({});
  });
});

describe("asStringRecord", () => {
  it("keeps only entries whose value is a string", () => {
    expect(asStringRecord({ "1:1": "note", "2:2": 5, "3:3": null })).toEqual({ "1:1": "note" });
  });
  it("returns {} for non-objects", () => {
    expect(asStringRecord(null)).toEqual({});
    expect(asStringRecord(42)).toEqual({});
  });
});
