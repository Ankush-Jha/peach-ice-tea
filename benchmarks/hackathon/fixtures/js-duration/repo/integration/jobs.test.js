'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { loadJobs } = require('../src/jobs');

function writeJobs(jobs) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'jobs-'));
  const file = path.join(dir, 'jobs.json');
  fs.writeFileSync(file, JSON.stringify({ jobs }));
  return file;
}

test('a real jobs file with fractional and whole timeouts', () => {
  const file = writeJobs([
    { name: 'backup', timeout: '1.5h' },
    { name: 'report', timeout: '0.25m 10s' },
    { name: 'ping', timeout: '45s' },
  ]);
  assert.deepEqual(loadJobs(file), [
    { name: 'backup', timeoutSeconds: 5400 },
    { name: 'report', timeoutSeconds: 25 },
    { name: 'ping', timeoutSeconds: 45 },
  ]);
});

test('a repeated unit names the job that is wrong', () => {
  const file = writeJobs([{ name: 'ok', timeout: '1m' }, { name: 'broken', timeout: '1h1h' }]);
  assert.throws(() => loadJobs(file), /job broken: invalid duration/);
});
