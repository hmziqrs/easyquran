// The overlays page for the vision pass: every overlay captured OPEN, in
// both appearances, at phone and desktop width. Run on a real target so
// the screenshots draw (`nsh test --target macos vision/overlays.test.tsx`);
// headless draws nothing. The entry is the repo root (`open("..")`): the
// runner resolves it from this file's directory. Navigation assumes the
// shell exposes the sidebar row `nav-overlays` (its test id).
import { expect, open, test } from "nsh/test";

const OUT = "docs/evidence/overlays";

/** Opens the page and waits for its title. Desktop width has the
 * sidebar's `nav-overlays` row; a phone shows the home index instead, so
 * it presses the row labeled Overlays. */
async function page(scheme: "light" | "dark", width?: number) {
  const app = await open("..", {
    colorScheme: scheme,
    ...(width ? { width, height: 700 } : {}),
  });
  await app.press(width ? "id:nav-overlays" : "label:Overlays");
  await app.waitFor("text:Rename file…");
  return app;
}

for (const scheme of ["light", "dark"] as const) {
  test(`dialog and alerts open, in ${scheme} at desktop width`, async () => {
    const app = await page(scheme, 900);

    // The rename dialog, open with the field focused.
    await app.press("role:button[Rename file…]");
    await app.waitFor("text:Rename file");
    await app.screenshot(`${OUT}/dialog-open-${scheme}.png`);
    await app.press("role:button[Cancel]");

    // The system's alert: the runner answers dialogs the moment they
    // show, so the handler is installed first and the capture is of the
    // flow's outcome; `dialogs()` proves the alert opened, with its title
    // and the destructive and cancel buttons.
    await app.onDialog((dialog) => dialog.buttons.includes("Delete draft") ? "Delete draft" : null);
    await app.press("role:button[Delete draft…]");
    await app.waitFor("text:Deleted the draft");
    expect(app.dialogs().length).toBe(1);
    expect(app.dialogs()[0].title).toContain("Release notes");
    await app.screenshot(`${OUT}/alert-flow-${scheme}.png`);
  });

  test(`sheet, popover and menus open, in ${scheme} at desktop width`, async () => {
    const app = await page(scheme, 900);

    // The sheet as a side panel.
    await app.press("role:button[Notification settings…]");
    await app.waitFor("text:Opened as a right panel");
    await app.screenshot(`${OUT}/sheet-side-${scheme}.png`);
    await app.press("role:button[Done]");

    // The share popover, anchored and open.
    await app.press("id:share-trigger");
    await app.waitFor("text:Share “Q3 roadmap”");
    await app.screenshot(`${OUT}/popover-open-${scheme}.png`);
    await app.press("id:share-trigger");

    // The document menu, open with its sections.
    await app.press("id:actions-trigger");
    await app.waitFor("role:menuitem[Rename]");
    await app.screenshot(`${OUT}/menu-open-${scheme}.png`);
    await app.press("role:menuitem[Rename]");

    // The context menu, at the pointer.
    await app.rightClick("id:file-card");
    await app.waitFor("role:menuitem[Copy]");
    await app.screenshot(`${OUT}/context-menu-open-${scheme}.png`);
    await app.press("role:menuitem[Copy]");
  });

  test(`hover card, tooltip and toasts open, in ${scheme} at desktop width`, async () => {
    const app = await page(scheme, 900);

    // The hover card, open over its trigger.
    await app.hover("id:hover-card-trigger");
    await app.waitFor("role:button[Follow]");
    await app.screenshot(`${OUT}/hover-card-open-${scheme}.png`);
    await app.hover("id:file-card");

    // The tooltip, open over the save chip.
    await app.hover("id:save-chip");
    await app.waitFor("text:Saves the draft (⌘S)");
    await app.screenshot(`${OUT}/tooltip-open-${scheme}.png`);
    await app.hover("id:file-card");

    // Two toasts, one of them with its action.
    await app.press("role:button[Save the draft]");
    await app.press("role:button[Sync now]");
    await app.waitFor("text:Sync failed");
    await app.screenshot(`${OUT}/toasts-open-${scheme}.png`);
  });

  test(`dock, resizable panels and code surfaces, in ${scheme} at desktop width`, async () => {
    const app = await page(scheme, 900);

    // The dock area with its docks open and a drag preview is exercised by
    // the unit tests; here it is captured at rest.
    await app.waitFor("id:tab-main");
    await app.screenshot(`${OUT}/dock-${scheme}.png`);

    // The panels captured in the stepped state, so the capture shows the
    // keyboard path's effect on the split.
    await app.press("role:button[Wider]");
    await app.waitFor("text:First panel at 240 pt");
    await app.press("role:button[Narrower]");
    await app.waitFor("text:First panel at 200 pt");
    await app.screenshot(`${OUT}/panels-${scheme}.png`);

    await app.waitFor("text:Edits: 0");
    await app.screenshot(`${OUT}/editor-${scheme}.png`);
    await app.waitFor("text:+4 −1 in 2 changes");
    await app.screenshot(`${OUT}/diff-${scheme}.png`);
    await app.screenshot(`${OUT}/text-view-${scheme}.png`);
  });

  test(`sheet and popovers at phone width, in ${scheme}`, async () => {
    const app = await page(scheme);

    // The bottom sheet at the tall detent: the detent is chosen first —
    // the sheet covers the detent buttons while it is open — and the
    // marker rows are the visible height effect the detent causes.
    await app.press("role:button[Tall]");
    await app.press("role:button[Notification settings…]");
    await app.waitFor("text:Opened as a bottom panel");
    await app.waitFor("id:sheet-tall-marker");
    await app.screenshot(`${OUT}/sheet-bottom-tall-${scheme}.png`);
    await app.press("role:button[Done]");
    await app.waitFor("text:Detent step 2 of 0–2");

    // The menu as a sheet at the bottom on a phone.
    await app.press("id:actions-trigger");
    await app.waitFor("role:menuitem[Rename]");
    await app.screenshot(`${OUT}/menu-sheet-${scheme}.png`);
  });
}
