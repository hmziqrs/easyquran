export const meta = {
  name: 'web-comment-audit',
  description: 'MAXIMAL loop: score every comment in web/ 0-100, delete all scoring <99 (only compiler-functional comments survive), repeat full passes until a pass removes zero',
  phases: [
    { title: 'Audit', detail: 'per-folder agents delete every non-functional comment' },
    { title: 'Verify', detail: 'run svelte-check to confirm cleanup broke nothing' },
  ],
}

// ----------------------------------------------------------------------------
// Work units: one folder (or a split of a big folder) per agent. Disjoint sets
// of files so parallel agents never edit the same file.
// ----------------------------------------------------------------------------
const CODE_EXT = '(.ts, .js, .svelte, .css, .scss) — skip .md/.txt/.json/.xml/.html content'

const WORK = [
  { id: 'boot', label: 'boot (flagged)', files: [
    'web/src/lib/boot/analytics.ts',
    'web/src/lib/boot/crash-reporting.ts',
    'web/src/lib/boot/offline-engine.ts',
    'web/src/lib/boot/service-worker.ts',
  ]},
  { id: 'icons', label: 'icon/icons.ts (flagged)', files: [
    'web/src/lib/components/icon/icons.ts',
    'web/src/lib/components/icon/Icon.svelte',
    'web/src/lib/components/icon/index.ts',
  ]},
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
    'web/src/lib/components/footer', 'web/src/lib/components/layout', 'web/src/lib/components/nav',
    'web/src/lib/components/notifications', 'web/src/lib/components/panel',
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
  { id: 'firebase', label: 'lib/firebase', dir: 'web/src/lib/firebase' },
  { id: 'data', label: 'lib/data', dir: 'web/src/lib/data' },
  { id: 'workers', label: 'lib/workers', dir: 'web/src/lib/workers' },
  { id: 'storage', label: 'lib/storage', dir: 'web/src/lib/storage' },
  { id: 'config', label: 'lib/config', dir: 'web/src/lib/config' },
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
// MAXIMAL rubric. Only compiler/linter-functional comments survive; all prose
// (file headers, module docs, JSDoc, why/gotcha/cross-ref notes) is removed.
// ----------------------------------------------------------------------------
const RUBRIC = [
  'You are auditing part of the EasyQuran web/ (SvelteKit + Svelte 5) codebase.',
  'POLICY = MAXIMAL. Comments are noise that poisons context for AI agents. Find EVERY comment, score it',
  '0-100, and DELETE every comment scoring BELOW 99. The survival bar is near-absolute: only comments a',
  'COMPILER / LINTER / RUNTIME needs may remain. ALL prose goes — including explanations that look useful.',
  '',
  '## What counts as a comment',
  '  - JS/TS line:   // ...',
  '  - JS/TS block:  /* ... */   (includes JSDoc /** ... */)',
  '  - Svelte/HTML:  <!-- ... -->',
  '  - CSS:          /* ... */',
  '',
  '## KEEP (score 99-100) — ONLY these survive, verbatim (never reword):',
  '  - Tool pragmas that suppress/direct tooling: // @ts-ignore, // @ts-expect-error, // eslint-disable*,',
  '    // biome-ignore*, // prettier-ignore, // @vite-ignore, // stylelint-disable*, /* eslint-* */.',
  '  - Svelte compiler directives: <!-- svelte-ignore ... -->, <!-- svelte:options ... -->, svelte-bound, etc.',
  '  - Triple-slash references: /// <reference path="..." />, /// <reference types="..." />.',
  '  - License / copyright / legal-notice headers (the actual legal text — NOT a module description).',
  '  - JSDoc a tool CONSUMES for type inference: an @type that genuinely narrows a type, @template,',
  '    @overload, or @svelte-* tags — ONLY when removing it would change emitted types or cause an error.',
  '    (Plain @param / @returns / @description prose is NOT consumed for inference -> REMOVE it.)',
  '',
  '## REMOVE (score < 99) — DELETE ALL of the following, EVERYWHERE, including at the TOP of files:',
  '  - File-header / module doc blocks describing what the file or module does — e.g. a top-of-file block',
  '    like "analytics.ts - ...", "owner.ts - SERVER-ONLY ...", or blocks wrapped in ===/--- borders.',
  '    Delete the ENTIRE block (all lines, including the /* ... */ or // delimiters).',
  '  - Function / method / class / component JSDoc — including @param/@returns/@description that merely',
  '    describe the signature or restate the name.',
  '  - "Why" / intent / business-rule / design-decision comments.',
  '  - Gotcha / pitfall / workaround / race-condition / library-quirk notes (YES, remove these too — MAXIMAL).',
  '  - Cross-references to other files, specs, docs, URLs, or data formats.',
  '  - Inline explanations next to code; section dividers; ASCII decoration; TODO / FIXME / HACK / NOTE.',
  '  - Commented-out code; stale or contradictory comments.',
  '  - Any other descriptive or explanatory prose not explicitly in the KEEP list.',
  '',
  'DEFAULT: if a comment is not in the KEEP list, REMOVE it. When uncertain, REMOVE.',
  '',
  '## HOW TO EDIT (be surgical — never touch code)',
  '  1. Glob/Read each target file.',
  '  2. For each comment scoring < 99, use Edit (old_string -> new_string) to delete ONLY the comment.',
  '     - Whole-line comment (nothing else on the line): remove the entire line; collapse one resulting',
  '       blank line if it is now adjacent to another blank line. Preserve indentation of any code.',
  '     - Comment sharing a line with code: strip just the comment token; keep the code.',
  '     - Multi-line block comment: remove ALL of its lines, including the /* ... */ delimiters.',
  '  3. NEVER delete or alter non-comment code, imports, types, signatures, logic, or markup.',
  '  4. NEVER modify files outside your target list.',
  '  5. Leave no stray /* or */ behind, and no dangling punctuation.',
  '',
  '## OUTPUT',
  'Return a structured result: list EVERY comment you removed (and any notable one you kept) with file,',
  'kind, score, action, reason (<=12 words). If a file has no removable comments, omit it. Be thorough',
  'and honest — do not invent reasons to keep prose, and do not touch functional pragmas.',
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
    'regression introduced by a removed comment (most likely a removed svelte-ignore / @ts-ignore / consumed',
    'JSDoc). Report pass/fail, counts, and every issue with file + line + message. Do NOT fix; just report.',
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
