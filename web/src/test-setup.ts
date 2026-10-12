import { afterAll } from "vite-plus/test";

// bits-ui's body-scroll-lock resets `document.body` on a 24ms timer after the
// last lock dies (dialog, popover, and tooltip content all mount one, even when
// never locked). A test file that unmounts such a component leaves that timer
// pending; if it fires after vitest has torn the file's happy-dom environment
// down, `document` no longer exists and the whole run dies on an uncaught
// "document is not defined" (attributed to whichever file is running then).
// Draining one lock-cleanup window at each file's end keeps the timer inside
// the file's own environment, where the body-style reset is a harmless no-op.
afterAll(async () => {
  await new Promise((resolve) => setTimeout(resolve, 30));
});
