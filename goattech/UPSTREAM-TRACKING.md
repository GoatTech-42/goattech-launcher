# Upstream tracking (horizon-rust/horizon)

Last MERGED upstream batch: 36f4bc00d (2026-10-31, reviewed + merged here).

Pending batch (2026-11-03, corrected counts): the 2026-11-02 sync sits on
branch `upstream-sync-20261102` (merge 46f093c90, upstream ba5ba8b00), with
the qualification branch `qual-bin-suite-46f093c90`. It is NOT merged into
`main` as of 2026-11-03. True size of the batch: 5,816 commits (1,445
first-parent PR merges) since the 2026-10-31 base 36f4bc00d; net delta
against main's merge-base 954bd4698 is 1,694 files (+430,602/-28,872),
including upstream's newly vendored third_party/naga. An earlier revision of
this section said "66 commits since 94b3b4aca; 6 launcher files" - that line
does not describe this batch by any reproducible count and is corrected here.
Review state: independent finite SOURCE PASS on 46f093c90 (2026-11-02,
merge-fidelity scope: 4-file/2,788-blob manifest, delimiter and i18n
placeholder checks, conflict resolutions verified - not a per-commit review
of upstream's own history); hosted suite recorded 549 pass / 0 fail (run
41482712172). Author-side gates on the exact tree (2026-11-03, sandbox):
lib 493 pass / 0 fail (8 ignored), bins 52 pass / 0 fail, fresh cli build,
goattech/proof-m1.sh live proof OK - 545 observed locally vs 549 recorded
hosted, the difference not reconciled by assumption. By merge titles the
batch is upstream chore/ports PRs (vol-a client classes, vol-b game classes)
plus native-hooks stamping. Still open before any merge: the
pixels/interaction running-app gate on the batch tree, and finite review of
the fresh conflict resolutions against current main on branch
`integrate-nov2-batch-20261103` (11 files: telemetry docs, 7 lang files,
heads.rs, instances_page.rs, main.rs).
Batch landings that matter to GoatTech: Native rebuilds now stay on the Horizon the instance plays
and inside its instance (647ec89bf: new base_for/--update, toolchain commands
scrub RUSTFLAGS and CARGO_* so a player's ~/.cargo/config.toml can't change the
flags, prune keeps the three newest builds), native builds use the fast profile
at low OS priority and keep copied-program permissions (abeb5845d), macOS on
Intel gets x86_64 builds (1102fa42d, backend.rs +146). Engine: anticheat
prediction feature set (vanilla client on a jittery connection never flagged),
perf parallelization (preload world at launch, server-thread overlap, parallel
world/core-resource loads, adaptive view distance), gameplay parity fixes
(candle snuff, villager age boundary, merchant space reselect, wind-burst armor
stand, knowledge-book unlock), saved chunks past view distance. No manifest or
lang changes; no security- or compatibility-critical fix in the batch.

Merge notes: conflicts in README.md (kept the GoatTech fork README) and one
cli.rs hunk (native-build execute block: our side was the old logic with our
crate:: import convention, upstream rewrote the logic; took upstream's version
verbatim - bare native_mods:: paths resolve through our auto-merged
`use crate::native_mods;`). backend.rs and main.rs auto-merged. Gates on the
merged tree: cargo check green; 336/336 launcher lib tests (+7 upstream);
44/44 launcher bin tests; goattech/proof-m1.sh green (version catalog, vanilla
+ Fabric instance create, real Modrinth ViaFabricPlus download with SHA-512
match, instance isolation). GUI still unverified (no display route), so nothing
here is a pixel claim.

Previous batch: 94b3b4aca (2026-10-30, 570 commits, merge c272edd0e).

Practice: review each new upstream commit/merged PR, integrate what survives review
with the lib test suite + gt-cli live proof green, keep GoatTech behavior intact
(mono theme, import routing/refusals, brand). Compatibility- or security-critical
upstream fixes jump the queue; everything else batches. Report meaningful tested
landings; never merge unreviewed.
