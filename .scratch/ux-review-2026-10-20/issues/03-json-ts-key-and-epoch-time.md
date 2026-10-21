# 03: JSON lines read the `ts` key and epoch-number times

**What to build:** JSON lines from common loggers get a timestamp: zap (`"ts": 1697712000.123`), pino and bunyan (`"time": 1697712000123`), and any line whose time key holds a number rather than a string. Today only string values under `timestamp`, `time`, `@timestamp`, `datetime`, `date` are read, so these logs show an empty TIMESTAMP and are flagged "No timestamps".

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] `ts` is recognised as a time key alongside the existing ones
- [ ] A numeric time value is read as epoch seconds or epoch milliseconds, decided by its magnitude (fractional seconds kept)
- [ ] The TIMESTAMP cell shows the stored text of the value (the number as written)
- [ ] Such files interleave by time and can be bounded by a Time Range
- [ ] Parser tests cover zap, pino and bunyan sample lines
