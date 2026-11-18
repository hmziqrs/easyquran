// The motion page's demos, captured at their end states: light and dark at
// phone width, and the key sections at desktop width. Screenshots need a
// target that draws, so the vision phase runs this file on one (`nsh test
// vision/motion.test.tsx --target macos|ios`); headless, the steps still
// prove the end states. The shell mounts the page behind its Motion nav
// item (`id:page-motion` is the shell's route id for it). `open` resolves
// entries against this file, so the package is the parent.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";

const EVIDENCE = "docs/evidence/motion";

/** Where the swipe rows sit in frame. */
const TO_SWIPE = 1600;
/** Where the transform card sits in frame. */
const TO_TRANSFORM = 2650;
/** To the page's bottom, where the drop and scrub sections sit. */
const TO_BOTTOM = 4000;

async function motion(scheme: "light" | "dark"): Promise<App> {
  // The entry resolves against this file, so the package is the parent.
  const app = await open("..", { clock: {}, colorScheme: scheme });
  // The home rows can still be settling when the first press lands, and
  // the tap then opens the row under it: pop back and press again.
  for (let at = 0; at < 3; at++) {
    await app.press("id:page-motion");
    try {
      await app.waitFor("text:Motion and gestures", { timeout: 3000 });
      return app;
    } catch {
      await app.back();
    }
  }
  throw new Error("the Motion page never opened");
}

/** The desktop window: 1280 × 800, the width the desktop fit is judged at. */
async function desktop(scheme: "light" | "dark"): Promise<App> {
  const app = await motion(scheme);
  await app.setViewport(1280, 800);
  return app;
}

/** Drops One after Two, with the section scrolled into frame and the
 * rows settled into the new order (the drop's spring runs on the clock). */
async function dropped(app: App): Promise<void> {
  // The drop section is the page's last, so the bottom frame holds its
  // rows and the order line however the scroll settles.
  await app.scroll("id:motion-page-scroll", TO_BOTTOM);
  await app.drag("id:Two", { x: 0, y: 64 });
  await app.waitFor("text:Order: One · Three · Two · Four");
  // The rows' layout spring runs on the platform's real time, not the
  // manual clock: let the 700 ms settle finish before the capture.
  await app.wait(900);
}

test("the spring and tween demo settles at its target", async () => {
  const app = await motion("light");
  await app.press("text:Play to 240");
  await app.waitFor("text:Rest 240 · finished");
  expect(await app.screenshot(`${EVIDENCE}/spring-settled-light.png`)).toBeTruthy();
});

test("the route flip lands on the entering route", async () => {
  const app = await motion("light");
  await app.press("label:Route A — alpha");
  await app.waitFor("text:Route B — beta");
  expect(await app.screenshot(`${EVIDENCE}/route-beta-light.png`)).toBeTruthy();
});

test("the layout reorder records its new order", async () => {
  const app = await motion("light");
  await app.press("role:button[Reverse]");
  await app.waitFor("text:Reversed — order 5 · 4 · 3 · 2 · 1");
  expect(await app.screenshot(`${EVIDENCE}/layout-reversed-light.png`)).toBeTruthy();
});

test("a swipe dismisses its row", async () => {
  const app = await motion("light");
  await app.scroll("id:motion-page-scroll", TO_SWIPE);
  await app.drag("id:Beta", { x: -240, y: 0 });
  await app.waitFor("text:/^Beta dismissed/");
  expect(await app.screenshot(`${EVIDENCE}/swipe-dismissed-light.png`)).toBeTruthy();
});

test("a drop reorders the list", async () => {
  const app = await motion("light");
  await dropped(app);
  expect(await app.screenshot(`${EVIDENCE}/drop-reordered-light.png`)).toBeTruthy();
});

test("the transform card follows a drag", async () => {
  const app = await motion("light");
  await app.scroll("id:motion-page-scroll", TO_TRANSFORM);
  await app.drag("id:transform-card", { x: 40, y: -20 });
  await app.waitFor("text:Drag 45 pt · scale 1.00× · angle 0°");
  expect(await app.screenshot(`${EVIDENCE}/transform-dragged-light.png`)).toBeTruthy();
});

test("the scroll-linked header hides and the list rests", async () => {
  const app = await motion("light");
  await app.scroll("id:header-scroll", 600);
  await app.waitFor("text:At rest at 600");
  expect(await app.screenshot(`${EVIDENCE}/header-scrolled-light.png`)).toBeTruthy();
});

test("the scrubber keeps its playhead where the release left it", async () => {
  const app = await motion("light");
  await app.scroll("id:motion-page-scroll", TO_BOTTOM);
  await app.drag("id:scrub-pad", { x: 80, y: 0 });
  await app.waitFor("text:/^1 grant · scrubbed to/");
  expect(await app.screenshot(`${EVIDENCE}/scrub-granted-light.png`)).toBeTruthy();
});

test("the calm path ends in the same states", async () => {
  const app = await motion("dark");
  await app.press("text:Calm motion on this page");
  await app.press("text:Reveal the drawer");
  await app.waitFor("text:Instant 0");
  await app.press("text:Play to 240");
  await app.waitFor("text:Instant 240");
  expect(await app.screenshot(`${EVIDENCE}/calm-instant-dark.png`)).toBeTruthy();
});

// Desktop width: the same sections at 1280 × 800, light.
test("desktop: the spring and tween demo settles", async () => {
  const app = await desktop("light");
  await app.press("text:Play to 240");
  await app.waitFor("text:Rest 240 · finished");
  expect(await app.screenshot(`${EVIDENCE}/desktop-springs-light.png`)).toBeTruthy();
});

test("desktop: a swipe dismisses its row", async () => {
  const app = await desktop("light");
  await app.scroll("id:motion-page-scroll", TO_SWIPE);
  await app.drag("id:Beta", { x: -240, y: 0 });
  await app.waitFor("text:/^Beta dismissed/");
  expect(await app.screenshot(`${EVIDENCE}/desktop-swipe-light.png`)).toBeTruthy();
});

test("desktop: a drop reorders the list", async () => {
  const app = await desktop("light");
  await dropped(app);
  expect(await app.screenshot(`${EVIDENCE}/desktop-drop-light.png`)).toBeTruthy();
});

test("desktop: the scroll-linked header hides and the list rests", async () => {
  const app = await desktop("light");
  await app.scroll("id:header-scroll", 600);
  await app.waitFor("text:At rest at 600");
  expect(await app.screenshot(`${EVIDENCE}/desktop-header-light.png`)).toBeTruthy();
});

test("desktop: the scrubber keeps its playhead", async () => {
  const app = await desktop("light");
  await app.scroll("id:motion-page-scroll", TO_BOTTOM);
  await app.drag("id:scrub-pad", { x: 80, y: 0 });
  await app.waitFor("text:/^1 grant · scrubbed to/");
  expect(await app.screenshot(`${EVIDENCE}/desktop-scrub-light.png`)).toBeTruthy();
});

// Dark at desktop width: one capture evidences the dark desktop fit.
test("dark desktop: the spring and tween demo settles", async () => {
  const app = await desktop("dark");
  await app.press("text:Play to 240");
  await app.waitFor("text:Rest 240 · finished");
  expect(await app.screenshot(`${EVIDENCE}/desktop-springs-dark.png`)).toBeTruthy();
});

// Dark appearance across the page, phone width.
test("dark: the spring and tween demo settles", async () => {
  const app = await motion("dark");
  await app.press("text:Play to 240");
  await app.waitFor("text:Rest 240 · finished");
  expect(await app.screenshot(`${EVIDENCE}/spring-settled-dark.png`)).toBeTruthy();
});

test("dark: a swipe dismisses its row", async () => {
  const app = await motion("dark");
  await app.scroll("id:motion-page-scroll", TO_SWIPE);
  await app.drag("id:Beta", { x: -240, y: 0 });
  await app.waitFor("text:/^Beta dismissed/");
  expect(await app.screenshot(`${EVIDENCE}/swipe-dismissed-dark.png`)).toBeTruthy();
});

test("dark: a drop reorders the list", async () => {
  const app = await motion("dark");
  await dropped(app);
  expect(await app.screenshot(`${EVIDENCE}/drop-reordered-dark.png`)).toBeTruthy();
});

test("dark: the scroll-linked header hides and the list rests", async () => {
  const app = await motion("dark");
  await app.scroll("id:header-scroll", 600);
  await app.waitFor("text:At rest at 600");
  expect(await app.screenshot(`${EVIDENCE}/header-scrolled-dark.png`)).toBeTruthy();
});

test("dark: the transform card follows a drag", async () => {
  const app = await motion("dark");
  await app.scroll("id:motion-page-scroll", TO_TRANSFORM);
  await app.drag("id:transform-card", { x: 40, y: -20 });
  await app.waitFor("text:Drag 45 pt · scale 1.00× · angle 0°");
  expect(await app.screenshot(`${EVIDENCE}/transform-dragged-dark.png`)).toBeTruthy();
});
