/**
 * Shared capture matrix for the visual harness (docs/plan/README.md (phase specs in git history)).
 *
 * Every script (capture/reference/states) and the V1 token sweep route draw their axes
 * from here so a palette added in one place is picked up everywhere. Tiers follow plan 07:
 * per commit (default palette, both modes, 1440 + 390), per phase (all palettes, both
 * modes, 1440), before merge (the full cross-product).
 */

export const PALETTES = ["sacred", "ink", "sepia", "sapphire"] as const;
export type Palette = (typeof PALETTES)[number];

/** Matches DEFAULT_PALETTE in web/src/lib/config/site.ts — keep the two in step. */
export const DEFAULT_PALETTE: Palette = "sacred";

export const MODES = ["light", "dark"] as const;
export type Mode = (typeof MODES)[number];

export const WIDTHS = [1440, 1280, 1024, 768, 390] as const;
export type Width = (typeof WIDTHS)[number];

export interface RouteDef {
  /** Directory name used in output filenames (`<check>/<route>__…`). */
  id: string;
  /** Path on the dev/preview server. */
  path: string;
  /** Plan 07 check id the route hosts (V1 tokens, V3 specimen, …). */
  check: string;
}

/** Routes the generic capture script knows about. Area-owned boards append theirs. */
export const ROUTES: RouteDef[] = [{ id: "tokens", path: "/design/tokens", check: "v1" }];

export type Tier = "commit" | "phase" | "full";

export interface Combo {
  palette: Palette;
  mode: Mode;
  width: Width;
}

const COMMIT_WIDTHS: Width[] = [1440, 390];
const PHASE_WIDTHS: Width[] = [1440];

export function tierCombos(tier: Tier): Combo[] {
  const out: Combo[] = [];
  if (tier === "commit") {
    for (const mode of MODES) {
      for (const width of COMMIT_WIDTHS) {
        out.push({ palette: DEFAULT_PALETTE, mode, width });
      }
    }
    return out;
  }
  if (tier === "phase") {
    for (const palette of PALETTES) {
      for (const mode of MODES) {
        for (const width of PHASE_WIDTHS) {
          out.push({ palette, mode, width });
        }
      }
    }
    return out;
  }
  for (const palette of PALETTES) {
    for (const mode of MODES) {
      for (const width of WIDTHS) {
        out.push({ palette, mode, width });
      }
    }
  }
  return out;
}

/** `tokens__sacred__dark__1440` — the filename convention under web/.visual/. */
export function comboSlug(routeId: string, combo: Combo): string {
  return `${routeId}__${combo.palette}__${combo.mode}__${combo.width}`;
}

/** Membership test that re-narrows a string to the literal union of `list`. */
function isOneOf<T extends string>(list: readonly T[], v: string): v is T {
  // SAFETY: widening `list` to readonly string[] is lossless for an `includes` membership
  // check; the predicate re-narrows to T on return, which only holds because of that check.
  return (list as readonly string[]).includes(v);
}

export const isPalette = (v: string): v is Palette => isOneOf(PALETTES, v);
export const isMode = (v: string): v is Mode => isOneOf(MODES, v);

/** `--width 390` → 390, or null when the value is not one of the matrix widths. */
export function parseWidth(v: string): Width | null {
  for (const width of WIDTHS) {
    if (String(width) === v) return width;
  }
  return null;
}
