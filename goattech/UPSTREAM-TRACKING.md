# Upstream tracking (horizon-rust/horizon)

Last reviewed upstream main: 082054175 (2026-10-30, reviewed + merged 22f4ae048).
Batch reviewed: 106 commits since 98b6a442c. No upstream commit touched launcher/,
lang/, horizon-theme, the workspace manifest, goattech/ or website/; the merge was
conflict-free. Gate: 322/322 launcher lib tests green on the merged tree.

Practice: review each new upstream commit/merged PR, integrate what survives review
with the lib test suite + gt-cli live proof green, keep GoatTech behavior intact
(mono theme, import routing/refusals, brand). Compatibility- or security-critical
upstream fixes jump the queue; everything else batches. Report meaningful tested
landings; never merge unreviewed.
