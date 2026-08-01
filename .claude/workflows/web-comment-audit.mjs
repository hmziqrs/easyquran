export const meta = {
  name: 'web-comment-audit',
  description: 'Loop: score every comment in web/ 0-100, remove those scoring <99, repeat full passes until a pass finds zero removable comments',
  phases: [
    { title: 'Audit', detail: 'per-folder agents score & remove comments below 99' },
    { title: 'Verify', detail: 'run svelte-check to confirm cleanup broke nothing' },
  ],
}

// ----------------------------------------------------------------------------
// Work units: one folder (or a split of a big folder) per agent. Disjoint sets
// of files so parallel agents never edit the same file.
// ----------------------------------------------------------------------------
const CODE_EXT = '(.ts, .js, .svelte, .css, .scss) — skip .md/.txt/.json/.xml/.html content'

const WORK = [
  { id: 'stores-reader', label: 'stores/reader', files: [
    'web/src/lib/stores/reader.svelte.ts',
    'web/src/lib/stores/reader-context.svelte.ts',
    'web/src/lib/stores/reader-core.svelte.ts',
    'web/src/lib/stores/reader-persistence.svelte.ts',
    'web/src/lib/stores/reader-session.svelte.ts',
    'web/src/lib/stores/reader-settings.svelte.ts',
    'web/src/lib/stores/reader-share.svelte.ts',
  ]},
  { id: 'stores-other', label: 'stores/other+tests', files: [
    'web/src/lib/stores/notifications.svelte.ts',
    'web/src/lib/stores/prefs.svelte.ts',
    'web/src/lib/stores/quran.svelte.ts',
    'web/src/lib/stores/consent.svelte.ts',
    'web/src/lib/stores/verse-cache.svelte.ts',
    'web/src/lib/stores/annotations.svelte.ts',
    'web/src/lib/stores/__tests__/persistence-decoders.test.ts',
    'web/src/lib/stores/__tests__/reader-persistence.test.ts',
    'web/src/lib/stores/__tests__/reader-reactivity.probe.svelte.ts',
    'web/src/lib/stores/__tests__/reader.test.ts',
  ]},
  { id: 'quran-core', label: 'quran/core+tests', files: [
    'web/src/lib/quran/manifest.ts',
    'web/src/lib/quran/offline.ts',
    'web/src/lib/quran/protocol.ts',
    'web/src/lib/quran/source-plan.ts',
    'web/src/lib/quran/sql.ts',
    'web/src/lib/quran/wire.ts',
    'web/src/lib/quran/worker-client.ts',
    'web/src/lib/quran/wasm-query-runner.ts',
    'web/src/lib/quran/__tests__/sql.test.ts',
    'web/src/lib/quran/__tests__/wire.test.ts',
    'web/src/lib/quran/__tests__/worker-client.test.ts',
  ]},
  { id: 'quran-search', label: 'quran/search', dir: 'web/src/lib/quran/search' },
  { id: 'quran-view', label: 'quran/view', dir: 'web/src/lib/quran/view' },
  { id: 'components-app', label: 'components/app', dirs: [
    'web/src/lib/components/brand', 'web/src/lib/components/card', 'web/src/lib/components/chip',
    'web/src/lib/components/footer', 'web/src/lib/components/icon', 'web/src/lib/components/layout',
    'web/src/lib/components/nav', 'web/src/lib/components/notifications', 'web/src/lib/components/panel',
    'web/src/lib/components/seo', 'web/src/lib/components/status', 'web/src/lib/components/text',
    'web/src/lib/components/tweaks',
  ], extraFiles: ['web/src/lib/components/index.ts'] },
  { id: 'components-ui-a', label: 'ui/a', dirs: [
    'web/src/lib/components/ui/accordion', 'web/src/lib/components/ui/button',
    'web/src/lib/components/ui/input', 'web/src/lib/components/ui/label',
    'web/src/lib/components/ui/separator', 'web/src/lib/components/ui/skeleton',
    'web/src/lib/components/ui/tabs', 'web/src/lib/components/ui/textarea',
  ]},
  { id: 'components-ui-b', label: 'ui/b', dirs: [
    'web/src/lib/components/ui/sheet', 'web/src/lib/components/ui/sidebar',
    'web/src/lib/components/ui/tooltip',
  ]},
  { id: 'data', label: 'lib/data', dir: 'web/src/lib/data' },
  { id: 'firebase', label: 'lib/firebase', dir: 'web/src/lib/firebase' },
  { id: 'workers', label: 'lib/workers', dir: 'web/src/lib/workers' },
  { id: 'storage', label: 'lib/storage', dir: 'web/src/lib/storage' },
  { id: 'config-boot', label: 'config+boot', dirs: ['web/src/lib/config', 'web/src/lib/boot'] },
  { id: 'seo-server', label: 'seo+server', dirs: ['web/src/lib/seo', 'web/src/lib/server'] },
  { id: 'theme-types', label: 'theme+types', dirs: ['web/src/lib/theme', 'web/src/lib/types'] },
  { id: 'routes', label: 'routes', dir: 'web/src/routes' },
  { id: 'root', label: 'root configs', dirs: ['web/scripts'], extraFiles: [
    'web/vite-plugin-quran.ts', 'web/vite.config.ts', 'web/vitest.config.ts', 'web/quran-data-source.ts',
  ]},
]

function describe(u) {
  const parts = []
  if (u.dir) parts.push(`every code file ${CODE_EXT} under ${u.dir}/ (Glob: "${u.dir}/**/*")`)
  if (u.dirs) u.dirs.forEach((d) => parts.push(`every code file ${CODE_EXT} under ${d}/ (Glob: "${d}/**/*")`))
  if (u.files) parts.push(`these exact files: ${u.files.join(', ')}`)
  if (u.extraFiles) parts.push(`plus these exact files: ${u.extraFiles.join(', ')}`)
  return parts.join('\n   - ')
}

// ----------------------------------------------------------------------------
// Rubric + instructions shared by every audit agent.
// ----------------------------------------------------------------------------
const RUBRIC = [
  'You are auditing part of the EasyQuran web/ (SvelteKit + Svelte 5) codebase for USELESS comments.',
  'CONTEXT: Useless or confusing comments poison the context for AI agents AND humans. Your job is to',
  'find EVERY comment, score its usefulness 0-100, and DELETE every comment that scores BELOW 99.',
  'The bar to survive is intentionally extreme: only comments that clearly earn their place stay.',
  '',
  '## What counts as a comment',
  '  - JS/TS line:   // ...',
  '  - JS/TS block:  /* ... */   (includes JSDoc /** ... */)',
  '  - Svelte/HTML:  <!-- ... -->',
  '  - CSS:          /* ... */',
  '',
  '## SCORING RUBRIC (0-100) — score ONLY on "value the code itself cannot express"',
  'KEEP (score 99-100, leave the text VERBATIM, do not reword):',
  '  - Explains WHY: intent, business rule, constraint, or a non-obvious decision the code does not state.',
  '  - References external context: a spec, doc, URL, issue, data-format, or algorithm ("matches Tanzil dump format",',
  '    "per RFC 3986", "see docs/quran-web-delivery.md", "mirrors source X").',
  '  - Documents a workaround, gotcha, pitfall, race condition, or library/framework quirk that would trip a reader.',
  '  - Explains genuinely subtle data-flow / async ordering / Svelte reactivity NOT inferable from the code.',
  '  - A "do not change / do not simplify because X" warning tied to a concrete reason.',
  '',
  'REMOVE (score < 99) — delete the comment text (and its now-empty line):',
  '  - Restates WHAT the code already says ("// increment count", "// return user", "// setup the store").',
  '  - Obvious/tautological, or merely repeats the function / variable / type name.',
  '  - Section-divider banners or ASCII decoration ("// ====== HELPERS ======") with no real navigational value.',
  '  - Commented-out code (dead).',
  '  - Stale, outdated, or that CONTRADICTS the current code.',
  '  - Vague TODO / FIXME / HACK / NOTE with no actionable owner or concrete context.',
  '  - CONFUSING, AMBIGUOUS, or MISLEADING wording — even if well-intentioned, if it does not crisply aid',
  '    understanding, remove it. THIS IS THE PRIMARY REASON FOR THIS CLEANUP.',
  '  - JSDoc / annotations that only restate types already present in the signature.',
  '',
  'DEFAULT ACTION: when uncertain whether a comment is valuable, REMOVE it. (A 99 threshold means removal is',
  'the default; survival requires clearly non-obvious value.)',
  '',
  '## NEVER remove (these are FUNCTIONAL, not real comments — score 100, leave untouched)',
  '  - Svelte compiler directives: <!-- svelte-ignore ... -->, <!-- svelte:options ... -->, svelte-bound, etc.',
  '  - Tool pragmas: // @ts-ignore, // @ts-expect-error, // eslint-disable..., // biome-ignore...,',
  '    // prettier-ignore, // @vite-ignore, /* eslint-... */, // stylelint-disable, etc.',
  '  - Triple-slash references: /// <reference path="..." />, /// <reference types="..." />.',
  '  - JSDoc a tool CONSUMES for inference: an @type that genuinely narrows a tricky type, @template, @overload,',
  '    or @svelte-* tags. (Pure descriptive JSDoc prose is still subject to removal. If removing it would change',
  '    emitted types or introduce a type/a11y/lint error, KEEP it.)',
  '  - License / copyright headers.',
  '  - Anything inside a string literal, template literal, regex, or URL that merely looks like a comment.',
  '',
  '## HOW TO EDIT (be surgical)',
  '  1. Glob/Read each target file.',
  '  2. For each comment scoring < 99, use Edit (old_string -> new_string) to delete ONLY the comment text.',
  '     - If the comment is the only thing on its line, remove the whole line (and collapse one resulting blank',
  '       line if it is clearly noise). Preserve indentation of any remaining code.',
  '     - If code shares the line, strip just the comment token, keep the code.',
  '  3. NEVER delete or alter non-comment code. NEVER touch imports, types, signatures, logic, or markup.',
  '  4. NEVER modify files outside your assigned target list.',
  '  5. After editing, re-Read is not required, but make sure no dangling comment opener/closer or stray punctuation remains.',
  '',
  '## OUTPUT',
  'Return a structured result: list EVERY comment you evaluated (removed OR kept) with file, kind, score,',
  'action, and a <=12-word reason. If a file has no comments worth acting on, simply omit it. A file with zero',
  'removable comments is a SUCCESS — do not invent reasons to delete good comments, and do not invent reasons',
  'to keep bad ones. Be honest and precise.',
].join('\n')

const ROUND_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    unitId: { type: 'string' },
    filesScanned: { type: 'integer', minimum: 0 },
    commentsActedOn: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        properties: {
          file: { type: 'string' },
          kind: { type: 'string', enum: ['line', 'block', 'html', 'css'] },
          score: { type: 'integer', minimum: 0, maximum: 100 },
          action: { type: 'string', enum: ['removed', 'kept'] },
          reason: { type: 'string', maxLength: 160 },
        },
        required: ['file', 'kind', 'score', 'action', 'reason'],
      },
    },
    removedCount: { type: 'integer', minimum: 0 },
    keptCount: { type: 'integer', minimum: 0 },
    notes: { type: 'string' },
  },
  required: ['unitId', 'filesScanned', 'commentsActedOn', 'removedCount', 'keptCount'],
}

const VERIFY_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    passed: { type: 'boolean' },
    errorCount: { type: 'integer', minimum: 0 },
    warningCount: { type: 'integer', minimum: 0 },
    issues: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        properties: {
          file: { type: 'string' },
          line: { type: 'integer' },
          code: { type: 'string' },
          message: { type: 'string' },
        },
        required: ['file', 'message'],
      },
    },
    rawTail: { type: 'string' },
  },
  required: ['passed', 'errorCount', 'warningCount', 'issues'],
}

// ----------------------------------------------------------------------------
// Main: loop full passes until one removes zero comments (or MAX_PASSES hit).
// ----------------------------------------------------------------------------
phase('Audit')
const removalLog = []
let pass = 0
const MAX_PASSES = 4
let grandTotal = 0

while (pass < MAX_PASSES) {
  pass += 1
  const phaseLabel = `Pass ${pass}`
  const results = await parallel(
    WORK.map((u) => () =>
      agent(
        `Audit unit "${u.label}".\n\nYOUR TARGETS:\n   - ${describe(u)}\n\n${RUBRIC}`,
        { schema: ROUND_SCHEMA, label: u.label, phase: phaseLabel }
      ).then((r) => ({ ...r, _unit: u.label }))
    )
  )
  const ok = results.filter(Boolean)
  const roundRemoved = ok.reduce((s, r) => s + (r.removedCount || 0), 0)
  const roundKept = ok.reduce((s, r) => s + (r.keptCount || 0), 0)
  grandTotal += roundRemoved
  const unitsTouched = ok.filter((r) => (r.removedCount || 0) > 0).map((r) => r._unit)
  removalLog.push({ pass, roundRemoved, roundKept, unitsTouched, perUnit: ok })
  log(
    `Pass ${pass}: removed ${roundRemoved} comment(s), kept ${roundKept}, across ${ok.length} units` +
      (unitsTouched.length ? ` (touched: ${unitsTouched.join(', ')})` : '')
  )
  if (roundRemoved === 0) {
    log(`Pass ${pass} found ZERO comments below threshold — codebase is clean. Ending loop.`)
    break
  }
}

// ----------------------------------------------------------------------------
// Verify: confirm the cleanup did not break types / a11y / lint.
// ----------------------------------------------------------------------------
phase('Verify')
const verify = await agent(
  [
    'Run the project type/a11y/lint gate and report whether the comment cleanup caused regressions.',
    'Run this command (use a 300000ms timeout):',
    '   pnpm -C web run check 2>&1 | tail -80',
    'If pnpm is missing, fall back to: cd web && npx svelte-check --tsconfig ./tsconfig.json',
    'The baseline BEFORE this cleanup was CLEAN (0 errors, 0 warnings). So ANY error or warning now is a',
    'regression introduced by a removed comment (most likely a removed svelte-ignore / @ts-ignore / consumed JSDoc).',
    'Report pass/fail, counts, and every issue with file + line + message. Do NOT attempt fixes; just report.',
  ].join('\n'),
  { schema: VERIFY_SCHEMA, label: 'svelte-check', phase: 'Verify' }
)

return {
  passesRun: pass,
  cleanOnLastPass: removalLog[removalLog.length - 1] && removalLog[removalLog.length - 1].roundRemoved === 0,
  grandTotalRemoved: grandTotal,
  perPass: removalLog.map((p) => ({ pass: p.pass, removed: p.roundRemoved, kept: p.roundKept, touched: p.unitsTouched })),
  verify,
}
