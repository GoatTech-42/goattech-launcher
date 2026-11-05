CREATE TABLE IF NOT EXISTS sessions (
  session TEXT PRIMARY KEY,
  version TEXT NOT NULL,
  platform TEXT NOT NULL,
  lan INTEGER NOT NULL,
  seen_at INTEGER NOT NULL
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS sessions_seen ON sessions(seen_at);
CREATE TABLE IF NOT EXISTS minute_totals (
  minute INTEGER PRIMARY KEY,
  playing INTEGER NOT NULL,
  lan INTEGER NOT NULL,
  versions TEXT NOT NULL,
  platforms TEXT NOT NULL
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS hour_totals (
  hour INTEGER PRIMARY KEY,
  playing_max INTEGER NOT NULL,
  lan_max INTEGER NOT NULL
) WITHOUT ROWID;
-- Once-per-game-session basic performance summaries: daily counts of each
-- coarse bucket label per release and OS. No per-session rows, no IDs, no
-- timestamps finer than the UTC day. Deleted after 7 days by cron.
CREATE TABLE IF NOT EXISTS basic_daily (
  day TEXT NOT NULL,
  release TEXT NOT NULL,
  os TEXT NOT NULL,
  metric TEXT NOT NULL,
  bucket TEXT NOT NULL,
  count INTEGER NOT NULL,
  PRIMARY KEY (day, release, os, metric, bucket)
) WITHOUT ROWID;
