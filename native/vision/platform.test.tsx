// The vision pass over the platform pages: the full capture matrix — both
// pages in light and dark, at phone (390 × 844) and desktop (1280 × 800)
// widths, scrolled phone captures for the cards the fold hides, and zh-CN
// phone captures evidencing the translations. `app.screenshot` draws only on
// a target run — start it with `nsh test --target macos` (or `--target ios`);
// a headless run has no pixels, and the captures report as skipped.
//
// One app per test: appearance switches (`setColorScheme`) are mid-test, but
// a width change remounts the shell, so each width is its own app.
//
// Shell contract: the home page's rows carry the route's key (`page-platform`,
// `page-device`), and back pops the pushed page. The pages' scroll views carry
// `platform-page-scroll` / `device-page-scroll`.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";

const OUT = "docs/evidence/platform";

async function capture(app: App, name: string) {
  const path = `${OUT}/${name}.png`;
  try {
    await app.screenshot(path);
    console.log(`captured ${path}`);
  } catch (error) {
    console.log(`SKIPPED: ${path} (${error})`);
  }
}

test("phone width, light: both pages, plus the cards the fold hides", async () => {
  const app = await open("..", { colorScheme: "light" });

  await app.press("id:page-platform");
  await app.waitFor("Clipboard");
  await capture(app, "platform-en-phone-light");
  // Language card and RTL in frame, then the bottom: the desktop
  // window-and-menus card, the system tab bar and the keyboard card. The
  // second scroll lands at the end; on a short layout it is already there and
  // the step rejects, which for a capture is fine to ride through.
  await app.scroll("id:platform-page-scroll", 1100);
  await capture(app, "platform-en-phone-light-scroll-language");
  try {
    await app.scroll("id:platform-page-scroll", 1400);
  } catch {
    // Already at the end of the scroll view.
  }
  await capture(app, "platform-en-phone-light-scroll-bottom");

  await app.back();
  await app.press("id:page-device");
  await app.waitFor("Pixel ratio");
  await capture(app, "device-en-phone-light");
  // The "Not exposed on 0.5.0" card fully in frame; a short layout that
  // already shows it has no scroll range and the step rejects — fine here.
  try {
    await app.scroll("id:device-page-scroll", 900);
  } catch {
    // Already showing the whole page.
  }
  await capture(app, "device-en-phone-light-scrolled");

  expect(app.texts().length).toBeGreaterThan(0);
});

test("desktop width, dark and light: both pages", async () => {
  const app = await open("..", { colorScheme: "dark", width: 1280, height: 800 });

  await app.press("id:page-platform");
  await app.waitFor("Clipboard");
  await capture(app, "platform-en-desktop-dark");
  await app.setColorScheme("light");
  await capture(app, "platform-en-desktop-light");

  // Steps search the top page of the stack, so pop the platform page before
  // pressing the device row, sidebar visible or not.
  await app.back();
  await app.press("id:page-device");
  await app.waitFor("Pixel ratio");
  await capture(app, "device-en-desktop-light");
  await app.setColorScheme("dark");
  await capture(app, "device-en-desktop-dark");

  expect(app.texts().length).toBeGreaterThan(0);
});

test("phone width in Chinese, dark and light: both pages", async () => {
  const app = await open("..", { colorScheme: "dark" });

  await app.press("id:page-platform");
  await app.waitFor("Clipboard");
  await capture(app, "platform-en-phone-dark");
  await app.back();
  await app.press("id:page-device");
  await app.waitFor("Pixel ratio");
  await capture(app, "device-en-phone-dark");
  await app.back();

  await app.press("id:page-platform");
  await app.waitFor("Clipboard");
  // The language card on the page itself: every string re-renders.
  await app.press("中文");
  await app.waitFor("剪贴板");
  await capture(app, "platform-zh-phone-dark");
  await app.setColorScheme("light");
  await capture(app, "platform-zh-phone-light");

  await app.back();
  await app.press("id:page-device");
  await app.waitFor("像素比");
  await capture(app, "device-zh-phone-light");
  await app.setColorScheme("dark");
  await capture(app, "device-zh-phone-dark");

  expect(app.texts().length).toBeGreaterThan(0);
});
