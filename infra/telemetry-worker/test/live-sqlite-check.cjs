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
const DB = {
  prepare(sql) {
    return { sql, args: [], bind(...args) { this.args = args; return this; },
      async all() { return { results: db.prepare(sql).all(...this.args) }; },
      async run() { return { meta: db.prepare(sql).run(...this.args) }; } };
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
  db.close();
  console.log('PASS real worker + SQLite: admission, capped refresh, live count/LAN/version/OS, strict120s cutoff, stale exclusion, cron expiry and readmission');
})().catch(error => { console.error(error); process.exitCode = 1; });
