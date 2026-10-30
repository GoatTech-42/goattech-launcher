# Upstream tracking (horizon-rust/horizon)

Last reviewed upstream main: 98b6a442c (2026-10-30, publication baseline).
Known pending at that point: 29 commits behind (per GitHub compare at publication).

Practice: review each new upstream commit/merged PR, integrate what survives review
with the lib test suite + gt-cli live proof green, keep GoatTech behavior intact
(mono theme, import routing/refusals, brand). Compatibility- or security-critical
upstream fixes jump the queue; everything else batches. Report meaningful tested
landings; never merge unreviewed.
