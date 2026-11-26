// Real worker routes against in-memory SQLite. No network or deployed-service claim.
// Run with Node >=22: node infra/telemetry-worker/test/live-sqlite-check.cjs
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { DatabaseSync } = require('node:sqlite');
const db = new DatabaseSync(':memory:');
db.exec(fs.readFileSync(path.join(__dirname, '..', 'schema.sql'), 'utf8'));
let now = Date.parse('2026-11-26T12:00:00Z');
class Clock extends Date {
  constructor(...args) { super(...(args.length ? args : [now])); }
  static now() { return now; }
}
let failSql = null, faultHits = 0;
const DB = {
  prepare(sql) {
    return { sql, args: [], bind(...args) { this.args = args; return this; },
      async all() { if (failSql && sql.includes(failSql)) { faultHits++; throw Error('synthetic read failure'); } return { results: db.prepare(sql).all(...this.args) }; },
      async run() { if (failSql && sql.includes(failSql)) { faultHits++; throw Error('synthetic write failure'); } return { meta: db.prepare(sql).run(...this.args) }; } };
  },
  async batch(statements) {
    db.exec('BEGIN');
    try {
      const results = [];
      for (const statement of statements) results.push(await statement.run());
      db.exec('COMMIT');
      return results;
    } catch (error) { db.exec('ROLLBACK'); throw error; }
  },
};
const context = { Request, Response, TextDecoder, Uint8Array, URL, Date: Clock, console };
vm.createContext(context);
const source = fs.readFileSync(path.join(__dirname, '..', 'src', 'index.js'), 'utf8');
vm.runInContext(source.replace('export default {', 'globalThis.worker = {'), context);
const env = { DB, MAX_SESSIONS: 2 };
let requests = 0;
async function request(method, body) {
  return context.worker.fetch(new Request('https://test.invalid/v1/live', {
    method, headers: { 'CF-Connecting-IP': String(++requests) },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  }), env);
}
const ping = (session, version, platform, lan) => ({ session: session.repeat(32), version, platform, lan });
async function count(expected) {
  const response = await request('GET');
  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), expected);
}
(async () => {
  assert.equal((await request('POST', ping('a', 'v1', 'windows', false))).status, 204);
  // Below the cap, a known session must refresh without a conflicting insert.
  assert.equal((await request('POST', ping('a', 'v1', 'windows', false))).status, 204);
  assert.equal(db.prepare('SELECT COUNT(*) n FROM sessions').get().n, 1);
  assert.equal((await request('POST', ping('b', 'v2', 'linux', true))).status, 204);
  assert.equal((await request('POST', ping('c', 'v3', 'macos', false))).status, 503);
  assert.equal(db.prepare('SELECT COUNT(*) n FROM sessions').get().n, 2);
  await count({ playing: 2, lan: 1, versions: { v1: 1, v2: 1 }, platforms: { windows: 1, linux: 1 } });
  // Refresh at the cap must update an existing row, not reject or duplicate it.
  now += 11000;
  assert.equal((await request('POST', ping('a', 'v2', 'windows', true))).status, 204);
  await count({ playing: 2, lan: 2, versions: { v2: 2 }, platforms: { windows: 1, linux: 1 } });
  // At119s b remains live. Bypass only the read cache to exercise the exact
  // adjacent SQL boundary at120s without waiting another cache interval.
  now += 108000;
  await count({ playing: 2, lan: 2, versions: { v2: 2 }, platforms: { windows: 1, linux: 1 } });
  now += 1000;
  vm.runInContext('countCache.at = 0', context);
  await count({ playing: 1, lan: 1, versions: { v2: 1 }, platforms: { windows: 1 } });
  now += 11000;
  await count({ playing: 0, lan: 0, versions: {}, platforms: {} });
  // Expired rows do not inflate the public count, then cron actually removes them.
  assert.equal(db.prepare('SELECT COUNT(*) n FROM sessions').get().n, 2);
  now += 3600000;
  await context.worker.scheduled({}, env);
  assert.equal(db.prepare('SELECT COUNT(*) n FROM sessions').get().n, 0);
  assert.equal((await request('POST', ping('c', 'v3', 'macos', false))).status, 204);
  now += 11000;
  await count({ playing: 1, lan: 0, versions: { v3: 1 }, platforms: { macos: 1 } });
  // Distinct version groups on one OS must sum rather than overwrite.
  env.MAX_SESSIONS = 5;
  assert.equal((await request('POST', ping('d', 'v4', 'linux', false))).status, 204);
  assert.equal((await request('POST', ping('e', 'v5', 'linux', false))).status, 204);
  // Unsupported OS values normalize to other before persistence and counting.
  assert.equal((await request('POST', ping('f', 'v6', 'plan9', false))).status, 204);
  assert.equal(db.prepare('SELECT platform FROM sessions WHERE session = ?').get('f'.repeat(32)).platform, 'other');
  now += 11000;
  await count({ playing: 4, lan: 0, versions: { v3: 1, v4: 1, v5: 1, v6: 1 }, platforms: { macos: 1, linux: 2, other: 1 } });
  // Cache is deliberately stale for <10s, then refreshes at exactly10s.
  assert.equal((await request('POST', ping('1', 'v7', 'linux', false))).status, 204);
  await count({ playing: 4, lan: 0, versions: { v3: 1, v4: 1, v5: 1, v6: 1 }, platforms: { macos: 1, linux: 2, other: 1 } });
  now += 9999;
  await count({ playing: 4, lan: 0, versions: { v3: 1, v4: 1, v5: 1, v6: 1 }, platforms: { macos: 1, linux: 2, other: 1 } });
  now += 1;
  await count({ playing: 5, lan: 0, versions: { v3: 1, v4: 1, v5: 1, v6: 1, v7: 1 }, platforms: { macos: 1, linux: 3, other: 1 } });
  // More than12versions must preserve totals, including a real version named other.
  db.exec('DELETE FROM sessions');
  const insert = db.prepare('INSERT INTO sessions VALUES (?, ?, ?, ?, ?)');
  for (let i = 0; i < 14; i++) insert.run(i.toString(16).padStart(32, '0'), i === 0 ? 'other' : `v${String(i).padStart(2, '0')}`, 'linux', i % 2, Math.floor(now / 1000));
  now += 10000;
  const overflow = await (await request('GET')).json();
  assert.equal(overflow.playing, 14);
  assert.equal(overflow.lan, 7);
  assert.deepEqual(overflow.platforms, { linux: 14 });
  assert.equal(Object.keys(overflow.versions).length, 13);
  assert.equal(overflow.versions.other, 1);
  assert.equal(overflow.versions['(other)'], 2);
  assert.equal(Object.values(overflow.versions).reduce((a, b) => a + b, 0), 14);
  // SQL retention keeps the strict boundary and removes only older snapshots.
  db.exec('DELETE FROM sessions; DELETE FROM minute_totals; DELETE FROM hour_totals');
  const second = Math.floor(now / 1000);
  const minuteCutoff = second - 7 * 24 * 3600;
  const hourCutoff = second - 90 * 24 * 3600;
  const minuteInsert = db.prepare('INSERT INTO minute_totals VALUES (?, 1, 0, ?, ?)');
  minuteInsert.run(minuteCutoff - 1, '{}', '{}');
  minuteInsert.run(minuteCutoff, '{}', '{}');
  minuteInsert.run(minuteCutoff + 1, '{}', '{}');
  const hourInsert = db.prepare('INSERT INTO hour_totals VALUES (?, 1, 0)');
  for (const offset of [-1, 0, 1]) hourInsert.run(hourCutoff + offset);
  await context.worker.scheduled({}, env);
  assert.deepEqual(db.prepare('SELECT minute FROM minute_totals ORDER BY minute').all().map(r => r.minute), [minuteCutoff, minuteCutoff + 1]);
  assert.deepEqual(db.prepare('SELECT hour FROM hour_totals ORDER BY hour').all().map(r => r.hour), [hourCutoff, hourCutoff + 1]);
  // At an hour boundary cron snapshots the live minute and rolls up ONLY the
  // previous hour, taking playing/LAN maxima independently.
  db.exec('DELETE FROM sessions; DELETE FROM minute_totals; DELETE FROM hour_totals');
  now = Date.parse('2026-11-27T12:00:00Z');
  const hour = Math.floor(now / 1000);
  for (const id of ['a', 'd', 'e', 'f']) insert.run(id.repeat(32), 'live', 'linux', 1, hour);
  const snapshotInsert = db.prepare('INSERT INTO minute_totals VALUES (?, ?, ?, ?, ?)');
  snapshotInsert.run(hour - 3660, 99, 99, '{}', '{}');
  snapshotInsert.run(hour - 3600, 3, 1, '{}', '{}');
  snapshotInsert.run(hour - 60, 2, 2, '{}', '{}');
  await context.worker.scheduled({}, env);
  const snapshot = db.prepare('SELECT * FROM minute_totals WHERE minute = ?').get(hour);
  assert.equal(snapshot.playing, 4);
  assert.equal(snapshot.lan, 4);
  assert.deepEqual(JSON.parse(snapshot.versions), { live: 4 });
  assert.deepEqual(JSON.parse(snapshot.platforms), { linux: 4 });
  const totals = db.prepare('SELECT * FROM hour_totals').all();
  assert.equal(totals.length, 1);
  assert.equal(totals[0].hour, hour - 3600);
  assert.equal(totals[0].playing_max, 3);
  assert.equal(totals[0].lan_max, 2);
  // Re-running that minute replaces one snapshot, never duplicates it.
  await context.worker.scheduled({}, env);
  assert.equal(db.prepare('SELECT COUNT(*) n FROM minute_totals WHERE minute = ?').get(hour).n, 1);
  // Only the hour boundary rolls up, and empty minutes add no snapshot.
  now += 60000;
  db.exec('DELETE FROM sessions; DELETE FROM hour_totals');
  await context.worker.scheduled({}, env);
  assert.equal(db.prepare('SELECT COUNT(*) n FROM hour_totals').get().n, 0);
  assert.equal(db.prepare('SELECT COUNT(*) n FROM minute_totals WHERE minute = ?').get(hour + 60).n, 0);
  // Stored sessions survive the exact1hour TTL but disappear1second later.
  const ttlNow = Math.floor(now / 1000);
  insert.run('b'.repeat(32), 'ttl', 'linux', 0, ttlNow - 3600);
  insert.run('c'.repeat(32), 'expired', 'linux', 0, ttlNow - 3601);
  await context.worker.scheduled({}, env);
  assert.deepEqual(db.prepare('SELECT version FROM sessions').all().map(r => r.version), ['ttl']);
  now += 1000;
  await context.worker.scheduled({}, env);
  assert.equal(db.prepare('SELECT COUNT(*) n FROM sessions').get().n, 0);
  // Snapshot reads and writes can fail independently of viable cleanup. Exercise
  // actual expired rows, not just a recorded DELETE call, for each failure point.
  for (const fault of ['COUNT(*) AS n', 'INSERT OR REPLACE INTO minute_totals', 'SELECT MAX(playing)', 'INSERT OR REPLACE INTO hour_totals']) {
    now = Date.parse('2026-11-28T12:00:00Z');
    const second = Math.floor(now / 1000);
    db.exec('DELETE FROM sessions; DELETE FROM minute_totals; DELETE FROM hour_totals');
    insert.run('a'.repeat(32), 'current', 'linux', 0, second);
    insert.run('b'.repeat(32), 'expired', 'linux', 0, second - 7200);
    snapshotInsert.run(second - 60, 1, 0, '{}', '{}');
    snapshotInsert.run(second - 8 * 86400, 1, 0, '{}', '{}');
    hourInsert.run(second - 91 * 86400);
    const basicInsert = db.prepare('INSERT INTO basic_daily VALUES (?, ?, ?, ?, ?, ?)');
    basicInsert.run('2026-11-21', fault, 'linux', 'ttt_ms', 'lt500', 1);
    failSql = fault;
    faultHits = 0;
    await context.worker.scheduled({}, env);
    failSql = null;
    assert.equal(faultHits, 1, `fault reached exactly once: ${fault}`);
    assert.deepEqual(db.prepare('SELECT version FROM sessions').all().map(r => r.version), ['current'], fault);
    assert.equal(db.prepare('SELECT COUNT(*) n FROM minute_totals WHERE minute < ?').get(second - 7 * 86400).n, 0, fault);
    assert.equal(db.prepare('SELECT COUNT(*) n FROM hour_totals WHERE hour < ?').get(second - 90 * 86400).n, 0, fault);
    assert.equal(db.prepare('SELECT COUNT(*) n FROM basic_daily WHERE release = ?').get(fault).n, 0, fault);
  }
  db.close();
  console.log('PASS real worker + SQLite: admission, capped refresh, live count/LAN/version/OS, strict120s cutoff, stale exclusion, cron expiry and readmission');
})().catch(error => { console.error(error); process.exitCode = 1; });
