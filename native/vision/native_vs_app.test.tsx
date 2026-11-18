// The Native vs app page (`src/ui/native_vs_app.rs`) on a real target,
// both appearances, at the desktop width: AppKit draws the right column
// on macOS. Run on a real target so the screenshots draw:
// `nsh test --target macos vision/native_vs_app.test.tsx`.
import { open, test } from "nsh/test";

const OUT = "docs/evidence/ui/native-vs-app";
const TEST_TIMEOUT = 180_000;

for (const scheme of ["light", "dark"] as const) {
  test(`native vs app, ${scheme}, desktop`, async () => {
    const app = await open("..", { colorScheme: scheme });
    await app.setViewport(1280, 800);
    await app.press("label:Native vs app >> nth=0");
    await app.waitFor("text:last pressed: none");
    await app.scroll("id:native-scroll", -30000);
    for (let at = 0; at < 9; at++) {
      await app.screenshot(`${OUT}/${scheme}-${at}.png`);
      await app.scroll("id:native-scroll", 620);
    }
  }, TEST_TIMEOUT);
}
