'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { parseDuration } = require('../src/duration');

test('whole units', () => {
  assert.equal(parseDuration('45s'), 45);
  assert.equal(parseDuration('2m'), 120);
  assert.equal(parseDuration('1h30m'), 5400);
  assert.equal(parseDuration('1h 30m 15s'), 5415);
  assert.equal(parseDuration('0s'), 0);
  assert.equal(parseDuration('90s'), 90);
});

test('rejects garbage', () => {
  for (const bad of ['', '   ', '10', 'h', '5x', '1h30', 'abc', '1h-5m', null, 42]) {
    assert.throws(() => parseDuration(bad), /invalid duration/, String(bad));
  }
});

test('fractional values', () => {
  assert.equal(parseDuration('1.5h'), 5400);
  assert.equal(parseDuration('0.5m'), 30);
  assert.equal(parseDuration('2.5s'), 3);
  assert.equal(parseDuration('0.25m 10s'), 25);
  assert.equal(parseDuration('1.5h30m'), 7200);
});

test('fractional values round to the nearest second', () => {
  assert.equal(parseDuration('0.01m'), 1);
  assert.equal(parseDuration('0.001h'), 4);
  assert.equal(parseDuration('0.4s'), 0);
});

test('fractions need digits on both sides of the point', () => {
  for (const bad of ['.5h', '1.h', '1..5h', '1.5.5h']) {
    assert.throws(() => parseDuration(bad), /invalid duration/, bad);
  }
});

test('each unit at most once, in h-m-s order', () => {
  for (const bad of ['1h1h', '30m1h', '5s5s', '10s2m', '1h 1h']) {
    assert.throws(() => parseDuration(bad), /invalid duration/, bad);
  }
  assert.equal(parseDuration('1h15s'), 3615);
});
