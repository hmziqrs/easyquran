// Per-IP budget for offline pack downloads (A16). Process-local: the app runs as
// a single instance, and a restart only resets a daily allowance.
//
// Values are the A16 spec: 20 downloads a day, a burst of 5 per minute, and
// 100 MB a day per address. A GET of the pack counts; HEAD does not.

export const PACK_DOWNLOADS_PER_DAY = 20;
export const PACK_BURST = 5;
export const PACK_BURST_WINDOW_MS = 60_000;
export const PACK_BYTES_PER_DAY = 100 * 1024 * 1024;

const DAY_MS = 86_400_000;
const MAX_TRACKED_IPS = 10_000;

interface Account {
  day: number;
  downloads: number;
  bytes: number;
  burstStart: number;
  burstCount: number;
}

export interface PackBudgetDecision {
  readonly retryAfterSecs: number;
  readonly reason: "daily-downloads" | "daily-bytes" | "burst";
}

export class OfflinePackBudget {
  readonly #accounts = new Map<string, Account>();

  /** Charges one download of `bytes` to `ip`. Returns null when allowed. */
  charge(ip: string, bytes: number, now: number): PackBudgetDecision | null {
    const day = Math.floor(now / DAY_MS);
    const secsToMidnight = Math.ceil((day * DAY_MS + DAY_MS - now) / 1000);
    let account = this.#accounts.get(ip);
    if (account === undefined || account.day !== day) {
      if (this.#accounts.size >= MAX_TRACKED_IPS) this.#sweep(day);
      // Still full of today's addresses: evict the oldest so memory stays bounded.
      if (this.#accounts.size >= MAX_TRACKED_IPS) {
        const oldest = this.#accounts.keys().next();
        if (oldest.done !== true) this.#accounts.delete(oldest.value);
      }
      account = { day, downloads: 0, bytes: 0, burstStart: now, burstCount: 0 };
      this.#accounts.set(ip, account);
    }
    if (account.downloads >= PACK_DOWNLOADS_PER_DAY) {
      return { retryAfterSecs: secsToMidnight, reason: "daily-downloads" };
    }
    if (account.bytes + bytes > PACK_BYTES_PER_DAY) {
      return { retryAfterSecs: secsToMidnight, reason: "daily-bytes" };
    }
    if (now - account.burstStart >= PACK_BURST_WINDOW_MS) {
      account.burstStart = now;
      account.burstCount = 0;
    }
    if (account.burstCount >= PACK_BURST) {
      const wait = Math.ceil((account.burstStart + PACK_BURST_WINDOW_MS - now) / 1000);
      return { retryAfterSecs: Math.max(1, wait), reason: "burst" };
    }
    account.downloads += 1;
    account.bytes += bytes;
    account.burstCount += 1;
    return null;
  }

  #sweep(day: number): void {
    for (const [ip, account] of this.#accounts) {
      if (account.day !== day) this.#accounts.delete(ip);
    }
  }
}
