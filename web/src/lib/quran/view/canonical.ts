import { OpenerKind, type OpenerKind as OpenerKindValue } from "../../data/quran-types.ts";

/** Canonical opener meaning for the supported Hafs/Medina numbering model. */
export function canonicalOpenerKind(surah: number): OpenerKindValue {
  if (surah === 1) return OpenerKind.Verse;
  if (surah === 9) return OpenerKind.None;
  return OpenerKind.Header;
}
