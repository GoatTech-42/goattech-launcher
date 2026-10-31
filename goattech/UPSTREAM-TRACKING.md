# Upstream tracking (horizon-rust/horizon)

Last reviewed upstream main: 36f4bc00d (2026-10-31, reviewed + merged here).
Batch reviewed: 66 commits since 94b3b4aca; 6 launcher files. Landings that
matter to GoatTech: native rebuilds now stay on the Horizon the instance plays
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
