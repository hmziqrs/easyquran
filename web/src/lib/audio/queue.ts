import { isSurahNumber } from "./url";

/** A contiguous run of ayahs inside one surah. Clamping to the surah is the caller's job. */
export interface QueueSegment {
  readonly surah: number;
  readonly from: number;
  readonly to: number;
}

export interface QueueItem {
  readonly surah: number;
  readonly ayah: number;
}

export interface PlayQueue {
  readonly segments: readonly QueueSegment[];
  /** Every ayah in play order, flattened across segments. */
  readonly items: readonly QueueItem[];
  readonly index: number;
}

/** Upper bound on queue length, so a bad saved session cannot build a huge list. */
export const MAX_QUEUE_ITEMS = 1000;

export function isValidSegment(segment: QueueSegment): boolean {
  if (!isSurahNumber(segment.surah)) return false;
  if (!Number.isInteger(segment.from) || !Number.isInteger(segment.to)) return false;
  return segment.from >= 1 && segment.to >= segment.from;
}

/** Null for no segments, an invalid segment, or a queue over MAX_QUEUE_ITEMS. */
export function buildQueue(segments: readonly QueueSegment[]): PlayQueue | null {
  if (segments.length === 0) return null;
  const items: QueueItem[] = [];
  for (const segment of segments) {
    if (!isValidSegment(segment)) return null;
    for (let ayah = segment.from; ayah <= segment.to; ayah++) {
      items.push({ surah: segment.surah, ayah });
    }
    if (items.length > MAX_QUEUE_ITEMS) return null;
  }
  return { segments: [...segments], items, index: 0 };
}

export function queueAt(queue: PlayQueue, index: number): PlayQueue | null {
  if (!Number.isInteger(index) || index < 0 || index >= queue.items.length) return null;
  return { ...queue, index };
}

/** Moves one ayah forward (delta 1) or back (delta -1). Null at either end. */
export function stepQueue(queue: PlayQueue, delta: 1 | -1): PlayQueue | null {
  return queueAt(queue, queue.index + delta);
}

export function currentItem(queue: PlayQueue): QueueItem | undefined {
  return queue.items[queue.index];
}

/**
 * Index of the first ayah of the next (direction 1) or previous (direction -1)
 * surah, or null when there is none in the queue.
 */
export function surahBoundary(queue: PlayQueue, direction: 1 | -1): number | null {
  const current = currentItem(queue);
  if (!current) return null;
  if (direction === 1) {
    for (let j = queue.index + 1; j < queue.items.length; j++) {
      if (queue.items[j]?.surah !== current.surah) return j;
    }
    return null;
  }
  let j = queue.index - 1;
  while (j >= 0 && queue.items[j]?.surah === current.surah) j--;
  if (j < 0) return null;
  const previousSurah = queue.items[j]?.surah;
  while (j - 1 >= 0 && queue.items[j - 1]?.surah === previousSurah) j--;
  return j;
}
