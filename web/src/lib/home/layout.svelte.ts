import { browser } from "$app/env";

import { asNumber, asObject, asString, readJSON, writeJSON } from "#lib/storage/index.js";

/** Homepage widget layout (Feature 5). Local-only per §11.3: `easyquran.home`, never synced — per-device widgets need no sync policy (documentation/settings-system.md §7 gap stays moot). */

const STORAGE_KEY = "easyquran.home";
const SCHEMA_VERSION = 1;

export const HOME_WIDGET_IDS = [
  "continue-reading",
  "bookmarks",
  "notes",
  "recents",
  "most-read",
  "daily-ayah",
] as const;
export type HomeWidgetId = (typeof HOME_WIDGET_IDS)[number];

/** Per-widget settings. V1 surfaces the recents row count (clamped 1..8, default 5). */
export interface HomeWidgetSettings {
  count?: number;
}

export interface HomeWidgetEntry {
  readonly id: HomeWidgetId;
  readonly visible: boolean;
  /** Grid span (forward-compat; the v1 column is single-track so every span is 1). */
  readonly span: number;
  readonly settings: HomeWidgetSettings;
}

export interface HomeLayoutEnvelope {
  readonly v: number;
  readonly widgets: readonly HomeWidgetEntry[];
}

interface WidgetSpec {
  readonly id: HomeWidgetId;
  /** BM-01 hub order default: Continue → Bookmarks → Notes → Recents visible. */
  readonly defaultVisible: boolean;
  readonly defaultSpan: number;
  readonly defaultCount?: number;
}

export const HOME_WIDGET_SPECS: readonly WidgetSpec[] = [
  { id: "continue-reading", defaultVisible: true, defaultSpan: 1 },
  { id: "bookmarks", defaultVisible: true, defaultSpan: 1 },
  { id: "notes", defaultVisible: true, defaultSpan: 1 },
  { id: "recents", defaultVisible: true, defaultSpan: 1, defaultCount: 5 },
  // Present but hidden by default (Feature 5.2): most-read consumes the opt-in
  // analytics surface (Feature 6); daily-ayah is editorial extra.
  { id: "most-read", defaultVisible: false, defaultSpan: 1 },
  { id: "daily-ayah", defaultVisible: false, defaultSpan: 1 },
];

const RECENTS_COUNT_MIN = 1;
const RECENTS_COUNT_MAX = 8;
export const RECENTS_COUNT_CHOICES = [3, 5, 8] as const;

function defaultEntry(spec: WidgetSpec): HomeWidgetEntry {
  const settings: HomeWidgetSettings =
    spec.defaultCount === undefined ? {} : { count: spec.defaultCount };
  return { id: spec.id, visible: spec.defaultVisible, span: spec.defaultSpan, settings };
}

function defaultEnvelope(): HomeLayoutEnvelope {
  return { v: SCHEMA_VERSION, widgets: HOME_WIDGET_SPECS.map((spec) => defaultEntry(spec)) };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one widget's unparsed settings object from the localStorage envelope; this decoder is its parse boundary.
function decodeSettings(raw: unknown, spec: WidgetSpec): HomeWidgetSettings {
  const stored = asObject(raw);
  if (!stored) return {};
  const count = asNumber(stored.count, RECENTS_COUNT_MIN, RECENTS_COUNT_MAX);
  if (spec.defaultCount === undefined || count === undefined) return {};
  return { count };
}

/** Forward-compatible decode (Feature 5.2): unknown ids drop, missing ids append, every field re-validated — bad rows degrade to defaults, never crash. */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON → JSON.parse); this function is the parser and validates every field below.
export function decodeHomeLayout(raw: unknown): HomeLayoutEnvelope {
  const stored = asObject(raw);
  if (!stored) return defaultEnvelope();
  if (stored.v !== SCHEMA_VERSION) return defaultEnvelope();
  const byId = new Map(HOME_WIDGET_SPECS.map((spec) => [spec.id, spec] as const));
  const widgets: HomeWidgetEntry[] = [];
  const seen = new Set<HomeWidgetId>();
  const rows = Array.isArray(stored.widgets) ? stored.widgets : [];
  for (const row of rows) {
    const entry = asObject(row);
    if (!entry) continue;
    const rawId = asString(entry.id);
    // SAFETY: byId is keyed by exactly the HOME_WIDGET_IDS union, so a get() hit proves rawId is a HomeWidgetId.
    const spec = rawId === undefined ? undefined : byId.get(rawId as HomeWidgetId);
    if (spec === undefined) continue;
    // SAFETY: spec !== undefined re-proves the same membership for the row id below.
    const id = rawId as HomeWidgetId;
    if (seen.has(id)) continue;
    seen.add(id);
    widgets.push({
      id,
      visible: entry.visible !== false,
      span: asNumber(entry.span, 1, 2) ?? spec.defaultSpan,
      settings: decodeSettings(entry.settings, spec),
    });
  }
  for (const spec of HOME_WIDGET_SPECS) {
    if (!seen.has(spec.id)) widgets.push(defaultEntry(spec));
  }
  return { v: SCHEMA_VERSION, widgets };
}

class HomeLayoutStore {
  #envelope = $state.raw<HomeLayoutEnvelope>(defaultEnvelope());
  #hydrated = false;

  hydrate(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;
    this.#envelope = decodeHomeLayout(readJSON(STORAGE_KEY));
  }

  get entries(): readonly HomeWidgetEntry[] {
    return this.#envelope.widgets;
  }

  toggle(id: HomeWidgetId): void {
    this.#patch(id, (entry) => ({ ...entry, visible: !entry.visible }));
  }

  move(id: HomeWidgetId, direction: -1 | 1): void {
    const widgets = [...this.#envelope.widgets];
    const index = widgets.findIndex((entry) => entry.id === id);
    const target = index + direction;
    if (index === -1 || target < 0 || target >= widgets.length) return;
    const [moved] = widgets.splice(index, 1);
    if (moved === undefined) return;
    widgets.splice(target, 0, moved);
    this.#write({ ...this.#envelope, widgets });
  }

  setCount(id: HomeWidgetId, count: number): void {
    const clamped = asNumber(count, RECENTS_COUNT_MIN, RECENTS_COUNT_MAX);
    if (clamped === undefined) return;
    this.#patch(id, (entry) => ({ ...entry, settings: { count: clamped } }));
  }

  #patch(id: HomeWidgetId, patch: (entry: HomeWidgetEntry) => HomeWidgetEntry): void {
    const widgets = this.#envelope.widgets.map((entry) => (entry.id === id ? patch(entry) : entry));
    this.#write({ ...this.#envelope, widgets });
  }

  #write(next: HomeLayoutEnvelope): void {
    this.#envelope = next;
    if (browser) writeJSON(STORAGE_KEY, next);
  }
}

export const homeLayout = new HomeLayoutStore();
