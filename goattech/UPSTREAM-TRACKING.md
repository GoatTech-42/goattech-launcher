# Upstream tracking (horizon-rust/horizon)

Last reviewed upstream main: 94b3b4aca (2026-10-30, reviewed + merged here).
Batch reviewed: 570 commits since 082054175; 20 touched launcher/. Landings that
matter to GoatTech: the Mods page hides sources and tabs that are not usable yet
(0416fe600), then keeps the Resource packs tab on Modrinth (4cf3a14ba) and uses
imported packs in profiles (574c60055); a bad pack never blocks import or launch,
with a hard-link fallback for downloads (b49925118). Mods browser, Modpacks tab,
AI agent and Data collection settings, and the Servers dashboard are localized
(0b912926a, 5b492b240, 44d3e49db, 0c5074bba); the language row is a dropdown
(56402b95f). New servers check Minecraft accounts by default; --check-accounts
stays as a hidden no-op for old scripts (b636b37e9, 6f654f918). Native builds:
offline split-pin mods, no console window on Windows, an up-front statement of
what a Linux native build lacks (9bb2c8818, 4ecf52119, adf54b0c6). Engine fixes
with no launcher surface: wire values, protocol-incompatible marking, chat
"Not Secure" bar, open_url (d331af996, 9e6453c45, db2c7b799, 43f142115).
Cargo.toml: comment-only.

Merge notes: conflicts in README.md (kept the GoatTech fork README),
launcher/src/main.rs (kept both the "instances" and "resourcepacks" route arms),
and all 7 lang files, resolved key-by-key against the merge base: upstream-owned
strings follow upstream, GoatTech strings (brand, Java runtimes row, About
attribution, mods filter line, import section) kept, keys the merged code no
longer asks for pruned. Gates on the merged tree: cargo check green; 329/329
launcher lib tests; 44/44 launcher bin tests; goattech/proof-m1.sh green
(version catalog, vanilla + Fabric instance create, real Modrinth ViaFabricPlus
download with SHA-512 match, instance isolation). GUI still unverified (no
working display route), so nothing here is a pixel claim.

Practice: review each new upstream commit/merged PR, integrate what survives review
with the lib test suite + gt-cli live proof green, keep GoatTech behavior intact
(mono theme, import routing/refusals, brand). Compatibility- or security-critical
upstream fixes jump the queue; everything else batches. Report meaningful tested
landings; never merge unreviewed.
