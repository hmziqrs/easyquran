import assert from "node:assert/strict";

export function corpusStates(sizes, widths) {
  return [
    ...sizes.flatMap((size) => widths.map((width) => ({ size, width, phone: false }))),
    { size: 48, width: 320, phone: true },
  ];
}

export function corpusRowId(row) {
  return `${row.mode}:${row.offset}:${row.size}:${row.width}:${row.phone}`;
}

export function resumeCorpus(saved, scope, keys, occurrences, allowLegacy = false) {
  assert.equal(saved.engine, "safari");
  assert.deepEqual(saved.errors, []);
  assert.ok(Array.isArray(saved.rows));
  if (saved.scope) assert.deepEqual(saved.scope, scope, "Checkpoint scope changed");
  else assert.ok(allowLegacy, "Legacy checkpoint requires explicit provenance acceptance");
  const states = scope.states;
  const expected = [];
  for (const mode of scope.modes) {
    for (let offset = 0; offset < keys.length; offset += scope.batch_size) {
      for (const state of states) expected.push({ mode, offset, ...state });
    }
  }
  assert.ok(saved.rows.length <= expected.length);
  for (const [index, row] of saved.rows.entries()) {
    assert.equal(corpusRowId(row), corpusRowId(expected[index]), "Checkpoint order changed");
    const batch = new Set(keys.slice(row.offset, row.offset + scope.batch_size));
    assert.equal(row.specimens, batch.size, "Incomplete checkpoint batch");
    assert.equal(row.copied_verses, scope.verify_copy ? batch.size : 0);
    assert.equal(row.private_occurrences, occurrences.filter((item) => batch.has(item.key)).length);
    assert.equal(row.page_overflow, false);
    assert.ok(row.run_overflow.every((key) => ["12:21", "18:110", "56:23"].includes(key)));
    const requested = row.phone ? [390, 844] : [1100, 900];
    assert.deepEqual(row.viewport.requested, requested);
    assert.deepEqual(row.viewport.actual, requested);
    assert.equal(row.viewport.exact, true);
    for (const ending of row.end_geometry) {
      for (const sign of ending.rows) {
        assert.ok(sign.clearance_em >= 0.02);
        assert.ok(Math.abs(sign.center_error_em) <= 0.05);
        assert.ok(sign.previous_line_clearance === null || sign.previous_line_clearance >= 0);
      }
    }
  }
  const count = Math.floor(saved.rows.length / states.length) * states.length;
  return {
    rows: saved.rows.slice(0, count),
    discarded_partial_rows: saved.rows.length - count,
    version: saved.version,
    previous_error: saved.error ?? null,
  };
}
