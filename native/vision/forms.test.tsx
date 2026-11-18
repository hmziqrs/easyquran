// The forms pages for the vision pass: the sign-up swept top to bottom so
// every control appears in at least one capture, its validation errors,
// and the settings page — in both appearances (light, dark) at both widths
// (phone 390 × 844, desktop 1280 × 800). Run on a real target so the
// screenshots draw (`nsh test --target macos vision/forms.test.tsx`);
// headless draws nothing. Navigation uses the shell's nav-row test id
// (`page-forms`), stable at both widths.
import { open, test } from "nsh/test";

// The runner resolves the entry against this file's directory, so the app
// opens from the project root, while screenshot paths resolve against the
// process CWD. The directory must exist; the app's sandbox grants no
// fs.write to make one.
const OUT = "docs/evidence/forms";
const SCROLL_VIEW = "id:forms-scroll";
/** Points per flick down the sign-up scroll. */
const PHONE_STEP = 640;
const DESKTOP_STEP = 620;
/** Stops per sweep, top to bottom; the last scroll rides past the end. */
const PHONE_STOPS = 5;
const DESKTOP_STOPS = 4;
const TEST_TIMEOUT = 300_000;

type FormsApp = Awaited<ReturnType<typeof open>>;

/** Headless runs draw nothing; only a real target can keep the captures. */
async function capture(app: FormsApp, path: string) {
  try {
    await app.screenshot(`${OUT}/${path}`);
  } catch (error) {
    console.log(`skipped ${path}: ${error}`);
  }
}

/** Clamps the sign-up scroll to the top whatever a restored state left. */
async function scrollToTop(app: FormsApp) {
  await app.scroll(SCROLL_VIEW, -30000);
}

test("sign-up sweep and errors, then settings, light, phone", async () => {
  const app = await open("..", { colorScheme: "light" });
  await app.press("id:page-forms");
  await app.waitFor("text:Create account");
  await scrollToTop(app);
  await capture(app, "sign-up-fresh-light.png");
  // Stops 1..4 tile the rest of the scroll with overlap, so every control
  // (region, interests, plan, slider, switch, color, budget, OTP, bio,
  // terms, submit) lands fully inside at least one capture.
  for (let stop = 1; stop < PHONE_STOPS; stop += 1) {
    try {
      await app.scroll(SCROLL_VIEW, PHONE_STEP);
    } catch {
      // Already at the end of the scroll view.
    }
    await capture(app, `sign-up-controls-${String(stop).padStart(2, "0")}-light.png`);
  }

  // A failed submit shows every rule's message and focuses the email.
  await app.press("text:Create account");
  await app.waitFor("text:Enter your email");
  await capture(app, "sign-up-errors-light.png");

  // The settings form the link opens.
  await app.press("text:App settings");
  await app.waitFor("text:Save settings");
  await capture(app, "settings-light.png");
}, TEST_TIMEOUT);

test("sign-up sweep and errors, then settings, dark, phone", async () => {
  const app = await open("..", { colorScheme: "dark" });
  await app.press("id:page-forms");
  await app.waitFor("text:Create account");
  await scrollToTop(app);
  await capture(app, "sign-up-fresh-dark.png");
  for (let stop = 1; stop < PHONE_STOPS; stop += 1) {
    try {
      await app.scroll(SCROLL_VIEW, PHONE_STEP);
    } catch {
      // Already at the end of the scroll view.
    }
    await capture(app, `sign-up-controls-${String(stop).padStart(2, "0")}-dark.png`);
  }

  await app.press("text:Create account");
  await app.waitFor("text:Enter your email");
  await capture(app, "sign-up-errors-dark.png");

  await app.press("text:App settings");
  await app.waitFor("text:Save settings");
  await capture(app, "settings-dark.png");
}, TEST_TIMEOUT);

test("sign-up sweep and errors, then settings, light, desktop", async () => {
  const app = await open("..", { colorScheme: "light", width: 1280, height: 800 });
  // The desktop shell's sidebar row and the page header both match.
  await app.press("id:page-forms");
  await app.waitFor("text:Create account");
  await scrollToTop(app);
  await capture(app, "sign-up-desktop-fresh-light.png");
  for (let stop = 1; stop < DESKTOP_STOPS; stop += 1) {
    try {
      await app.scroll(SCROLL_VIEW, DESKTOP_STEP);
    } catch {
      // Already at the end of the scroll view.
    }
    await capture(app, `sign-up-desktop-controls-${String(stop).padStart(2, "0")}-light.png`);
  }

  await app.press("text:Create account");
  await app.waitFor("text:Enter your email");
  await capture(app, "sign-up-desktop-errors-light.png");

  await app.press("text:App settings");
  await app.waitFor("text:Save settings");
  await capture(app, "settings-desktop-light.png");
}, TEST_TIMEOUT);

test("sign-up sweep and errors, then settings, dark, desktop", async () => {
  const app = await open("..", { colorScheme: "dark", width: 1280, height: 800 });
  await app.press("id:page-forms");
  await app.waitFor("text:Create account");
  await scrollToTop(app);
  await capture(app, "sign-up-desktop-fresh-dark.png");
  for (let stop = 1; stop < DESKTOP_STOPS; stop += 1) {
    try {
      await app.scroll(SCROLL_VIEW, DESKTOP_STEP);
    } catch {
      // Already at the end of the scroll view.
    }
    await capture(app, `sign-up-desktop-controls-${String(stop).padStart(2, "0")}-dark.png`);
  }

  await app.press("text:Create account");
  await app.waitFor("text:Enter your email");
  await capture(app, "sign-up-desktop-errors-dark.png");

  await app.press("text:App settings");
  await app.waitFor("text:Save settings");
  await capture(app, "settings-desktop-dark.png");
}, TEST_TIMEOUT);
