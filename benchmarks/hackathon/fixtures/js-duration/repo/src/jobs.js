'use strict';

const fs = require('node:fs');
const { parseDuration } = require('./duration');

/**
 * Loads a jobs file ({"jobs": [{"name", "timeout"}]}) and resolves every
 * timeout to whole seconds. Throws, naming the job, on an invalid timeout.
 */
function loadJobs(path) {
  const { jobs } = JSON.parse(fs.readFileSync(path, 'utf8'));
  return jobs.map((job) => {
    try {
      return { name: job.name, timeoutSeconds: parseDuration(job.timeout) };
    } catch (error) {
      throw new Error(`job ${job.name}: ${error.message}`);
    }
  });
}

module.exports = { loadJobs };
