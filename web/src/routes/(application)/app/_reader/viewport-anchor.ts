import { clamp } from "es-toolkit";

export type ViewportAnchor =
  | {
      kind: "verse";
      localPage: number;
      verseKey: string;
      viewportPoint: number;
      ratio: number;
    }
  | {
      kind: "page";
      localPage: number;
      viewportPoint: number;
      ratio: number;
    };

export function viewportMarker(): number {
  return Math.min(window.innerHeight * 0.35, 260);
}

export function nextFrame(): Promise<void> {
  return new Promise((resolveFrame) => requestAnimationFrame(() => resolveFrame()));
}

export function closestPage(container: HTMLElement, marker: number): HTMLElement | null {
  const sections = [...container.querySelectorAll<HTMLElement>("[data-local-page]")];
  let closest: HTMLElement | null = null;
  let closestDistance = Number.POSITIVE_INFINITY;
  for (const section of sections) {
    const rect = section.getBoundingClientRect();
    if (rect.top <= marker && rect.bottom > marker) return section;
    const distance = Math.min(Math.abs(rect.top - marker), Math.abs(rect.bottom - marker));
    if (distance < closestDistance) {
      closest = section;
      closestDistance = distance;
    }
  }
  return closest;
}

function closestVerse(section: HTMLElement, marker: number): HTMLElement | null {
  let closest: HTMLElement | null = null;
  let closestDistance = Number.POSITIVE_INFINITY;
  for (const row of section.querySelectorAll<HTMLElement>("[data-verse-key]")) {
    const text = row.querySelector<HTMLElement>(".verse-text");
    if (!text) continue;
    const rect = text.getBoundingClientRect();
    if (rect.top <= marker && rect.bottom > marker) return row;
    const distance = Math.min(Math.abs(rect.top - marker), Math.abs(rect.bottom - marker));
    if (distance < closestDistance) {
      closest = row;
      closestDistance = distance;
    }
  }
  return closest;
}

function anchorRatio(marker: number, rect: DOMRect): number {
  return clamp((marker - rect.top) / Math.max(rect.height, 1), 0, 1);
}

export function captureViewportAnchor(container: HTMLElement | null): ViewportAnchor | null {
  if (!container) return null;
  const marker = viewportMarker();
  const section = closestPage(container, marker);
  if (!section) return null;
  const localPage = Number(section.dataset.localPage);
  if (!Number.isSafeInteger(localPage)) return null;
  if (section.hasAttribute("data-page-rendered")) {
    const row = closestVerse(section, marker);
    const text = row?.querySelector<HTMLElement>(".verse-text");
    const verseKey = row?.dataset.verseKey;
    if (text && verseKey) {
      const textRect = text.getBoundingClientRect();
      return {
        kind: "verse",
        localPage,
        verseKey,
        // The anchor must remember where the tracked point actually WAS on
        // screen — not the search marker. Near the document top the marker
        // falls outside the content, the nearest verse sits BELOW it, and
        // storing the marker made every restore scroll that verse up to the
        // marker — the visible jump at scrollY 0. When the marker falls
        // inside the text, tracked === marker and nothing changes.
        viewportPoint: truePoint(textRect, anchorRatio(marker, textRect)),
        ratio: anchorRatio(marker, textRect),
      };
    }
  }
  const sectionRect = section.getBoundingClientRect();
  return {
    kind: "page",
    localPage,
    viewportPoint: truePoint(sectionRect, anchorRatio(marker, sectionRect)),
    ratio: anchorRatio(marker, sectionRect),
  };
}

/** The on-screen position of the point `ratio` through the rect at capture time. */
function truePoint(rect: DOMRect, ratio: number): number {
  return rect.top + rect.height * ratio;
}

export function restoreViewportAnchor(
  container: HTMLElement | null,
  anchor: ViewportAnchor,
): boolean {
  if (!container) return false;
  let node: HTMLElement | null = null;
  if (anchor.kind === "verse") {
    node = container.querySelector<HTMLElement>(
      `[data-verse-key="${anchor.verseKey}"] .verse-text`,
    );
  }
  node ??= container.querySelector<HTMLElement>(`[data-local-page="${anchor.localPage}"]`);
  if (!node) return false;
  const rect = node.getBoundingClientRect();
  const targetPoint = rect.top + rect.height * anchor.ratio;
  const delta = targetPoint - anchor.viewportPoint;
  if (Math.abs(delta) > 0.5) window.scrollTo(0, window.scrollY + delta);
  return true;
}
