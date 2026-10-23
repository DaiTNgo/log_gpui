# 03: JSON lines read the `ts` key and epoch-number times

**What to build:** JSON lines from common loggers get a timestamp: zap (`"ts": 1697712000.123`), pino and bunyan (`"time": 1697712000123`), and any line whose time key holds a number rather than a string. Today only string values under `timestamp`, `time`, `@timestamp`, `datetime`, `date` are read, so these logs show an empty TIMESTAMP and are flagged "No timestamps".

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] `ts` is recognised as a time key alongside the existing ones
- [x] A numeric time value is read as epoch seconds or epoch milliseconds, decided by its magnitude (fractional seconds kept)
- [x] The TIMESTAMP cell shows the stored text of the value (the number as written)
- [ ] Such files interleave by time and can be bounded by a Time Range
- [x] Parser tests cover zap, pino and bunyan sample lines

## Comments

- 2026-10-23: Done. `ts` joins the time keys; a numeric value is read as seconds, milliseconds, microseconds or nanoseconds since 1970 by its size (`epoch_seconds`), and a number before 1973 is taken for a duration, not a time. The TIMESTAMP cell shows the number as written. Interleaving follows from the timestamp; Time Range is left unticked until checked in the app.
