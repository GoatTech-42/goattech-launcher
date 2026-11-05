// Committed regression checks for the telemetry worker's routes and cron,
// run with plain node (no deps): node test/route-check.cjs
// The worker source is vm-run with a DB-call-recording double and a fixed
// clock, so contract branches, cleanup ordering and the retention cutoff are
// exercised against the actual code. SQL aggregation is checked separately
// against node:sqlite when the runtime offers it (node >=22).
const fs = require('fs'), vm = require('vm'), path = require('path'), assert = require('assert/strict');
const src = fs.readFileSync(path.join(__dirname, '..', 'src', 'index.js'), 'utf8');
// The production source must carry no override for the release allowlist; the
// fixture releases below are injected into this test's copy only.
assert(!src.includes('env.PUBLISHED_RELEASES'), 'no env override in the shipped source');
const fixtureSrc = src.replace(
  'const PUBLISHED_RELEASES = [];',
  'const PUBLISHED_RELEASES = ["v1.2.3", "v0.0.0-test"];'
);
assert(fixtureSrc !== src, 'the fixture allowlist was injected');

let now = Date.parse('2026-11-12T23:59:00Z');
class FixedDate extends Date { constructor(...a) { super(...(a.length ? a : [now])); } static now() { return now; } }
const context = { Request, Response, TextDecoder, Uint8Array, URL, Date: FixedDate, console };
vm.createContext(context);
vm.runInContext(fixtureSrc.replace('export default {', 'globalThis.worker = {'), context);

let batches = [], failAll = false, failCount = false;
const DB = {
  prepare(sql) {
    return { sql, args: [],
      bind(...a) { this.args = a; return this; },
      async all() { if (failAll || failCount) throw Error('dbfail'); return { results: [] }; },
      async run() { if (failAll) throw Error('dbfail'); return { meta: { changes: 1 } }; } };
  },
  async batch(ss) { if (failAll) throw Error('dbfail'); batches.push(ss); return ss.map(() => ({ meta: { changes: 1 } })); },
};
const env = { DB };

let ip = 0; const results = [];
async function post(name, body, expected, headers = {}) {
  const before = batches.length;
  const req = new Request('https://test.invalid/v1/basic-stats', { method: 'POST', body, headers: { 'CF-Connecting-IP': String(++ip), ...headers } });
  const r = await context.worker.fetch(req, env);
  assert.equal(r.status, expected, name);
  results.push({ name, status: r.status, writes: batches.length - before });
}
const base = { schema: 'goattech-basic-stats/1', release: 'v1.2.3', os: 'linux', ttt_ms: 'lt500' };

(async () => {
  // Healthy paths.
  await post('valid one metric', JSON.stringify(base), 204);
  await post('valid all metrics', JSON.stringify({ ...base, ttw_ms: 'lt1000', p95_ms: 'lt8_4' }), 204);
  await post('published seam fixture release', JSON.stringify({ ...base, release: 'v0.0.0-test' }), 204);

  // Contract parity: every bucket label the client can send is accepted...
  const TTT = ['lt500', '500_1000', '1000_2000', '2000_4000', '4000_8000', '8000_16000', 'ge16000'];
  const TTW = ['lt1000', '1000_3000', '3000_6000', '6000_12000', '12000_30000', 'ge30000'];
  const P95 = ['lt8_4', '8_4_16_7', '16_7_33_4', '33_4_66_7', '66_7_100_0', 'ge100_0'];
  for (const b of TTT) await post(`ttt ${b}`, JSON.stringify({ ...base, ttt_ms: b }), 204);
  for (const b of TTW) await post(`ttw ${b}`, JSON.stringify({ ...base, ttw_ms: b }), 204);
  for (const b of P95) await post(`p95 ${b}`, JSON.stringify({ ...base, p95_ms: b }), 204);
  for (const os of ['windows', 'macos']) await post(`os ${os}`, JSON.stringify({ ...base, os }), 204);

  // ...and everything outside the contract is refused before any write.
  const bad = [
    ['extra id key', { ...base, id: 'secret' }], ['raw measurement', { ...base, ttt_ms: 123 }],
    ['unknown bucket', { ...base, ttt_ms: '123' }], ['wrong os', { ...base, os: 'other' }],
    ['null metric', { ...base, p95_ms: null }], ['no metric', { schema: base.schema, release: base.release, os: base.os }],
    ['bad schema', { ...base, schema: 'goattech-basic-stats/2' }],
  ];
  for (const [name, obj] of bad) await post(name, JSON.stringify(obj), 400);

  // B1: the allowlist, not a shape - an unpublished tag-shaped string, an
  // ID-carrying suffix and an empty allowlist all refuse before any write.
  await post('unpublished release', JSON.stringify({ ...base, release: 'v9.9.9' }), 400);
  await post('arbitrary unique release suffix', JSON.stringify({ ...base, release: 'v1.2.3-unique-user-12345' }), 400);
  await post('release with extra shape', JSON.stringify({ ...base, release: 'v1.2.3 ' }), 400);
  {
    // The pristine source (empty allowlist) refuses the otherwise-valid
    // report with zero writes - what production does before the first release.
    const pristine = { Request, Response, TextDecoder, Uint8Array, URL, Date: FixedDate, console };
    vm.createContext(pristine);
    vm.runInContext(src.replace('export default {', 'globalThis.worker = {'), pristine);
    const before = batches.length;
    const req = new Request('https://test.invalid/v1/basic-stats', { method: 'POST', body: JSON.stringify(base), headers: { 'CF-Connecting-IP': 'empty-list' } });
    const r = await pristine.worker.fetch(req, env);
    assert.equal(r.status, 400, 'empty allowlist refuses the otherwise-valid report');
    assert.equal(batches.length, before, 'no write before any approved release');
    results.push({ name: 'empty allowlist refuses all', status: r.status });
  }

  // Body cap: exact 512 lands even with a lying Content-Length; 513 does not.
  const text = JSON.stringify(base);
  await post('exact 512 padded', text + ' '.repeat(512 - text.length), 204, { 'Content-Length': '1' });
  await post('513 padded despite false length', text + ' '.repeat(513 - text.length), 413, { 'Content-Length': '1' });

  // DB failure on the write path: 503, never a silent ok.
  failAll = true; await post('DB failure returns 503', text, 503); failAll = false;

  // B3: a failing count query must not suppress privacy cleanup.
  failCount = true;
  await context.worker.scheduled({}, env); // count fails, cleanup must still run
  failCount = false;
  const cleanup = batches.at(-1).map((s) => ({ sql: s.sql, args: s.args }));
  assert(cleanup.some((s) => s.sql.startsWith('DELETE FROM basic_daily')), 'cleanup ran despite count failure');
  results.push({ name: 'cron count failure still cleans up' });

  // B2: inclusive cutoff - at Nov 12 23:59Z the Nov 5 day goes, Nov 6 stays.
  const basic = cleanup.find((s) => s.sql.startsWith('DELETE FROM basic_daily'));
  assert.match(basic.sql, /<="\s*\?1|<= \?1/, 'cutoff is inclusive');
  assert.equal(basic.args[0], '2026-11-05', 'cutoff day is the UTC day seven days back');
  results.push({ name: 'retention cutoff inclusive at 7 days', cutoff: basic.args[0] });
  // A cron one minute into Nov 13 has already deleted the Nov 5 bucket: the
  // oldest kept data is then from Nov 6 00:00, just under 7 days old.
  now = Date.parse('2026-11-13T00:01:00Z');
  await context.worker.scheduled({}, env);
  const cleanup2 = batches.at(-1).find((s) => s.sql.startsWith('DELETE FROM basic_daily'));
  assert.equal(cleanup2.args[0], '2026-11-06', 'rollover moves the cutoff with the UTC day');

  // Rate boundary: 20 accepted, 21st throttled, shared with the other routes.
  let statuses = [];
  const req = () => new Request('https://test.invalid/v1/basic-stats', { method: 'POST', body: text, headers: { 'CF-Connecting-IP': 'same-rate' } });
  for (let i = 0; i < 21; i++) statuses.push((await context.worker.fetch(req(), env)).status);
  assert.equal(statuses[19], 204); assert.equal(statuses[20], 429);
  results.push({ name: 'rate boundary 20/21' });

  // Real SQL when the runtime offers it: schema applies, upsert aggregates,
  // inclusive cutoff deletes the old day and keeps the new one.
  let sqlite;
  try { sqlite = require('node:sqlite'); } catch { /* older node: SQL path unchecked here */ }
  if (sqlite) {
    const db = new sqlite.DatabaseSync(':memory:');
    db.exec(fs.readFileSync(path.join(__dirname, '..', 'schema.sql'), 'utf8'));
    const upsert = db.prepare(`INSERT INTO basic_daily (day, release, os, metric, bucket, count)
       VALUES (?1, ?2, ?3, ?4, ?5, 1)
       ON CONFLICT (day, release, os, metric, bucket) DO UPDATE SET count = count + 1`);
    upsert.run('2026-11-05', 'v1.2.3', 'linux', 'ttt_ms', 'lt500', );
    upsert.run('2026-11-05', 'v1.2.3', 'linux', 'ttt_ms', 'lt500');
    upsert.run('2026-11-06', 'v1.2.3', 'linux', 'ttt_ms', 'lt500');
    const plain = (rows) => rows.map((r) => ({ day: r.day, count: r.count }));
    const before = plain(db.prepare('SELECT day, count FROM basic_daily ORDER BY day').all());
    assert.deepEqual(before, [
      { day: '2026-11-05', count: 2 },
      { day: '2026-11-06', count: 1 },
    ]);
    db.prepare('DELETE FROM basic_daily WHERE day <= ?1').run('2026-11-05');
    assert.deepEqual(plain(db.prepare('SELECT day, count FROM basic_daily').all()), [{ day: '2026-11-06', count: 1 }]);
    results.push({ name: 'sqlite aggregation + inclusive cutoff' });
  }

  console.log(JSON.stringify({ ok: true, results }, null, 2));
})().catch((e) => { console.error(e); process.exitCode = 1; });
