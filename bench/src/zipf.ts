/** Deterministic PRNG (mulberry32) — same seed gives the same key stream for every runtime. */
export function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Zipf sampler over ranks 1..n with exponent alpha. Precomputes the CDF once, then samples by
 * binary search — O(log n) per draw, so target generation never becomes the bottleneck.
 */
export class Zipf {
  readonly #cdf: Float64Array;
  readonly #next: () => number;

  constructor(n: number, alpha: number, seed: number) {
    if (n < 1) throw new Error(`[zipf] n must be >= 1, got ${n}`);
    this.#cdf = new Float64Array(n);
    let total = 0;
    for (let i = 0; i < n; i += 1) {
      total += 1 / Math.pow(i + 1, alpha);
      this.#cdf[i] = total;
    }
    for (let i = 0; i < n; i += 1) this.#cdf[i] /= total;
    this.#next = rng(seed);
  }

  /** Returns a zero-based rank; rank 0 is the most popular. */
  sample(): number {
    const u = this.#next();
    let lo = 0;
    let hi = this.#cdf.length - 1;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (this.#cdf[mid]! < u) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  }
}
