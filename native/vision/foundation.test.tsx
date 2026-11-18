// The foundation's vision captures: the Diagnostics page and the central
// error surface, in light and dark. Run on a real target, where
// screenshots exist:
//
//   nsh test vision/foundation.test.tsx --target macos
//
// Assumptions the shell owns: the root (nsh.json "root": "App") mounts the
// central error surface, routes to `foundation::diagnostics_page`, and
// honors the first-run contract (`app.first_run_done` in Storage skips the
// flow). Every screenshot is saved under docs/evidence/foundation/.
import { mkdir } from "fs/promises";
import { open, test } from "nsh/test";
import type { App } from "nsh/test";

const DIR = "docs/evidence/foundation";

async function shot(app: App, name: string) {
  await mkdir(DIR, { recursive: true });
  await app.screenshot(`${DIR}/${name}.png`);
}

async function diagnosticsPage(scheme: "light" | "dark") {
  return open(".", {
    clock: {},
    storage: { "app.first_run_done": "1" },
    colorScheme: scheme,
  });
}

async function capture(name: string, scheme: "light" | "dark") {
  const app = await diagnosticsPage(scheme);
  // The shell's entry to the page: a Diagnostics destination by name.
  await app.press("text:Diagnostics");
  await app.waitFor("text:Capabilities");
  await shot(app, `diagnostics-${name}`);

  // The error surface, raised the way Diagnostics provokes it.
  await app.press("role:button[Send test error]");
  await app.waitFor("text:Test error from Diagnostics. Dismiss it when read.");
  await shot(app, `error-surface-${name}`);

  await app.press("role:button[Dismiss]");
  await app.waitFor("text:/Test error from Diagnostics/", { state: "gone" });
}

test("diagnostics and the error surface in light", async () => {
  await capture("light", "light");
});

test("diagnostics and the error surface in dark", async () => {
  await capture("dark", "dark");
});
