# Engine source architecture: vendored fork vs removal, and auto-sync

Status: **recommendation (judgment), 2026-11-03**, grounded in the tree at
`main` a6adaf738 and the pending sync branch `upstream-sync-20261102`. This is
advice for the owner, not a decision; the destructive option is explicitly not
taken here.

## The two boundaries (keep them separate)

1. **Compile-time engine crates.** The launcher path-depends on ten in-repo
   engine crates (`launcher/Cargo.toml`: horizon-theme, horizon-core,
   horizon-prepare, horizon-friends, horizon-bench, horizon-port-pack,
   horizon-mods, horizon-release-sign, horizon-telemetry, horizon-world).
   None of the ten is published on crates.io (crates.io API check 2026-11-03:
   every name returns 404). GoatTech's own engine-side patches are
   deliberately thin but not theme-only: `crates/horizon-theme` (two source
   files plus icon assets); a telemetry default-endpoint opt-out in
   `crates/horizon-telemetry/src/lib.rs` (`endpoint()` returns `None` unless
   `HORIZON_TELEMETRY_URL` explicitly names a server, with tests); a matching
   module comment in `crates/horizon-client/src/heartbeat.rs`; and one
   reworded line in `crates/horizon-telemetry-server/README.md`. Everything
   else the fork adds lives in `launcher/`, `goattech/`, `docs/` and brand
   assets.
2. **Vendored UI dependencies.** `third_party/gpui`, `wgpu-hal` (and upstream
   also vendors `naga`) are UPSTREAM's own vendoring, wired by upstream's own
   `[patch.crates-io]` redirects. The fork inherits this; it did not choose it
   and should not un-vendor locally unless upstream does.

The Rust game instance itself already ships as a DOWNLOADED nightly artifact
(`versions/nightly-*`): playing the engine does not require the in-repo engine
source. The in-repo source is what the launcher is compiled and tested
against, and the fork's room to patch engine behavior when it must.

## Options

**A. Keep the vendored engine, sync upstream in reviewed batches (status quo).**
Merge cost is real but modest in evidence: the 2026-10-31 batch was 66 upstream
commits with two small conflicts; the 2026-11-02 batch arrived already prepared
as `upstream-sync-20261102` + `qual-bin-suite-46f093c90`, unmerged pending
review. The merged tree is where the verification lives: lib + bin suites and
the gt-cli live proof all run on the integrated source.

**B. Remove the vendored engine; launcher-only repo with git dependencies.**
Judgment: not worth it now. The launcher would pin ten unpublished engine
crates as git deps - the same tracking burden, and verification weakens if
the extracted crates' home no longer runs the suites and the live proof
against the integrated source (a separate engine repo could keep doing that;
the loss is conditional on its CI, not automatic). What is not conditional:
the merge window for upstream API breaks disappears, and the fork's own
engine-side patches (theme, telemetry opt-out) would still force a small
fork regardless. The burden removal appears to offer does not actually go away.
Revisit only if upstream publishes its crates AND the fork's engine-side
patches (today: theme, telemetry opt-out, and their docs) are gone or
upstreamed.

**C. Auto-sync.** Automation of the tracking work, compatible with A and not a
license to skip review. UPSTREAM-TRACKING.md's rule stands: never merge
unreviewed. The right shape is auto-PREPARE: fetch upstream, cut the sync
branch, run the gates (lib + bin suites, gt-cli live proof), and produce the
review summary - then a reviewed merge decision. What exists today is a daily
upstream WATCH (monitoring + report); that is not the sync automation and is
not a substitute for it.

## Recommendation

Keep A, build C as auto-PREPARE tooling, reject B for now. No destructive
removal of any source; no new repository.

## Implementation sketch for auto-PREPARE (not built yet)

- Trigger: the daily upstream watch reports movement (or a cron in CI).
- Steps: branch `upstream-sync-<date>` off `main`, merge upstream main, run
  `cargo test` (lib + bin) and the gt-cli live proof, write the batch summary
  into `goattech/UPSTREAM-TRACKING.md` as PENDING (as the 2026-11-03 entry
  records the 2026-11-02 batch today), and report for review.
- Gate: a reviewed, tested merge by a human or the review pipeline; the tool
  never pushes to `main`.
