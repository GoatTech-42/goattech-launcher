# Engine source architecture: vendored fork vs removal, and auto-sync

Status: **recommendation (judgment), 2026-11-03**, grounded in the tree at
`main` a6adaf738 and the pending sync branch `upstream-sync-20261102`. This is
advice for the owner, not a decision; the destructive option is explicitly not
taken here.

## The two boundaries (keep them separate)

1. **Compile-time engine crates.** The launcher path-depends on eight in-repo
   engine crates (`launcher/Cargo.toml`: horizon-theme, horizon-core,
   horizon-prepare, horizon-friends, horizon-bench, horizon-port-pack,
   horizon-mods, horizon-telemetry). None are published on crates.io.
   GoatTech's own engine-side patches are deliberately thin: as of 2026-11-03
   they touch only `crates/horizon-theme` (two source files plus icon assets);
   everything else the fork adds lives in `launcher/` and `goattech/`.
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
Judgment: not worth it now. The launcher would pin 8+ unpublished engine crates
as git deps - the same tracking burden with weaker verification (suites and
live proof no longer run on the integrated tree), no merge window for upstream
API breaks, and a forced tiny fork of horizon-theme anyway for the fork's
branding. The burden it appears to remove does not actually go away.
Revisit only if upstream publishes its crates AND the fork's engine-side
patches (today: theme only) are gone or upstreamed.

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
