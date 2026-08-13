/**
 * IndexedDB exposes its failure reason as `DOMException | null` — `tx.error` is null on a plain
 * abort. Rejecting with that raw value hands `null` to the catch block and loses the reason
 * entirely, so every rejection is funnelled through here to guarantee a real Error.
 */
export function idbError(reason: DOMException | null, context: string): Error {
  return reason ?? new Error(`[idb] ${context} failed without an error reason`);
}
