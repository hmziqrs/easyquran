// The vision pass for the remote screen area: the Rust status page as the
// shell mounts it, in both appearances. Run on a real target, where
// screenshots exist: `nsh test vision/remote_screen.test.tsx --target macos`.
// The shell contract: a navigation item named "Remote screen" mounts
// remote_screen_page() (src/features/remote_screen/mod.rs).
import { open, test } from "nsh/test";

test("the remote screen page in both appearances", async () => {
  const app = await open(".");
  await app.press("text:Remote screen");
  await app.waitFor("text:/Delivery status/");

  await app.screenshot("docs/evidence/remote-screen/page-light.png");
  await app.setColorScheme("dark");
  await app.waitFor("text:/Delivery status/");
  await app.screenshot("docs/evidence/remote-screen/page-dark.png");
});
