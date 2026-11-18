// The lists page, captured for the vision pass: every view of the demo
// switcher, in light and dark, at phone and desktop width. Screenshots
// need a real target: `nsh test vision/lists.test.tsx --target macos`
// (the shell's `Lists` entry opens the page).
import { open, test } from "nsh/test";

// Each view's switcher chip (test id) and a text only that view shows.
const VIEWS: [string, string, string][] = [
  ["orders", "Orders", "Recent orders"],
  ["quotes", "Quotes", "3000 rows, in table order. Titles sort."],
  ["tree", "Tree", "Nothing selected"],
  ["calendar", "Calendar", "November 2026"],
  ["grid", "Grid", "Jump to card 1500"],
  ["chat", "Chat", "msg 280"],
  ["carousel", "Carousel", "Slide 1 of 5"],
  ["pager", "Pages", "Page 1 of 50"],
];

async function capture(width: number, height: number, tag: string) {
  const app = await open(".", { width, height, colorScheme: "light", clock: {} });
  await app.press("text:Lists");
  for (const scheme of ["light", "dark"] as const) {
    if (scheme === "dark") {
      await app.setColorScheme("dark");
    }
    for (const [key, label, probe] of VIEWS) {
      await app.press(`id:lists-tab-${key}`);
      await app.waitFor(`text:${label}`);
      await app.waitFor(`text:${probe}`);
      await app.screenshot(`docs/evidence/lists/${tag}-${scheme}-${label.toLowerCase()}.png`);
    }
    await app.setColorScheme("light");
  }
  app.dispose();
}

test("the lists page at phone width, light and dark", async () => {
  await capture(390, 844, "phone");
}, 120_000);

test("the lists page at desktop width, light and dark", async () => {
  await capture(1280, 800, "desktop");
}, 120_000);
