import type { ReadingThread } from "#lib/sessions/schema.js";
import type { HomeCard, SnippetCard } from "./cards.svelte.js";
import { slotStateFor } from "./schedule.js";

/**
 * Zikr-session pickers for the azkar page's recite panel: which card a
 * card-origin visit binds to, and which card the "next zikr" row offers once
 * the bound card's target is reached (or the reader asks for more after a
 * target-less recite). Pure helpers — no runes, explicit `nowMs` where the
 * schedule matters — so the panel and the unit tests share one rule.
 */

/** True once a card with a target has counted its way to that target. */
export function isTargetDone(card: HomeCard): boolean {
  return card.target !== undefined && card.count >= card.target;
}

function isDueNow(card: HomeCard, nowMs: number): boolean {
  const schedule = card.schedule;
  if (schedule === undefined) return false;
  return slotStateFor(schedule, nowMs)?.state === "due";
}

/**
 * The card a card-origin thread names: the thread's `originRef` card when it
 * still exists, is a snippet card and is a card of THIS page's slug. Anything
 * else — no thread, a non-card origin, a scope from another page, a stale or
 * cross-kind ref — returns null so the caller falls back to the first
 * same-slug match. Ended threads still bind: the thread keeps naming the card
 * the reader opened, and flipping to a sibling mid-visit would be worse.
 */
export function boundCardFor(
  thread: ReadingThread | null,
  cards: readonly HomeCard[],
  slug: string,
): SnippetCard | null {
  if (thread === null || thread.origin !== "card") return null;
  const ref = thread.originRef;
  if (ref === null || thread.scope !== `snippet:${slug}`) return null;
  const card = cards.find((entry) => entry.id === ref);
  if (card === undefined || card.kind !== "snippet" || card.slug !== slug) return null;
  return card;
}

/**
 * The next card worth reciting after `current`, deterministic:
 *
 * 1. Rotate the list to start AFTER the current card's position, wrapping;
 *    an unknown current id means the whole list in store order.
 * 2. Drop cards with no reader destination for this flow: same-slug snippet
 *    cards (navigating reloads this very page), zikr phrase cards (no href).
 * 3. Drop target-reached cards — finished cards are never re-offered.
 * 4. A schedule-due card wins (earliest in the wrapped order); otherwise the
 *    first remaining card in wrapped order.
 *
 * Nothing left → null: the quiet all-done line, never a dead CTA.
 */
export function nextZikrCard(
  cards: readonly HomeCard[],
  current: { readonly id: string; readonly slug: string },
  nowMs: number = Date.now(),
): HomeCard | null {
  const index = cards.findIndex((card) => card.id === current.id);
  const wrapped = index === -1 ? [...cards] : [...cards.slice(index + 1), ...cards.slice(0, index)];
  const eligible = wrapped.filter((card) => {
    if (card.kind === "zikr") return false;
    if (card.kind === "snippet" && card.slug === current.slug) return false;
    return !isTargetDone(card);
  });
  return eligible.find((card) => isDueNow(card, nowMs)) ?? eligible[0] ?? null;
}
