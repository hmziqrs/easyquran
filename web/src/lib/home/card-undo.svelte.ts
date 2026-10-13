import { homeCards, type HomeCard } from "./cards.svelte.js";

/**
 * Module-owned undo state for card removal (bookmarks BM-04 pattern:
 * optimistic remove + 5s undo). Module level so the banner survives the
 * detail page's navigation back to /yours — the snapshot is armed on one
 * surface and restored from the other. Rapid removals coalesce to the newest.
 * Restore goes through homeCards.add(): when the store is back at MAX_CARDS
 * the banner STAYS armed with the refusal line and the window restarts,
 * because decode trims to the first MAX_CARDS rows — an over-cap append would
 * be silently dropped on the next reload. Position is not preserved (add
 * appends last).
 */
export interface CardUndoState {
  readonly card: HomeCard;
  readonly failed: boolean;
}

const UNDO_WINDOW_MS = 5000;

let state = $state<CardUndoState | null>(null);
let timer: ReturnType<typeof setTimeout> | undefined;

function scheduleExpiry(): void {
  clearTimeout(timer);
  timer = setTimeout(() => {
    state = null;
  }, UNDO_WINDOW_MS);
}

/** Reactive read for banner surfaces — null while no removal is pending. */
export function cardUndoState(): CardUndoState | null {
  return state;
}

export function armCardUndo(card: HomeCard): void {
  state = { card, failed: false };
  scheduleExpiry();
}

export function dismissCardUndo(): void {
  clearTimeout(timer);
  state = null;
}

/** Restores the snapshot; on a full store the banner re-arms with its refusal. */
export function undoCardRemoval(): void {
  const current = state;
  if (current === null) return;
  if (homeCards.add(current.card)) {
    clearTimeout(timer);
    state = null;
    return;
  }
  state = { ...current, failed: true };
  scheduleExpiry();
}
