# Capture harness

[← Back to the index](../README.md)

`shoot.mjs` produced every screenshot and axe result in this audit. It drives headless Chrome through the Puppeteer copy already installed in `web/node_modules` — nothing extra to install.

## Run

```bash
just quran-fetch                       # once: provision db/ (gitignored)
cd web && PUBLIC_ENV=local pnpm dev --port 5391 --strictPort   # terminal 1
cd rust && cargo run -p ruxlog         # terminal 2 (optional; needed for translations/account)
node docs/research/ux-audit/tools/shoot.mjs docs/research/ux-audit/tools/example-spec.json   # from repo root
```

`out` and compose `src` paths are relative to the directory you run from.

## Spec format

```jsonc
{
  "base": "http://localhost:5391",
  "out": "path/for/output",
  "shots": [
    {
      "name": "area/file-name",           // → out/area/file-name.webp
      "url": "/en/app/al-baqarah",
      "w": 390, "h": 844,                 // viewport (default 1440×900); <600 px → phone emulation, 2× DPR
      "dpr": 3,                           // override device scale
      "mode": "dark", "palette": "sacred",// written to localStorage prefs before load
      "reader": { "lastRead": { "num": 18, "n": 10 } },  // seeds easyquran.reader
      "delay": 2000,                      // ms after load
      "full": true,                       // full-page capture
      "clip": { "x": 0, "y": 0, "width": 700, "height": 60 },
      "selector": ".verse-toolbar > div", // element capture
      "actions": [                        // run in order
        { "click": "button[aria-label='Toggle Sidebar']", "after": 900 },
        { "clickText": "Reading", "tag": "main button" },
        { "type": ["[role=dialog] input", "mercy"] },
        { "press": "Tab" }, { "hover": "…" }, { "wait": 1500 },
        { "eval": "window.scrollTo(0, 400)" }, { "goto": "/app/yours" },
        { "offline": true }, { "reload": true }
      ],
      "annotate": [                       // red boxes + labels drawn before capture
        { "sel": "css selector", "label": "1 · text", "labelSide": "top|right|left|below|below-end", "all": false, "index": 0 },
        { "text": "visible text", "tag": "span, p", "label": "…" },
        { "rect": [x, y, width, height], "label": "…" }   // page coordinates
      ],
      "axe": true,                        // write out/name.axe.json (WCAG 2.2 A/AA + best practice)
      "probe": "JS expression",           // result printed to stdout
      "noShot": true
    },
    { "name": "area/compare", "stack": true, "compose": [   // side-by-side / stacked composite
      { "src": "a.webp", "caption": "Before", "style": "max-height:96px;width:auto" }
    ] }
  ]
}
```

## Notes

- Each shot gets a fresh browser context, so localStorage never leaks between shots.
- Captures default to WebP q82 to keep the folder small (the full set is ≈2.5 MB).
- `axe.min.js` is fetched from cdnjs on demand; drop a copy next to the script to run offline.
