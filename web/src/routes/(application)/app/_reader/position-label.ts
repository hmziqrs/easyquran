import type { ReaderPositionState } from "$lib/data/mushaf-divisions";
import type { ReaderUiCopy } from "$lib/i18n/reader-copy";

/** "Page {page} · Juz {juz} · Hizb {hizb}" with the un-known divisions omitted. */
export function positionLabel(
  copy: ReaderUiCopy,
  position: ReaderPositionState | { globalPage: number; juz?: number | null; hizb?: number | null },
): string {
  let out = copy.shell.positionPage(position.globalPage);
  if (position.juz !== null && position.juz !== undefined) {
    out += ` · ${copy.shell.positionJuz(position.juz)}`;
  }
  if (position.hizb !== null && position.hizb !== undefined) {
    out += ` · ${copy.shell.positionHizb(position.hizb)}`;
  }
  return out;
}
