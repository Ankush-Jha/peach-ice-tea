'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { formatDuration, normalizeDuration } = require('../src/format');

test('formats whole seconds', () => {
  assert.equal(formatDuration(0), '0s');
  assert.equal(formatDuration(5400), '1h30m');
  assert.equal(formatDuration(3615), '1h15s');
  assert.equal(formatDuration(59), '59s');
});

test('normalizes accepted input', () => {
  assert.equal(normalizeDuration('90s'), '1m30s');
  assert.equal(normalizeDuration('1h 30m'), '1h30m');
  assert.equal(normalizeDuration('1.5h'), '1h30m');
  assert.equal(normalizeDuration('0.5m'), '30s');
});

test('rejects invalid seconds', () => {
  assert.throws(() => formatDuration(-1), /invalid seconds/);
  assert.throws(() => formatDuration(1.5), /invalid seconds/);
});
