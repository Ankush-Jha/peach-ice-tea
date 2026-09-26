'use strict';

const UNIT_SECONDS = { h: 3600, m: 60, s: 1 };

/**
 * Parses a compact duration such as "1h30m" or "45s" into whole seconds.
 * Whitespace between parts is allowed ("1h 30m"). Throws on anything else.
 */
function parseDuration(text) {
  if (typeof text !== 'string' || text.trim() === '') {
    throw new Error(`invalid duration: ${text}`);
  }
  const compact = text.replace(/\s+/g, '');
  let total = 0;
  let consumed = '';
  for (const match of compact.matchAll(/(\d+)([hms])/g)) {
    total += Number(match[1]) * UNIT_SECONDS[match[2]];
    consumed += match[0];
  }
  if (consumed !== compact) {
    throw new Error(`invalid duration: ${text}`);
  }
  return total;
}

module.exports = { parseDuration, UNIT_SECONDS };
