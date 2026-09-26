'use strict';

const { parseDuration } = require('./duration');

/** Formats whole seconds as the shortest compact duration, e.g. 5400 -> "1h30m". */
function formatDuration(seconds) {
  if (!Number.isInteger(seconds) || seconds < 0) {
    throw new Error(`invalid seconds: ${seconds}`);
  }
  if (seconds === 0) return '0s';
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return `${h ? `${h}h` : ''}${m ? `${m}m` : ''}${s ? `${s}s` : ''}`;
}

/** Normalises any accepted duration string to its canonical form. */
function normalizeDuration(text) {
  return formatDuration(parseDuration(text));
}

module.exports = { formatDuration, normalizeDuration };
