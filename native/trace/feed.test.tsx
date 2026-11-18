// Criterion 7's evidence: run `nsh test trace/feed.test.tsx --trace` and
// read the commit lines between the console markers. One like tap should
// cost a handful of mount items, and `nodes` should stay flat across the
// scroll of the recycled feed.
import { expect, open, test } from "nsh/test";

test("a like tap and a feed scroll stay cheap", async () => {
  // The entry is the package root; this file lives one level below it.
  const app = await open("..", { clock: {} });
  await app.press("id:page-data");
  await app.waitFor("label:Post 1, Shipping a Rust shell");
  await app.clock!.runFor(1_000);

  console.log("trace-measure: the like tap's commit lines follow");
  await app.press("role:button[Like post 2]");
  await app.clock!.runFor(400);
  expect(app.texts()).toContain("♥ 11");

  console.log("trace-measure: the scroll's commit lines follow");
  await app.scroll("id:feed-list", 4000);
  await app.clock!.runFor(1_000);
  await app.waitFor("label:Post 11, Shipping a Rust shell");

  // The like survived the scroll: the state lives in the data, not the cell.
  await app.scroll("id:feed-list", -4000);
  await app.waitFor("label:Post 2, Why cells recycle");
  expect(app.texts()).toContain("♥ 11");
});
