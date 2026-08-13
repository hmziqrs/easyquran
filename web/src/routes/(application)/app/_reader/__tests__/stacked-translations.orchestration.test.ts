import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/environment", () => ({ browser: true }));

const { workerStub } = vi.hoisted(() => ({
  // eslint-disable-next-line anti-slop/no-object-as-type -- vi.fn doubles the quranWorker surface; only readRange is exercised here
  workerStub: { readRange: vi.fn() },
}));
vi.mock("$lib/quran/worker-client", () => ({ quranWorker: workerStub }));

import type { TranslationCatalogueEntry } from "$lib/data/quran-types";
import type { AyahCoordinateValidator } from "$lib/quran/wire";
import { stackedTranslations } from "$lib/stores/stacked-translations.svelte";

import {
  createStackedTranslations,
  erroredFor,
  loadingFor,
  stackedFor,
  type CreateStackedTranslationsOptions,
} from "../stacked-translations.svelte";

const CATALOGUE: readonly TranslationCatalogueEntry[] = [
  {
    id: "en.sahih",
    language: "English",
    languageCode: "en",
    direction: "ltr",
    name: "Saheeh International",
    translator: "Saheeh International",
    sizeBytes: 1,
    downloadUrl: "",
  },
  {
    id: "ur.jalandhry",
    language: "Urdu",
    languageCode: "ur",
    direction: "rtl",
    name: "Maulana Jalal ad-Din",
    translator: "Maulana Jalal ad-Din",
    sizeBytes: 1,
    downloadUrl: "",
  },
];

const validator = (): boolean => true;

interface Inputs {
  from: number;
  to: number;
  routeKey: string;
  primary: string | null;
}

function rangeText(id: string, from: number, to: number) {
  return {
    ayahs: Array.from({ length: to - from + 1 }, (_, i) => ({
      key: `1:${from + i}`,
      surah: 1,
      ayah: from + i,
      globalIndex: from + i,
      text: `${id}-v${from + i}`,
    })),
    normalizations: [],
  };
}

function makeController(inputs: Inputs) {
  return createStackedTranslations({
    from: () => inputs.from,
    to: () => inputs.to,
    routeKey: () => inputs.routeKey,
    primarySourceId: () => inputs.primary,
    catalogue: () => CATALOGUE,
    validator: () => validator,
  } satisfies CreateStackedTranslationsOptions);
}

function flush(n = 12): Promise<void> {
  let p = Promise.resolve();
  for (let i = 0; i < n; i++) p = p.then(() => undefined);
  return p;
}

describe("createStackedTranslations", () => {
  beforeEach(() => {
    window.localStorage.clear();
    stackedTranslations.setIds([]);
    workerStub.readRange.mockReset();
  });
  afterEach(() => stackedTranslations.setIds([]));

  it("loads an extra and resolves ready text in store order", async () => {
    stackedTranslations.setIds(["en.sahih", "ur.jalandhry"]);
    workerStub.readRange.mockImplementation((f: number, t: number, _v: AyahCoordinateValidator, id: string) =>
      Promise.resolve(rangeText(id, f, t)),
    );
    const ctrl = makeController({ from: 1, to: 3, routeKey: "surah:1", primary: null });
    ctrl.sync();
    expect(loadingFor(ctrl.state, "1:1")).toEqual(["en.sahih", "ur.jalandhry"]);
    await flush();
    expect(loadingFor(ctrl.state, "1:1")).toEqual([]);
    expect(stackedFor(ctrl.state, "1:1").map((t) => t.sourceId)).toEqual([
      "en.sahih",
      "ur.jalandhry",
    ]);
    expect(stackedFor(ctrl.state, "1:1")[0]?.text).toBe("en.sahih-v1");
    ctrl.dispose();
  });

  it("accumulates byVerse across from/to expansion without clearing (no skeleton re-flash)", async () => {
    stackedTranslations.setIds(["en.sahih"]);
    workerStub.readRange.mockImplementation((f: number, t: number, _v: AyahCoordinateValidator, id: string) =>
      Promise.resolve(rangeText(id, f, t)),
    );
    const inputs: Inputs = { from: 1, to: 3, routeKey: "surah:1", primary: null };
    const ctrl = makeController(inputs);
    ctrl.sync();
    await flush();
    expect(stackedFor(ctrl.state, "1:1").map((t) => t.sourceId)).toEqual(["en.sahih"]);

    inputs.to = 6;
    ctrl.sync();
    expect(ctrl.state.byVerse.size).toBe(3);
    await flush();
    expect(stackedFor(ctrl.state, "1:1").map((t) => t.sourceId)).toEqual(["en.sahih"]);
    expect(stackedFor(ctrl.state, "1:6").map((t) => t.sourceId)).toEqual(["en.sahih"]);
    expect(ctrl.state.byVerse.size).toBe(6);
    ctrl.dispose();
  });

  it("clears byVerse only on route-identity change, never on range expansion", async () => {
    stackedTranslations.setIds(["en.sahih"]);
    workerStub.readRange.mockImplementation((f: number, t: number, _v: AyahCoordinateValidator, id: string) =>
      Promise.resolve(rangeText(id, f, t)),
    );
    const inputs: Inputs = { from: 1, to: 3, routeKey: "surah:1", primary: null };
    const ctrl = makeController(inputs);
    ctrl.sync();
    await flush();
    expect(ctrl.state.byVerse.size).toBe(3);

    inputs.to = 6;
    ctrl.sync();
    expect(ctrl.state.byVerse.size).toBe(3);
    await flush();

    inputs.routeKey = "surah:2";
    ctrl.sync();
    expect(ctrl.state.byVerse.size).toBe(0);
    expect(loadingFor(ctrl.state, "1:1")).toEqual(["en.sahih"]);
    ctrl.dispose();
  });

  it("passes the validator as a function (3rd arg), never invokes it as a factory", () => {
    const v = vi.fn(validator);
    stackedTranslations.setIds(["en.sahih"]);
    workerStub.readRange.mockResolvedValue({ ayahs: [], normalizations: [] });
    const ctrl = createStackedTranslations({
      from: () => 1,
      to: () => 1,
      routeKey: () => "s",
      primarySourceId: () => null,
      catalogue: () => CATALOGUE,
      validator: () => v,
    });
    ctrl.sync();
    expect(workerStub.readRange).toHaveBeenCalledWith(1, 1, v, "en.sahih");
    expect(v).not.toHaveBeenCalled();
    ctrl.dispose();
  });

  it("dedupes the route primary so it is never fetched as an extra", () => {
    stackedTranslations.setIds(["en.sahih", "ur.jalandhry"]);
    workerStub.readRange.mockResolvedValue({ ayahs: [], normalizations: [] });
    const ctrl = makeController({ from: 1, to: 1, routeKey: "t:en.sahih", primary: "en.sahih" });
    ctrl.sync();
    expect(ctrl.state.order).toEqual(["ur.jalandhry"]);
    expect(workerStub.readRange).toHaveBeenCalledTimes(1);
    expect(workerStub.readRange).toHaveBeenCalledWith(1, 1, validator, "ur.jalandhry");
    ctrl.dispose();
  });

  it("dispose() invalidates in-flight callbacks (post-dispose writes are no-op)", async () => {
    stackedTranslations.setIds(["en.sahih"]);
    let resolveRead!: (value: ReturnType<typeof rangeText>) => void;
    workerStub.readRange.mockImplementation(
      () => new Promise<ReturnType<typeof rangeText>>((res) => {
        resolveRead = res;
      }),
    );
    const ctrl = makeController({ from: 1, to: 3, routeKey: "surah:1", primary: null });
    ctrl.sync();
    expect(loadingFor(ctrl.state, "1:1")).toEqual(["en.sahih"]);

    ctrl.dispose();
    resolveRead(rangeText("en.sahih", 1, 3));
    await flush();

    expect(ctrl.state.byVerse.size).toBe(0);
    expect(stackedFor(ctrl.state, "1:1")).toEqual([]);
    ctrl.dispose();
  });

  it("a thrown readRange (offline + cold extra) surfaces as erroredFor, not loadingFor", async () => {
    stackedTranslations.setIds(["en.sahih"]);
    workerStub.readRange.mockRejectedValue(new Error("offline + cold"));
    const ctrl = makeController({ from: 1, to: 3, routeKey: "surah:1", primary: null });
    ctrl.sync();
    expect(loadingFor(ctrl.state, "1:1")).toEqual(["en.sahih"]);
    await flush();
    expect(erroredFor(ctrl.state, "1:1")).toEqual(["en.sahih"]);
    expect(loadingFor(ctrl.state, "1:1")).toEqual([]);
    expect(stackedFor(ctrl.state, "1:1")).toEqual([]);
    ctrl.dispose();
  });

  it("re-selecting a deselected errored extra retries instead of staying stuck", async () => {
    stackedTranslations.setIds(["en.sahih"]);
    workerStub.readRange.mockImplementation((f: number, t: number, _v: AyahCoordinateValidator, id: string) =>
      Promise.resolve(rangeText(id, f, t)),
    );
    workerStub.readRange.mockRejectedValueOnce(new Error("cold"));
    const ctrl = makeController({ from: 1, to: 3, routeKey: "surah:1", primary: null });
    ctrl.sync();
    await flush();
    expect(erroredFor(ctrl.state, "1:1")).toEqual(["en.sahih"]);

    stackedTranslations.remove("en.sahih");
    ctrl.sync();
    expect(erroredFor(ctrl.state, "1:1")).toEqual([]);

    workerStub.readRange.mockClear();
    stackedTranslations.setIds(["en.sahih"]);
    ctrl.sync();
    await flush();
    expect(stackedFor(ctrl.state, "1:1").map((t) => t.sourceId)).toEqual(["en.sahih"]);
    expect(loadingFor(ctrl.state, "1:1")).toEqual([]);
    ctrl.dispose();
  });

  it("does not refetch a ready extra when only the selection changes elsewhere", async () => {
    stackedTranslations.setIds(["en.sahih"]);
    workerStub.readRange.mockImplementation((f: number, t: number, _v: AyahCoordinateValidator, id: string) =>
      Promise.resolve(rangeText(id, f, t)),
    );
    const ctrl = makeController({ from: 1, to: 3, routeKey: "surah:1", primary: null });
    ctrl.sync();
    await flush();
    expect(stackedFor(ctrl.state, "1:1").length).toBe(1);

    workerStub.readRange.mockClear();
    stackedTranslations.toggle("ur.jalandhry");
    ctrl.sync();
    await flush();
    const calledSources = workerStub.readRange.mock.calls.map((c) => c[3]);
    expect(calledSources.filter((id) => id === "en.sahih")).toHaveLength(0);
    expect(calledSources.filter((id) => id === "ur.jalandhry")).toHaveLength(1);
    ctrl.dispose();
  });

  it("a ready extra that errors on an expansion refetch keeps loaded text without a doubled error row", async () => {
    stackedTranslations.setIds(["en.sahih"]);
    workerStub.readRange.mockImplementation((f: number, t: number, _v: AyahCoordinateValidator, id: string) =>
      Promise.resolve(rangeText(id, f, t)),
    );
    const inputs: Inputs = { from: 1, to: 3, routeKey: "surah:1", primary: null };
    const ctrl = makeController(inputs);
    ctrl.sync();
    await flush();
    expect(stackedFor(ctrl.state, "1:1").length).toBe(1);

    workerStub.readRange.mockRejectedValue(new Error("expansion offline"));
    inputs.to = 6;
    ctrl.sync();
    await flush();
    expect(stackedFor(ctrl.state, "1:1").map((t) => t.sourceId)).toEqual(["en.sahih"]);
    expect(erroredFor(ctrl.state, "1:1")).toEqual([]);
    expect(erroredFor(ctrl.state, "1:6")).toEqual(["en.sahih"]);
    ctrl.dispose();
  });

  it("an id with no catalogue meta surfaces as an error, never a silent ready", async () => {
    stackedTranslations.setIds(["en.sahih"]);
    workerStub.readRange.mockImplementation((f: number, t: number, _v: AyahCoordinateValidator, id: string) =>
      Promise.resolve(rangeText(id, f, t)),
    );
    const ctrl = createStackedTranslations({
      from: () => 1,
      to: () => 3,
      routeKey: () => "surah:1",
      primarySourceId: () => null,
      catalogue: () => CATALOGUE.filter((entry) => entry.id !== "en.sahih"),
      validator: () => validator,
    });
    ctrl.sync();
    await flush();
    expect(ctrl.state.status.get("en.sahih")).toBe("error");
    expect(erroredFor(ctrl.state, "1:1")).toEqual(["en.sahih"]);
    expect(stackedFor(ctrl.state, "1:1")).toEqual([]);
    ctrl.dispose();
  });

  it("an in-flight fetch from the prior route cannot write into the new route's state", async () => {
    stackedTranslations.setIds(["en.sahih"]);
    const pending: Array<(value: ReturnType<typeof rangeText>) => void> = [];
    workerStub.readRange.mockImplementation(
      () => new Promise<ReturnType<typeof rangeText>>((res) => pending.push(res)),
    );
    const inputs: Inputs = { from: 1, to: 3, routeKey: "surah:1", primary: null };
    const ctrl = makeController(inputs);
    ctrl.sync();
    expect(pending.length).toBe(1);

    inputs.routeKey = "surah:2";
    ctrl.sync();
    expect(pending.length).toBe(2);

    pending[0]!(rangeText("en.sahih", 1, 3));
    await flush();
    expect(ctrl.state.byVerse.size).toBe(0);

    pending[1]!(rangeText("en.sahih", 1, 3));
    await flush();
    expect(ctrl.state.byVerse.size).toBe(3);
    ctrl.dispose();
  });
});
