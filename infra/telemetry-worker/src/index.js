// GoatTech telemetry collector: anonymous "I'm playing" ping + live count only.
// No batch uploads, no hub, no exports, no history endpoints.
// Privacy: the client IP is read only as the key of a transient in-memory rate
// bucket (isolate-local; dropped lazily at minute rollover, hard-capped). It is
// never stored in the database. Logging is disabled in wrangler.toml
// (observability off), verified against the deployed settings after each deploy.
const WINDOW_SEC = 120;       // a session counts while seen within this window
const SESSION_TTL_SEC = 3600; // unseen sessions are deleted after this
const BODY_CAP = 512;
const RATE_LIMIT_PER_MIN = 20; // isolate-local per-IP soft throttle (client uses ~2/min)
const MAX_SESSIONS = 200000;   // admission bound (env.MAX_SESSIONS overrides in tests)
const CACHE_MS = 10000;        // public count cache, isolate-local
const MINUTE_KEEP = 7 * 24 * 3600;
const HOUR_KEEP = 90 * 24 * 3600;
const TOP_VERSIONS = 12;

const buckets = new Map(); // isolate-local only: ip -> n; never persisted
let bucketMin = -1;
let countCache = { at: 0, body: null }; // isolate-local

function limited(ip) {
  const now = Math.floor(Date.now() / 60000);
  // Lazy rollover: keys age out when the NEXT request in a new minute clears the
  // map; an idle isolate can hold them longer, but never past the hard cap.
  if (now !== bucketMin) { buckets.clear(); bucketMin = now; }
  if (buckets.size >= 10000) buckets.clear(); // memory bound
  const n = (buckets.get(ip) || 0) + 1;
  buckets.set(ip, n);
  return n > RATE_LIMIT_PER_MIN;
}

// Reads the body with a hard byte cap, ignoring Content-Length (absent or false
// headers cannot bypass the cap). Returns null when over the cap.
async function readCapped(request, cap) {
  if (!request.body) return "";
  const reader = request.body.getReader();
  const chunks = [];
  let total = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    total += value.byteLength;
    if (total > cap) {
      try { await reader.cancel(); } catch { /* ignore */ }
      return null;
    }
    chunks.push(value);
  }
  const all = new Uint8Array(total);
  let off = 0;
  for (const c of chunks) { all.set(c, off); off += c.byteLength; }
  return new TextDecoder().decode(all);
}

// SQL-side aggregation: D1 returns one row per (version, platform) pair (bounded
// by live sessions in the worst case, far fewer in practice). D1 still scans the
// live-window rows via the seen_at index - that read volume is the real cost and
// is documented in the README, not hidden.
async function countNow(db, nowSec) {
  const { results } = await db
    .prepare(
      `SELECT version, platform, COUNT(*) AS n, SUM(lan) AS lan_n
       FROM sessions WHERE seen_at > ?1 GROUP BY version, platform`
    )
    .bind(nowSec - WINDOW_SEC)
    .all();
  let playing = 0, lan = 0;
  const versions = new Map();
  const platforms = new Map();
  for (const r of results) {
    playing += r.n;
    lan += r.lan_n || 0;
    versions.set(r.version, (versions.get(r.version) || 0) + r.n);
    platforms.set(r.platform, (platforms.get(r.platform) || 0) + r.n);
  }
  const sorted = [...versions.entries()].sort((a, b) => b[1] - a[1]);
  const vobj = Object.fromEntries(sorted.slice(0, TOP_VERSIONS));
  const rest = sorted.slice(TOP_VERSIONS).reduce((s, [, n]) => s + n, 0);
  if (rest > 0) {
    // Pick an overflow label that cannot collide with a genuine version string.
    let label = "other";
    while (Object.prototype.hasOwnProperty.call(vobj, label)) label = `(${label})`;
    vobj[label] = rest;
  }
  return { playing, lan, versions: vobj, platforms: Object.fromEntries(platforms) };
}

async function handlePost(request, env) {
  const text = await readCapped(request, BODY_CAP);
  if (text === null) return new Response(null, { status: 413 });
  let ping;
  try { ping = JSON.parse(text); } catch { return new Response(null, { status: 400 }); }
  const { session, version, platform, lan } = ping ?? {};
  const KEYS = ["session", "version", "platform", "lan"];
  if (
    typeof ping !== "object" || ping === null || Array.isArray(ping) ||
    Object.keys(ping).some((k) => !KEYS.includes(k)) ||
    typeof session !== "string" || !/^[0-9a-f]{32}$/.test(session) ||
    typeof version !== "string" || version.length === 0 || version.length > 64 ||
    typeof lan !== "boolean"
  ) return new Response(null, { status: 400 });
  const nowSec = Math.floor(Date.now() / 1000);
  const platformNorm = platform === "windows" || platform === "macos" || platform === "linux" ? platform : "other";
  const cap = Number(env.MAX_SESSIONS) > 0 ? Number(env.MAX_SESSIONS) : MAX_SESSIONS;

  // Atomic admission: one transactional batch (D1 processes each database's queries
  // one at a time), so concurrent first pings cannot race the cap. Exactly one
  // statement writes: the refresh for a known session, or the insert only while
  // under the cap. The COUNT(*) scan runs only for candidate NEW sessions (the
  // NOT EXISTS key probe short-circuits first); a flood of fake new sessions burns
  // the read quota and fails closed - errors, never charges.
  const [upd, ins] = await env.DB.batch([
    env.DB.prepare(
      `UPDATE sessions SET version = ?2, platform = ?3, lan = ?4, seen_at = ?5 WHERE session = ?1`
    ).bind(session, version, platformNorm, lan ? 1 : 0, nowSec),
    env.DB.prepare(
      `INSERT INTO sessions (session, version, platform, lan, seen_at)
       SELECT ?1, ?2, ?3, ?4, ?5
       WHERE NOT EXISTS (SELECT 1 FROM sessions WHERE session = ?1)
         AND (SELECT COUNT(*) FROM sessions) < ?6`
    ).bind(session, version, platformNorm, lan ? 1 : 0, nowSec, cap),
  ]);
  if ((upd.meta.changes || 0) === 0 && (ins.meta.changes || 0) === 0) {
    return new Response(null, { status: 503 }); // over the admission cap
  }
  // Writes do not invalidate the count cache: up to CACHE_MS of staleness is by design.
  return new Response(null, { status: 204 });
}

async function cron(env) {
  const nowSec = Math.floor(Date.now() / 1000);
  const minute = nowSec - (nowSec % 60);
  const c = await countNow(env.DB, nowSec);
  if (c.playing > 0) {
    await env.DB.prepare(
      `INSERT OR REPLACE INTO minute_totals (minute, playing, lan, versions, platforms)
       VALUES (?1, ?2, ?3, ?4, ?5)`
    ).bind(minute, c.playing, c.lan, JSON.stringify(c.versions), JSON.stringify(c.platforms)).run();
  }
  const hour = minute - (minute % 3600);
  if (minute === hour) {
    const prev = hour - 3600;
    const { results } = await env.DB.prepare(
      `SELECT MAX(playing) AS p, MAX(lan) AS l FROM minute_totals WHERE minute >= ?1 AND minute < ?2`
    ).bind(prev, hour).all();
    if (results[0] && results[0].p != null) {
      await env.DB.prepare(
        `INSERT OR REPLACE INTO hour_totals (hour, playing_max, lan_max) VALUES (?1, ?2, ?3)`
      ).bind(prev, results[0].p, results[0].l).run();
    }
  }
  // Expiry and retention. If these deletes fail (daily quota spent), expired rows
  // remain stored but never inflate the count (every read filters seen_at), and a
  // later successful cron cleans them up.
  await env.DB.batch([
    env.DB.prepare("DELETE FROM sessions WHERE seen_at < ?1").bind(nowSec - SESSION_TTL_SEC),
    env.DB.prepare("DELETE FROM minute_totals WHERE minute < ?1").bind(nowSec - MINUTE_KEEP),
    env.DB.prepare("DELETE FROM hour_totals WHERE hour < ?1").bind(nowSec - HOUR_KEEP),
  ]);
}

export default {
  async fetch(request, env) {
    try {
      const url = new URL(request.url);
      if (url.pathname === "/healthz") {
        if (request.method !== "GET" && request.method !== "HEAD") {
          return new Response(null, { status: 405 });
        }
        return new Response("ok");
      }
      // Transient bucket key only; never stored or logged.
      const ip = request.headers.get("CF-Connecting-IP") || "anon";
      if (limited(ip)) return new Response(null, { status: 429 });
      if (url.pathname === "/v1/live" && request.method === "POST") return await handlePost(request, env);
      if (url.pathname === "/v1/live" && request.method === "GET") {
        const now = Date.now();
        if (countCache.body && now - countCache.at < CACHE_MS) {
          return new Response(countCache.body, { headers: { "Content-Type": "application/json" } });
        }
        const body = JSON.stringify(await countNow(env.DB, Math.floor(now / 1000)));
        countCache = { at: now, body };
        return new Response(body, { headers: { "Content-Type": "application/json" } });
      }
      return new Response(null, { status: 404 });
    } catch {
      // Fail closed: quota or DB trouble surfaces as 503, never as a silent write.
      return new Response(null, { status: 503 });
    }
  },
  async scheduled(event, env) {
    await cron(env);
  },
};
