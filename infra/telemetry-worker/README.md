# GoatTech telemetry worker

Anonymous "I'm playing" ping + live count collector for the GoatTech launcher,
running as a Cloudflare Worker + D1 on the free plan. Ping/count only:
no batch uploads, no hub, no exports, no public history.

## Privacy
- The client IP is read only as the key of an isolate-local rate bucket. Bucket
  keys are dropped lazily: when the next request after a minute rollover arrives,
  or when the map hits its 10k cap; an idle isolate can hold them longer. The IP
  is never stored in the database.
- Logging is disabled in wrangler.toml (`[observability] enabled = false`,
  `[observability.logs] enabled = false, invocation_logs = false` - invocation
  logs would record request/response metadata). The deployed settings are read
  back and verified after every deploy: that verification is a deploy gate.
- Ping payload is strictly validated: only session/version/platform/lan, anything
  else (including extra keys) is a 400. Bodies are stream-capped at 512 bytes;
  Content-Length is not trusted.
- Session expiry is best-effort, not a privacy guarantee: under quota exhaustion
  cron deletes can fail and expired rows remain in storage longer; they are never
  served (every read filters `seen_at`) and the next successful cron cleans them.

## Endpoints
- `POST /v1/live` - ping, 204 (503 over the admission cap). `GET /v1/live` - live
  count. `GET /healthz` (GET/HEAD only). Everything else: 404.

## Data and truthful limits (Cloudflare free plan, account-wide)
- Client cadence (source-verified): 1 ping/min while playing + 1 immediate ping on
  LAN-state change + ~1 count GET/min per open launcher. Failure backoff to 10 min.
- D1 writes: 2 per ping (row + seen_at index), cron minute rows <=1440/day,
  expiry/retention deletes ~usage. ~120 writes per player-hour.
- Requests: ~120 per player-hour. The 100k requests/day and 100k writes/day free
  quotas land in the same few-hundred player-hours/day range, SHARED with any
  other Worker on the account. Actual shared headroom is measured and reported
  before deploy; no usable-capacity number is guaranteed.
- D1 reads: count queries scan the live-window rows via the seen_at index
  (SQL GROUP BY; JS sees one row per version-platform pair, worst case one per
  live session). The per-minute cron repeats that scan ~1440x/day, so reads
  scale with continuously-live players; a 10s isolate-local count cache absorbs
  GET bursts. Stale counts up to 10s are by design.
- Admission bound: 200k concurrent sessions (a fuse, not a capacity promise).
  New sessions beyond it get 503; existing sessions keep working. Admission is
  atomic: one transactional batch per ping, and D1 serializes each database's
  queries. A flood of fake new sessions burns the read quota and fails closed.
- The free plan cannot be billed: quota exhaustion means errors until the daily
  reset (00:00 UTC), never charges. No payment method requirement.
- The launcher degrades silently on any failure: a failed count fetch shows no
  number, and failed pings back off.

## Deploy
1. `wrangler d1 create goattech-telemetry`, put the real id in `wrangler.toml`
   (the committed id is a local-dev placeholder), `wrangler d1 execute --file schema.sql`.
2. `wrangler deploy` (staging on workers.dev first; contract re-verified there).
3. In the dashboard: verify observability/Workers Logs OFF for this Worker.
4. Attach the `telemetry.goattech.us` custom domain only after the above pass.
Local runtime note: miniflare's local compatibility predates the pinned
compatibility_date (2026-11-03); deploy-time behavior is verified on workers.dev.
