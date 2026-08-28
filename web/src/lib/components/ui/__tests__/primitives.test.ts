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

describe("button §35", () => {
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

  it("default size is a 44px target with §35 inline padding and radius token", () => {
    expect(button).toContain("min-h-11");
    expect(button).toContain("px-[18px]");
    expect(button).toContain("rounded-md");
  });

  it("focus uses the --focus-ring outline, not a removed indicator", () => {
    expect(button).toContain(
      "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    );
  });
});

describe("icon button §36/§51", () => {
  it("default is a 44px target", () => {
    expect(iconButton).toContain("size-11");
  });

  it("enforces the Lucide family sizing 18/20/24 with ~1.75 stroke", () => {
    expect(iconButton).toContain("[&_svg]:size-[18px]");
    expect(iconButton).toContain("[&_svg]:size-5");
    expect(iconButton).toContain("[&_svg]:size-6");
    expect(iconButton.match(/\[&_svg\]:stroke-\[1\.75\]/g)).toHaveLength(3);
  });

  it("keeps every size inside the §13 button radius band (no rounded-lg/xl)", () => {
    const radii = iconButton.match(/rounded-(?:sm|md|lg|xl|pill)/g) ?? [];
    expect(radii.length).toBeGreaterThanOrEqual(3);
    expect([...new Set(radii)]).toEqual(["rounded-md"]);
  });

  it("requires an accessible name rendered as aria-label", () => {
    expect(iconButtonSvelte).toContain("label: string;");
    expect(iconButtonSvelte).toContain("aria-label={label}");
  });
});

describe("inputs §37", () => {
  it("input is a 44px field with focus-ring outline and offset", () => {
    expect(input).toContain("h-11");
    expect(input).toContain("border-border bg-surface");
    expect(input).toContain("rounded-md");
    expect(input).toContain(
      "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    );
    expect(input).not.toContain("ring-3");
  });

  it("textarea follows the same field contract multiline", () => {
    expect(textarea).toContain("border-border bg-surface");
    expect(textarea).toContain("rounded-md");
    expect(textarea).toContain("outline-focus-ring");
  });

  it("label uses the §11 body-s typography role", () => {
    expect(label).toContain("text-body-s");
  });
});

describe("tabs §38", () => {
  it("list is a hairline row, not a pill container", () => {
    expect(tabsList).toContain("border-b border-border");
  });

  it("trigger is a 44px target with underline active state and focus ring", () => {
    expect(tabsTrigger).toContain("min-h-11");
    expect(tabsTrigger).toContain("data-[state=active]:border-primary");
    expect(tabsTrigger).toContain("data-[state=active]:text-foreground");
    expect(tabsTrigger).toContain("outline-focus-ring");
  });
});
