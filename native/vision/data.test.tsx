// The data area's vision capture: run on a target that draws, e.g.
// `nsh test vision/data.test.tsx --target macos`. The shots land under
// docs/evidence/data/ for the vision pass to look at.
import { open, test } from "nsh/test";

const DIR = "docs/evidence/data";

test("captures the feed, a detail and the devtools", async () => {
  // The entry is the package root; this file lives one level below it.
  const app = await open("..", { clock: {} });
  await app.press("id:page-data");
  // The first page is still inside its latency: the skeletons show.
  await app.screenshot(`${DIR}/feed-skeletons.png`);
  await app.clock!.runFor(500);
  await app.waitFor("label:Post 1, Shipping a Rust shell");
  await app.screenshot(`${DIR}/feed-loaded.png`);

  await app.press("label:Post 3, Signals over snapshots");
  await app.clock!.runFor(500);
  await app.waitFor("text:revision 1");
  await app.screenshot(`${DIR}/detail-loaded.png`);

  await app.press("role:button[Post]");
  await app.screenshot(`${DIR}/detail-comment-required.png`);

  await app.press("role:button[Back to feed]");
  await app.waitFor("label:Post 3, Signals over snapshots");

  await app.back();
  await app.press("id:page-query-devtools");
  await app.clock!.runFor(300);
  await app.waitFor("text:Fake API");
  await app.screenshot(`${DIR}/devtools.png`);
});
