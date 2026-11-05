# GoatTech data collection

This document separates three paths in client source cb11369de93bc0fcc6f8dc3a19632455d76edb77 and collector source 5544e658c28cb11bf339d1acf497847ef0c637db (telemetry-worker branch). These pins identify the disclosure review baseline, not a later release artifact. It is not evidence that a collector is deployed, reachable or configured as described. No verified GoatTech privacy-page URL is established by this document.

- **Player-count ping:** on by default while the game runs, independently of the optional switches. Its destination is GoatTech by default, but a configuration override can change it. See the ping section below.
- **Basic performance summaries:** a separate switch, default-on only for a genuinely new GUI install. Existing installs stay off unless explicitly enabled; CLI entry points do not apply the fresh-install default. Production client and collector release allowlists are currently empty, so no public-release basic summary can be sent or accepted.
- **Four detailed kinds:** performance, crashes, bugs and benchmarks are off unless chosen. They use the outbox and require an explicitly configured upload server; they do not go to GoatTech's collector by default.

## Basic performance summaries

After an eligible session ends normally with a successful process exit, while the launcher remains open and the option remains on, the client may make one bounded send attempt. The measured Horizon game release must be explicitly allowed and there must be at least one valid measurement. A forced quit, crash or unsupported release produces no report. Turning the option off stops collection and clears its session sample; a request already in flight cannot be recalled. A failed report is dropped: no retry or outbox.

The payload is a format marker, measured public game release, OS (Linux/macOS/Windows), and up to three coarse ranges: time to title, time to first world and p95 frame time (only with enough frames). No raw timings, account, install or session ID, hardware inventory or gameplay content. The destination is fixed to GoatTech's basic-stats endpoint; `HORIZON_TELEMETRY_URL` can veto these sends with `off`/`none`/`0`, but cannot redirect them.

The reviewed collector source accumulates each accepted metric range into a UTC-day count by release and OS. These are report contributions, not unique-player or deduplicated session counts. The cleanup removes the UTC day at or before the seven-day cutoff on the next successful scheduled run: contributions normally become eligible roughly six to seven days after arrival. Cron delays or database/quota outages can extend storage; seven days is not a guaranteed maximum. No public basic-summary history endpoint exists.

Before enabling production reports, the release owner must approve matching client/collector release lists and verify that the deployed collector, logging settings and retention match the disclosure. Empty lists and an unavailable endpoint do not prove that no data was collected historically.

## Detailed optional reports

The rest of the detailed-report sections describe the four opt-in kinds and their upstream-compatible protocol, not basic summaries.

## Consent

`telemetry/consent.json` in the game folder (`HORIZON_HOME`, else `horizon` in the platform data folder):

```json
{ "version": 2, "answered": 1792060800, "choices": { "performance": true, "crashes": true, "bugs": false, "benchmarks": false }, "install_id": "3f2c…" }
```

- `version` is the consent version the player answered (`CONSENT_VERSION`). It goes up when what is collected changes. The launcher then asks again, and until the player answers nothing new is kept or sent under the switches; the always-on "I'm playing" ping (below) is unchanged by the answer.
- `answered` is when they answered, in seconds since 1970 (UTC).
- `install_id` is 128 random bits, made the first time the player says yes to anything. It ties their items together so they can be deleted, and nothing else. "Delete my data" asks the server to delete everything under it, then replaces it.

For these four detailed kinds only, outbox collection and sending require consent:

- `outbox::push` keeps an item only if its kind is allowed.
- `upload::flush` reads the consent again before sending and deletes any waiting item whose kind the player has turned off since, so with these four switches off it opens no detailed-upload connection. The independent ping and basic-summary paths are not governed by these switches.

`tests/consent_gating.rs` checks this against a local mock server that counts connections.

## What detailed reports contain

Every item has `day` (days since 1970, UTC: no finer time), `source` (`launcher` or `game`), `version` (the release), `kind` and `data`.

| Kind | Switch | `data` | Largest | A day |
|---|---|---|---|---|
| `perf` | performance | `system`, `settings` (render and simulation distance, max FPS, vsync, fullscreen, graphics mode), `seconds` played, `frames` and `ticks` summaries, `peak_ram_mb`, `to_title_ms`, `to_world_ms`, `slow_frames` | 4 KiB | 12 |
| `crash` | crash reports | `system`, `error`, `stack` (one frame a line), `thread`, the graphics and settings details, all scrubbed. No log, no world, no server | 32 KiB | 5 |
| `bug` | bug reports | `system`, the player's `text` (up to 4000 characters), and the `log` only if they add it after reading it | 80 KiB | 5 |
| `benchmark` | benchmark uploads | the benchmark report as `docs/benchmark-report.md` defines it, as it is, with its own schema field | 128 KiB | 10 |

`system` describes the machine, with what can explain its performance. A field the OS doesn't give is empty or 0:

| Field | What | Example |
|---|---|---|
| `os`, `os_version`, `arch` | the OS, its version, the CPU architecture | `linux`, `Fedora Linux 43`, `x86_64` |
| `cpu`, `threads`, `cores` | the CPU's brand name (on Windows the registry's; its family and model, `PROCESSOR_IDENTIFIER`, only where the registry has none), its threads and physical cores | `13th Gen Intel(R) Core(TM) i7-13700KF`, 24, 16 |
| `cpu_base_mhz`, `cpu_boost_mhz` | base and boost clock, where the OS gives them (not on Apple silicon) | 3400, 5400 |
| `ram_gb` | installed memory, rounded to the GiB | 32 |
| `gpu`, `gpu_vendor`, `gpu_kind` | the GPU the game draws with, its maker, `integrated` or `discrete` | `NVIDIA GeForce RTX 4080`, `nvidia`, `discrete` |
| `vram_mb` | a discrete GPU's own memory, rounded up to 256 MiB | 16384 |
| `driver`, `backend` | the driver version and the graphics API (`Vulkan`, `Metal`, `Dx12`) | `580.95.05`, `Vulkan` |
| `storage` | the drive under the game folder: `nvme`, `ssd` or `hdd` | `nvme` |
| `display` | the screen the window opened on: `width`, `height` (physical pixels), `refresh_hz`, `scale` | 2560, 1440, 165, 1.0 |
| `power`, `form` | on mains or battery (`ac`, `battery`), `laptop` or `desktop` | `ac`, `desktop` |

The launcher's own items (bug reports sent from it) have no GPU or screen: only the game knows them. The memory given to the game is not a setting in Horizon (no Java heap): `peak_ram_mb` is what it used. Benchmark reports carry the same facts in their `hardware` (`docs/benchmark-report.md`).

No serial number, host name, user name, MAC address or path is read. Settings → Data collection → "View what would be sent" and `horizon-launcher data show` print every field as it would leave this machine.

A summary (`frames`, `ticks`) is the count, the 50th, 95th and 99th percentile and the maximum in milliseconds (to the tenth), and 14 bucket counts: under 4 ms, 4–6, 6–8, 8–10, 10–12, 12–14, 14–16.7, 16.7–20, 20–25, 25–33.3, 33.3–50, 50–100, 100–250, and 250 ms and more. Raw frame times never leave: `Timings` records them in tenths of a millisecond (20 KB however long the session) and only its summary is kept. A perf item is under 1 KiB.

`slow_frames` says what froze: the frames of 250 ms or more while playing, counted by the step of the frame they spent most of their time in. A step is a span of the render thread (`spans.rs`), named in the code and the same in every game of a release: the slowest outer span, then the span inside it that took at least half of it, as a path (`renderer/r_uploads`, `tick/tick_mobs`; empty when no step was timed). Each entry has the step, its frame count, the longest of those frames and the longest the step took. At most 16 entries; later steps count under `other`. With performance statistics on, the render thread times its steps (wall time only, two clock reads a span); with them off it times nothing. Most sessions have no slow frame and send no `slow_frames`.

Automatic report bodies exclude account names or UUIDs, world names, server or LAN addresses, chat, file paths with the user's name and IP addresses; review any bug-report text or attached log before sending. `scrub::Scrubber` replaces the user folder in paths (`/Users/<user>/…`, `C:\Users\<user>\…`), the user name, e-mail and IP addresses and UUIDs. It also replaces the terms the caller passes (the world's name, the server's address, account names). No IP address is added to the report body. Network requests still expose IP information to the receiving service; source-level payload scrubbing is not a provider-wide no-logs guarantee.

## Storage and sending

- `telemetry/outbox/` holds one JSON file per item, `<day>-<kind>-<random>.json`. It never holds more than 200 items or 2 MiB, and the oldest go first. `kept.json` counts today's items per kind against the caps above.
- The launcher sends the outbox when it starts and when it closes. Items go as one JSON batch (`{"schema":1,"install_id":…,"items":[…]}`, up to 64 items and 1 MiB), compressed with zstd: `POST /v1/batch` with `Content-Encoding: zstd`. A day's perf summaries are under 2 KB on the wire.
- After a failure the launcher waits a minute, doubling up to a day, and sends at most 8 batches a day. A batch the server refuses with a 4xx (not 429) is dropped.
- No detailed-upload server is contacted by default in this build: no detailed outbox report is sent unless `HORIZON_TELEMETRY_URL` explicitly names a server. The upstream server is `https://telemetry.playhorizon.dev` (`DEFAULT_ENDPOINT`), on Railway in the EU (`europe-west4`, Amsterdam); setting the variable to it (or any other server) opts uploads in. The ping goes to GoatTech by default, and basic summaries have their own fixed GoatTech endpoint and release gate; neither uses this outbox upload-server setting. `HORIZON_TELEMETRY_URL=off` (or `none`, `0`) is the explicit off switch for everything, ping included. Tests, CI and the benchmark and test harnesses set `off`.
- What is sent still depends on the switches: with all four detailed switches off the detailed outbox stays empty. The ping and basic-summary switch are separate.
- `SCHEMA` versions the format. New optional fields keep it. A field that changes meaning or goes bumps it, and the server keeps reading the older ones.

## The "I'm playing" ping

The current client source enables the player-count ping independently of the four optional-report switches. The launcher's first-run question and Settings → Data collection show it as an "Always on" row next to the switches. Players who answered the question before the ping existed see the same text once on the Play page. Upstream's privacy page is not the retention authority for GoatTech's collector.

In this GoatTech build the ping and the count go to GoatTech's own collector, `telemetry.goattech.us` (`infra/telemetry-worker`, a small Cloudflare Worker), by default; detailed outbox uploads use a separate, unset-by-default endpoint and go only where explicitly configured. The reviewed collector source accepts the ping and basic summaries and serves the live count: no detailed batch uploads, hub, exports or public history endpoint. `HORIZON_TELEMETRY_URL=off` still turns the ping and count off; any other value of it overrides the address for both.

While the game runs (not a dedicated server, a benchmark or a run that quits by itself), a thread of its own (`crates/horizon-client/src/heartbeat.rs`) sends `POST /v1/live` once a minute. It also sends one at once when a world is opened to or closed from LAN or friends:

```json
{ "session": "9b1c…", "version": "v2026.10.22", "platform": "macos", "lan": false }
```

- `session` is 128 random bits made when the game starts. It is kept only in the game's memory and is not the install ID, so the payload supplies no stable identifier linking two sessions. This is not a guarantee against inference from network metadata or small aggregate groups.
- `version` is the release, `platform` is `linux`, `macos`, `windows` or `other`, and `lan` says whether a LAN or play-with-friends world is open.
- Nothing else: no account, install ID, world, server, settings or hardware. The GoatTech worker validates the ping shape before storage (`handlePost`); `live::Ping::check` defines client-side validation.

The collector keeps sessions in its database without the sender's address - the ping holds no IP address, and the address is used as an isolate-local in-memory rate-limit key, not a database field. Keys clear on a later request after minute rollover or at the bucket cap; an idle worker can retain them longer. Source configuration disables Worker observability and invocation logs, but deployed settings and platform-wide handling are not established by this document. A session counts while its last ping is under 2 minutes old. Its row (the random number, version, OS, LAN flag and last-ping time - nothing that names a player) is removed by a cleanup that runs about every minute against a one-hour age, so a row can persist up to roughly an hour after the pings stop, and Cloudflare quota can delay cleanup. Beyond those session rows the collector keeps no history of who played. `GET /v1/live` gives the live count, cached up to 10 seconds on the collector; versions past the 12 most played are summed under an overflow label (`other`, shifted if a real version has that name):

```json
{ "playing": 1243, "lan": 87, "versions": { "v2026.10.22": 1102, "other": 141 }, "platforms": { "linux": 301, "macos": 238, "windows": 704 } }
```

The launcher reads it when it opens and every minute after, and shows "N playing now" on the Play page. It shows nothing when the collector can't be reached. `HORIZON_TELEMETRY_URL=off` turns both off; `HORIZON_NO_HEARTBEAT=1` keeps a single automated run out of the count.
Once a minute the upstream server writes down only those totals (playing, LAN, per platform; no session, version or address) for the history at [playhorizon.dev/stats](https://playhorizon.dev/stats): every minute for 7 days, then per hour (average and peak) for 90 days, then per day (average and peak) for good. `GET /v1/live/history?range=24h|7d|30d|all` serves it (`&series=official`: the official server's count instead, below). The reviewed GoatTech collector source instead schedules minute totals (playing, LAN, version and platform breakdowns) for deletion after 7 days and hourly player/LAN maxima after 90 days, with no longer-term rollup or public history endpoint. Cleanup can be delayed by failures. The following history response and server behavior are upstream protocol documentation, not GoatTech endpoints or proof of a deployed policy:

```json
{ "source": "players", "range": "24h", "step": 300, "from": 1793404800, "to": 1793491260,
  "series": [[1793404800, 41.2, 44], [1793405100, 43.0, 47]],
  "peak": { "playing": 1243, "at": 1793480400 }, "all_time": { "playing": 1243, "at": 1793480400 },
  "latest": { "playing": 1190, "at": 1793491200 },
  "platforms": { "linux": 290.5, "macos": 230.1, "windows": 690.4 } }
```

`series` holds `[start, average, peak]` per point (5 minutes for 24h, an hour for 7d, 3 hours for 30d, a day for all; UTC Unix seconds), `peak` the range's highest minute, `all_time` the highest ever, `latest` the newest minute, `platforms` the range's average per platform.

The server also asks the official server (`horizon_telemetry::server_status::OFFICIAL_STATUS_ADDRESS`, its address with the port its SRV record gives, or `HORIZON_TELEMETRY_OFFICIAL_SERVER`) for its player count every minute, with the server-list status ping every client sends to show a server in its Multiplayer list, and keeps it the same way (`series=official`, playing only; a minute it didn't answer stays empty). Its TPS and MSPT come later, once its admin API is reachable from outside the machine.

The launcher reads the count when it opens and every minute after, and shows "N playing now" on the Play page (the collector's count, up to about 10 seconds stale), with thousands separators; clicking it opens playhorizon.dev/stats. Beside it, "M on the official SMP" is the official server's own count, from the same status ping the launcher sends once a minute (3-second timeout, nothing shown when it doesn't answer); clicking it opens the Servers page. It shows nothing when the server can't be reached. `HORIZON_TELEMETRY_URL=off` turns both off; `HORIZON_NO_HEARTBEAT=1` keeps a single automated run out of the count.

## Upstream-compatible detailed server (not GoatTech collector)

- `POST /v1/batch`
- `DELETE /v1/installs/<install_id>`: deletes everything sent under that ID.
- `GET /v1/aggregate`: the anonymous totals the website's results page reads. Its `breakdowns` split the play sessions of the period by hardware class (below), each class with its installs, sessions, hours and mean p50 and p99 frame times. Its `slow_frames` list what froze, per release and step: the slow frames, the sessions and installs that had one, and the longest frame and step, for steps at least as many installs had as the groups need.
- `GET /v1/benchmarks/summary`: players' benchmark uploads grouped by GPU, CPU, render distance and game for playhorizon.dev/benchmarks. A group shows only with at least 3 different installs, and only its medians and counts: no install ID, day, OS version, driver, exact memory or screen. Uploads are never published one by one. Each group's `breakdowns` count its installs per hardware class.
- `GET /v1/export/benchmarks` and `GET /v1/export/hardware`: for the maintainers' performance work only, behind a bearer token the public cannot use (404 while the server has none). The raw benchmark reports and the play sessions per machine description, including CPU threads and physical cores, with no minimum group size. Nothing identifying: no install ID, and only what a player already agreed to send (this page's lists).
- `POST /v1/live` and `GET /v1/live`: the "I'm playing" ping and the live count.
- `GET /v1/live/history?range=24h|7d|30d|all&series=players|official`: the live count, or the official server's, over time (above).

Public output never holds a raw hardware value, only these classes (`src/classes.rs`, the same in the website's `scripts/benchmarks.mjs`, horizon-rust/horizon-site), and a class is published only with at least 3 different installs:

| Breakdown | Classes |
|---|---|
| `ram` | 4 GB or less, 8 GB, 16 GB, 32 GB, 64 GB or more |
| `vram` | shared (integrated GPU), 2 GB or less, 4, 6, 8, 12, 16 GB, more than 16 GB |
| `storage` | nvme, ssd, hdd |
| `os` | linux, macos, windows |
| `gpu_kind` | integrated, discrete |

The driver, OS version, clocks, screen and power are kept raw for the 180 days of retention, to explain a slow group, and never published.

`crates/horizon-telemetry-server` is the server, at `https://telemetry.playhorizon.dev` on Railway (one SQLite file on a volume, region `europe-west4`). Its [README](../crates/horizon-telemetry-server/README.md) covers storage (about 190 bytes per play session), retention (180 days of raw rows, anonymous daily totals kept), rate limits and hosting.
