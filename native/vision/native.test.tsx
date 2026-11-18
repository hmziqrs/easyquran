// The gallery's native controls (`src/ui/native.rs`) and the app's fonts
// (Onest, JetBrains Mono) on a real target, both appearances, at the
// desktop width: AppKit draws the buttons, switch and segmented control on
// macOS. Run on a real target so the screenshots draw:
// `nsh test --target macos vision/native.test.tsx`; headless draws nothing.
import { open, test } from "nsh/test";

const OUT = "docs/evidence/ui/native";
const SUMMARY = "text:Every control of the app's own library, with its states. Hover, press, focus and disable each one.";
const TEST_TIMEOUT = 120_000;

for (const scheme of ["light", "dark"] as const) {
  test(`native controls, ${scheme}, desktop`, async () => {
    const app = await open("..", { colorScheme: scheme });
    await app.setViewport(1280, 800);
    await app.press("label:UI gallery >> nth=0");
    await app.waitFor(SUMMARY);
    await app.scroll("id:gallery-scroll", -30000);
    await app.screenshot(`${OUT}/gallery-top-${scheme}-macos.png`);
    // The native section follows Buttons.
    await app.scroll("id:gallery-scroll", 420);
    await app.waitFor("text:Native presses: 0");
    await app.screenshot(`${OUT}/native-controls-${scheme}-macos.png`);
  }, TEST_TIMEOUT);
}
