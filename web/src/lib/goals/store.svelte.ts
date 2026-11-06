import { browser } from "$app/env";

import { authState } from "#lib/auth/auth-state.svelte.js";
import { readJSON, writeJSON } from "#lib/storage/index.js";
import { newEntityId } from "#lib/sync/ids.js";

import {
  daysBetween,
  decodeGoals,
  draftError,
  goalFromDraft,
  type GoalDraft,
  type GoalDraftError,
  type ReadingGoal,
} from "./goal-progress";

/**
 * Reading goals on this device (F8). One document per owner: signed out, the
 * device's own goals; signed in, the account's. The preferences domain carries the
 * account document as `goals.reading` (contracts/sync.md). Owners stay apart, so
 * one account's goals never land in another's document on a shared phone.
 */

export const GOALS_KEY = "goals.reading";
const LOCAL_OWNER = "local";

function storageKeyFor(owner: string): string {
  return `easyquran.goals.${owner}`;
}

function currentOwner(): string {
  if (authState.status !== "authenticated" || authState.user === null) return LOCAL_OWNER;
  return String(authState.user.id);
}

class GoalsStore {
  #owner = $state<string | null>(null);
  #goals = $state.raw<readonly ReadingGoal[]>([]);

  /** Goals for the current owner. Reads storage directly when the owner has changed. */
  get entries(): readonly ReadingGoal[] {
    const owner = currentOwner();
    // Read the state field first so a derived over this getter tracks later writes
    // (a storage-only read left the list stale until reload).
    const current = this.#goals;
    if (!browser) return [];
    if (owner === this.#owner) return current;
    return decodeGoals(readJSON(storageKeyFor(owner)));
  }

  /** Show a document that arrived from another device for this account. */
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is a synced value of uncertain shape; decodeGoals is its parser.
  replace(raw: unknown): void {
    this.#ensureOwner();
    this.#write(decodeGoals(raw));
  }

  /** Adds a goal. Returns the first problem with the draft, or null when it was stored. */
  add(draft: GoalDraft): GoalDraftError | null {
    this.#ensureOwner();
    const problem = draftError(draft, this.#goals.length);
    if (problem !== null) return problem;
    this.#write([...this.#goals, goalFromDraft(draft, newEntityId())]);
    return null;
  }

  /** Pauses a running goal from `today`. The window stops growing until it resumes. */
  pause(id: string, today: string): void {
    this.#ensureOwner();
    this.#patch(id, (goal) => (goal.pausedFrom === null ? { ...goal, pausedFrom: today } : goal));
  }

  /** Resumes a paused goal, extending its window by the days it was paused. */
  resume(id: string, today: string): void {
    this.#ensureOwner();
    this.#patch(id, (goal) => {
      if (goal.pausedFrom === null) return goal;
      const paused = Math.max(0, daysBetween(goal.pausedFrom, today));
      return { ...goal, pausedFrom: null, pausedDays: goal.pausedDays + paused };
    });
  }

  remove(id: string): void {
    this.#ensureOwner();
    this.#write(this.#goals.filter((goal) => goal.id !== id));
  }

  #patch(id: string, change: (goal: ReadingGoal) => ReadingGoal): void {
    this.#write(this.#goals.map((goal) => (goal.id === id ? change(goal) : goal)));
  }

  /** Loads the current owner's document once per owner change. Call from events and effects only. */
  #ensureOwner(): void {
    if (!browser) return;
    const owner = currentOwner();
    if (owner === this.#owner) return;
    this.#owner = owner;
    this.#goals = decodeGoals(readJSON(storageKeyFor(owner)));
  }

  #write(next: readonly ReadingGoal[]): void {
    this.#goals = next;
    if (browser) writeJSON(storageKeyFor(this.#owner ?? LOCAL_OWNER), { goals: next });
  }
}

export const goals = new GoalsStore();
