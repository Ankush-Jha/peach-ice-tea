# Support fractional durations in job timeouts

Job configs set timeouts with `parseDuration` from `src/duration.js`, e.g. `"1h30m"`. Users keep
writing `"1.5h"` and `"0.5m"`, which are rejected today with `invalid duration`.

Please support fractional values:

- A value may have a fractional part (`1.5h`, `0.25m`, `2.5s`). It needs digits on both sides of
  the point: `.5h` and `1.h` stay invalid.
- The result is still a whole number of seconds; round to the nearest second (`0.01m` is 1 second).

While you are in there, there is a second complaint: `"1h1h"` and `"30m1h"` are accepted and
silently summed. Each unit may appear at most once, and units must appear in the order h, m, s.
Anything else is `invalid duration`.

Everything that works today must keep working, including `formatDuration` in `src/format.js`,
which round-trips through `parseDuration`. The tests in `test/` describe the expected behaviour.
