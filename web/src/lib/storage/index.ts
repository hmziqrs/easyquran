/* storage/ — policy-aware persistence mechanics shared by the domain stores.
 * Import from "$lib/storage" (or the submodules directly). */
export {
  readJSON,
  writeJSON,
  removeJSON,
  isFutureSchema,
  onStorageKey,
  onPageHide,
} from "./safe-storage";
export {
  asObject,
  asNumber,
  asString,
  asLiteral,
  asNullableObject,
  asBooleanRecord,
  asStringRecord,
} from "./decoders";
export { trailingDebounce } from "./debounce";
export type { Debounced } from "./debounce";
