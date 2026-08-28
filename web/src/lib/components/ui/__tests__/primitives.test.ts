import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vite-plus/test";

/**
 * Machine guard for the §35–§38 primitive contracts (docs/design-system.md), restyled onto
 * the foundations semantic tokens: §61 forbids hard-coded palette colors in component code,
 * so every file below must style through utilities that resolve via the palette blocks.
 */
// Read via process.cwd() (vitest runs from web/) — Vite rewrites `new URL(..., import.meta.url)`.
const read = (path: string): string => readFileSync(resolve(process.cwd(), path), "utf8");

const button = read("src/lib/components/ui/button/button-variants.ts");
const iconButton = read("src/lib/components/ui/icon-button/icon-button-variants.ts");
const iconButtonSvelte = read("src/lib/components/ui/icon-button/icon-button.svelte");
const input = read("src/lib/components/ui/input/input.svelte");
const textarea = read("src/lib/components/ui/textarea/textarea.svelte");
const label = read("src/lib/components/ui/label/label.svelte");
const tabsList = read("src/lib/components/ui/tabs/tabs-list.svelte");
const tabsTrigger = read("src/lib/components/ui/tabs/tabs-trigger.svelte");
const chip = read("src/lib/components/chip/Chip.svelte");
const metricCard = read("src/lib/components/card/MetricCard.svelte");

const PRIMITIVE_SOURCES = [
  ["button-variants.ts", button],
  ["icon-button-variants.ts", iconButton],
  ["icon-button.svelte", iconButtonSvelte],
  ["input.svelte", input],
  ["textarea.svelte", textarea],
  ["label.svelte", label],
  ["tabs-list.svelte", tabsList],
  ["tabs-trigger.svelte", tabsTrigger],
  ["Chip.svelte", chip],
  ["MetricCard.svelte", metricCard],
] as const;

describe("primitives §61 — semantic tokens only", () => {
  /* Bare white/black, every shaded family (`sky-500`, `slate-900`, …), and alpha forms
     (`bg-black/5`) — semantic tokens never carry a \d{2,3} shade, so no false positives
     (border-b-2 / outline-2 / text-[11px] style classes cannot match). */
  const paletteClass =
    /\b(?:bg|text|border|outline|fill|stroke)-(?:white|black|[a-z]+-\d{2,3})(?:\/\d+)?\b/;

  for (const [name, source] of PRIMITIVE_SOURCES) {
    it(`${name} contains no hard-coded palette color`, () => {
      expect(source.match(/#[0-9a-fA-F]{3,8}\b/), `${name} must not contain hex literals`).toBeNull();
      expect(source.match(paletteClass), `${name} must not use Tailwind palette colors`).toBeNull();
    });
  }
});

describe("button §35 + plan 03 state matrix", () => {
  it("primary pairs primary background with primary-foreground and a token hover", () => {
    expect(button).toContain("bg-primary text-primary-foreground");
    expect(button).toContain("hover:bg-primary-hover");
  });

  it("keeps the legacy accent CTA on the §35 primary role", () => {
    expect(button).toMatch(/\baccent:\s*"bg-primary text-primary-foreground/);
  });

  it("secondary uses surface + border, ghost stays transparent until hover", () => {
    expect(button).toMatch(/\bsecondary:\s*"bg-surface text-foreground border-border/);
    expect(button).toMatch(/\bghost:\s*"bg-transparent text-foreground hover:bg-surface-hover"/);
  });

  it("default size is a 44px target with board pill padding and pill radius", () => {
    expect(button).toContain("min-h-11");
    expect(button).toContain("px-6");
    expect(button).toContain("rounded-pill");
  });

  it("is a pill control — never a block radius (plan 03 geometry)", () => {
    expect(button).not.toContain("rounded-md");
    expect(button).not.toContain("rounded-lg");
  });

  it("focus uses the --focus-ring outline, not a removed indicator", () => {
    expect(button).toContain(
      "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    );
  });

  it("state matrix: active translate, disabled no-events + opacity, base owns focus-visible", () => {
    expect(button).toContain("active:translate-y-px");
    expect(button).toContain("disabled:pointer-events-none disabled:opacity-50");
  });

  it("every variant defines a hover state (focus-visible is owned by the base)", () => {
    const matches = [
      ...button.matchAll(
        /(?:^|\s)"?(?:primary|secondary|ghost|accent|quiet|ink|outline-ink)"?:\s*"([^"]+)"/g
      ),
    ];
    expect(matches.length).toBe(7);
    for (const match of matches) {
      expect(match[1], `${match[1]} must contain a hover: state`).toContain("hover:");
    }
  });
});

describe("icon button §36/§51 + plan 03 geometry", () => {
  it("default is a 44px target", () => {
    expect(iconButton).toContain("size-11");
  });

  it("enforces the Lucide family sizing 18/20/24 with ~1.75 stroke", () => {
    expect(iconButton).toContain("[&_svg]:size-[18px]");
    expect(iconButton).toContain("[&_svg]:size-5");
    expect(iconButton).toContain("[&_svg]:size-6");
    expect(iconButton.match(/\[&_svg\]:stroke-\[1\.75\]/g)).toHaveLength(3);
  });

  it("is a pill control at every size (999px on a square = the board's circular icon button)", () => {
    const radii = iconButton.match(/rounded-(?:sm|md|lg|xl|pill)/g) ?? [];
    expect(radii.length).toBeGreaterThanOrEqual(3);
    expect([...new Set(radii)]).toEqual(["rounded-pill"]);
  });

  it("state matrix: active translate + per-variant hover, focus ring in base", () => {
    expect(iconButton).toContain("active:translate-y-px");
    expect(iconButton).toContain("focus-visible:outline-focus-ring");
    for (const match of iconButton.matchAll(/(?:ghost|secondary|primary):\s*\n?\s*"([^"]+)"/g)) {
      expect(match[1]).toContain("hover:");
    }
  });

  it("requires an accessible name rendered as aria-label", () => {
    expect(iconButtonSvelte).toContain("label: string;");
    expect(iconButtonSvelte).toContain("aria-label={label}");
  });
});

describe("inputs §37 + plan 03 geometry", () => {
  it("input is a 44px PILL field with focus-ring outline and offset", () => {
    expect(input).toContain("h-11");
    expect(input).toContain("border-border bg-surface");
    expect(input).toContain("rounded-pill");
    expect(input).toContain(
      "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    );
    expect(input).not.toContain("ring-3");
  });

  it("textarea is the multiline TEXT BLOCK — 10px block radius, never a pill lozenge", () => {
    expect(textarea).toContain("border-border bg-surface");
    expect(textarea).toContain("rounded-md");
    expect(textarea).not.toContain("rounded-pill");
    expect(textarea).toContain("outline-focus-ring");
  });

  it("label uses the §11 body-s typography role", () => {
    expect(label).toContain("text-body-s");
  });
});

describe("tabs §38 + plan 03 geometry", () => {
  it("list is a plain pill row — no hairline, no container chrome", () => {
    expect(tabsList).toContain("inline-flex items-center gap-2");
    expect(tabsList).not.toContain("border-b");
  });

  it("trigger is a 44px pill with primary-fill active state, hover and focus ring", () => {
    expect(tabsTrigger).toContain("min-h-11");
    expect(tabsTrigger).toContain("rounded-pill");
    expect(tabsTrigger).toContain("hover:bg-surface-hover");
    expect(tabsTrigger).toContain("data-[state=active]:bg-primary");
    expect(tabsTrigger).toContain("data-[state=active]:text-primary-foreground");
    expect(tabsTrigger).toContain("data-[state=active]:hover:bg-primary");
    expect(tabsTrigger).toContain("outline-focus-ring");
  });
});
