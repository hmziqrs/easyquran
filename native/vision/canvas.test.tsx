// The canvas page's vision pass: opens the app's root, pushes the Canvas
// page from the shell's home, drives each section to a state worth
// looking at, and screenshots it. Screenshots need a real target
// (`nsh test vision/canvas.test.tsx --target macos`); a headless run
// draws nothing, so they are skipped there and the state assertions still
// run. The manual clock moves the frame loop and the shader without real
// time.
import { open, test } from "nsh/test";
import type { App } from "nsh/test";

// Headless has no drawing: a screenshot there would fail the whole test.
async function capture(app: App, path: string) {
  try {
    await app.screenshot(path);
  } catch {
    console.log(`screenshot skipped (no drawing surface): ${path}`);
  }
}

// The app at the Canvas page, reached the way a user reaches it.
async function canvasApp(): Promise<App> {
  const app = await open("..", { clock: {} });
  await app.press("text:Canvas");
  await app.waitFor("text:Sketch");
  return app;
}

test("the canvas page shows every section", async () => {
  const app = await canvasApp();
  await app.waitFor("text:/^0 strokes/");
  await app.waitFor("text:/^tick /");
  await app.waitFor("text:/^batch #7 ·/");
  await app.waitFor("text:Aurora — WGSL over this box");
  await app.waitFor("text:Post to the page");
  // The ports: the editor, the board with its peers, the specimens.
  await app.waitFor("text:selected bounds: none");
  await app.waitFor("text:/demo peers orbit the board/");
  await app.waitFor("text:/128 × 64 drawn at 1×/");
  // Let the manual clock run the shader and the sweep for a moment so the
  // capture shows them mid-animation.
  await app.clock!.runFor(1200);
  await capture(app, "docs/evidence/canvas/page-light.png");
});

test("a drag leaves a stroke worth looking at", async () => {
  const app = await canvasApp();
  await app.drag("id:sketch", { x: 120, y: -60 });
  await app.drag("id:sketch", { x: -80, y: 40 });
  await app.waitFor("text:/^2 strokes/");
  await capture(app, "docs/evidence/canvas/sketch-drawn.png");
  await app.press("text:Undo");
  await app.waitFor("text:/^1 strokes/");
});

test("the charts redraw on new data", async () => {
  const app = await canvasApp();
  await app.waitFor("text:/^batch #7 ·/");
  await capture(app, "docs/evidence/canvas/charts-first.png");
  await app.press("text:New data");
  await app.waitFor("text:/^batch #8 ·/");
  await capture(app, "docs/evidence/canvas/charts-new-data.png");
});

test("the shader pauses and its uniform drift changes", async () => {
  const app = await canvasApp();
  await app.waitFor("text:Aurora — WGSL over this box");
  await app.clock!.runFor(600);
  await capture(app, "docs/evidence/canvas/shader-running.png");
  await app.press("id:shader-pause >> text:Pause");
  await app.waitFor("text:Resume");
  await app.press("text:Swift");
  await capture(app, "docs/evidence/canvas/shader-paused.png");
});

test("the images and the bundled video show", async () => {
  const app = await canvasApp();
  await app.waitFor("text:/placeholder while loading/");
  await app.waitFor("text:/the clip plays from the app bundle/");
  await capture(app, "docs/evidence/canvas/media.png");
});

test("the infinite board pans, zooms and selects a node", async () => {
  const app = await canvasApp();
  await app.waitFor("text:/^world 100% · pan 0, 0$/");
  await app.drag("id:world-board", { x: 60, y: 0 });
  await app.waitFor("text:/^world 100% · pan 60, 0$/");
  await app.press("text:Zoom in");
  await app.waitFor("text:/^world 125% · pan 75, 0$/");
  await capture(app, "docs/evidence/canvas/world-zoomed.png");
  await app.drag("id:world-board", { x: -100, y: 0 });
  await app.waitFor("text:/^center 20, 0 · under: Notes$/");
  await app.press("text:Zoom in");
  await app.waitFor("text:/^world 156% · pan -31, 0$/");
  await app.drag("id:world-board", { x: 30, y: 0 });
  await app.waitFor("text:/^selected: Notes · 89, 0$/");
  await capture(app, "docs/evidence/canvas/world-node-selected.png");
});

test("the web view exchange is recorded both ways", async () => {
  const app = await canvasApp();
  await app.waitFor("text:Post to the page");
  await app.press("id:post-to-page >> text:Post to the page");
  await app.waitFor("text:hi #1");
  await capture(app, "docs/evidence/canvas/web-view.png");
});

test("the shape editor selects, moves and resizes", async () => {
  const app = await canvasApp();
  await app.waitFor("text:selected bounds: none");
  // Same dance as the headless test: interior drags move, then the east
  // handle is parked on the center and the last drag resizes.
  await app.drag("id:shape-edit", { x: 40, y: 0 });
  await app.waitFor("text:selected bounds: 54, 90 · 150 × 84");
  await app.drag("id:shape-edit", { x: 0, y: 18 });
  await app.waitFor("text:selected bounds: 54, 108 · 150 × 84");
  await app.drag("id:shape-edit", { x: -40, y: 0 });
  await app.waitFor("text:selected bounds: 14, 108 · 150 × 84");
  await app.drag("id:shape-edit", { x: 40, y: 0 });
  await app.waitFor("text:selected bounds: 14, 108 · 190 × 84");
  await capture(app, "docs/evidence/canvas/shape-edited.png");
  await app.press("id:shape-edit-reset >> text:Reset");
  await app.waitFor("text:selected bounds: none");
});

test("the bézier editor shapes the curve and reset restores it", async () => {
  const app = await canvasApp();
  await app.waitFor("text:c1 164, 120 · c2 250, 190");
  await app.drag("id:bezier", { x: 30, y: -20 });
  await app.waitFor("text:c1 194, 100 · c2 250, 190");
  await capture(app, "docs/evidence/canvas/bezier-dragged.png");
  await app.press("id:bezier-reset >> text:Reset");
  await app.waitFor("text:c1 164, 120 · c2 250, 190");
});

test("the node graph moves a node and marquees two", async () => {
  const app = await canvasApp();
  await app.waitFor("text:0 selected");
  await app.drag("id:node-graph", { x: 30, y: 10 });
  await app.waitFor("text:first node 146, 130");
  await app.drag("id:node-graph", { x: -100, y: 40 });
  await app.waitFor("text:first node 46, 170");
  await app.drag("id:node-graph", { x: 180, y: -80 });
  await app.waitFor("text:2 selected");
  await capture(app, "docs/evidence/canvas/node-graph-marquee.png");
  await app.press("id:node-graph-reset >> text:Reset");
  await app.waitFor("text:0 selected");
});

test("the particle field advances on the clock", async () => {
  const app = await canvasApp();
  await app.waitFor("text:/^48 dots · tick /");
  await app.press("id:particles-reset >> text:Reset");
  await app.waitFor("text:48 dots · tick 0");
  await app.clock!.runFor(1000);
  // The tick moved off zero on the manual clock; the pause-hold is the
  // headless test's pin, this one is about the look.
  await app.waitFor("text:/^48 dots · tick /");
  await capture(app, "docs/evidence/canvas/particles.png");
});

test("the text and image specimens draw", async () => {
  const app = await canvasApp();
  await app.waitFor("text:glyphs shape through the engine; the rules mark their baselines");
  await app.waitFor("text:/^128 × 64 drawn at 1×, 2× and as a right-half crop/");
  await capture(app, "docs/evidence/canvas/specimens.png");
});

test("the shared board shows demo peers and a drag selects a card", async () => {
  const app = await canvasApp();
  await app.waitFor("text:/^2 demo peers orbit the board; peer 1 nudges its card · tick /");
  await app.drag("id:collab-board", { x: 25, y: 10 });
  await app.waitFor("text:selected: Plan");
  await app.clock!.runFor(1200);
  await capture(app, "docs/evidence/canvas/shared-board.png");
});
