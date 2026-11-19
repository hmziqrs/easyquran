// The Rust root tested from TypeScript: `nsh test` runs this file in a
// headless runner it builds with the crate, and `open(".")` starts the root
// (`nsh.json` "root"). The shell area keeps this file current with the root.
import { expect, open, test } from "nsh/test";

test("the root mounts", async () => {
  const app = await open(".", { clock: {} });
  await app.waitFor("text:Home");
  // Home's own cards, not a stub.
  expect(app.texts()).toContain("Continue reading");
});

test("back on Home asks to exit", async () => {
  const app = await open(".", { clock: {} });
  await app.waitFor("text:Home");
  await app.back();
  await app.waitFor("text:Exit EasyQuran?");
  await app.press("role:button[Cancel]");
});
