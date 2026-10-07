export type { SurahCard } from "../../(marketing)/+page.server";

export type HomeVariant = "cobalt" | "mono" | "slate" | "berry";
export type HomeFont = "onest" | "nunito" | "jetbrains";
export type HomeAccent = "cobalt" | "graphite" | "slate" | "berry";
export type HomeSurface = "plain" | "soft";
export type HomeLayout = "split" | "centered" | "compact" | "soft";
export type HomeMode = "light" | "dark";

export interface HomeVariantDefinition {
  id: HomeVariant;
  name: string;
  description: string;
  font: HomeFont;
  accent: HomeAccent;
  surface: HomeSurface;
  layout: HomeLayout;
}

export interface HomePreviewSettings {
  font: HomeFont;
  accent: HomeAccent;
  surface: HomeSurface;
  mode: HomeMode;
}

const HOME_VARIANT_PRESETS = {
  cobalt: {
    id: "cobalt",
    name: "Cobalt",
    description: "Onest, a clear blue accent, and a split introduction on a flat neutral page.",
    font: "onest",
    accent: "cobalt",
    surface: "plain",
    layout: "split",
  },
  mono: {
    id: "mono",
    name: "Monochrome",
    description: "Centered, spare, and entirely neutral. Type and spacing carry the page.",
    font: "onest",
    accent: "graphite",
    surface: "plain",
    layout: "centered",
  },
  slate: {
    id: "slate",
    name: "Slate",
    description: "JetBrains Mono, compact rows, and a muted slate accent on soft gray.",
    font: "jetbrains",
    accent: "slate",
    surface: "soft",
    layout: "compact",
  },
  berry: {
    id: "berry",
    name: "Berry",
    description: "Onest with a berry accent, gentle panels, and a little more breathing room.",
    font: "onest",
    accent: "berry",
    surface: "soft",
    layout: "soft",
  },
} satisfies Record<HomeVariant, HomeVariantDefinition>;

export const HOME_VARIANTS = Object.values(HOME_VARIANT_PRESETS);

export function homeVariantDefinition(variant: HomeVariant): HomeVariantDefinition {
  return HOME_VARIANT_PRESETS[variant];
}
