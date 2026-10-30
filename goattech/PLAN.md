# GoatTech Launcher (Horizon fork) - milestone plan

Fork of horizon-rust/horizon (MIT). Branch `goattech/main` off verified upstream
origin/main HEAD 98b6a44 (2026-10-30). Direction (locked 2026-10-30): hybrid launcher -
keep BOTH runtimes: Rust (Horizon engine) and Java (Vanilla/Modded Fabric/Quilt instances,
upstream PR #4399, merge a827346dd5a29f65c4315d94d5ee63965404ccc2). Fabric mod support via
the Java lane; ViaFabricPlus as the running benchmark. Free-only. Public fork publication prepared; repo creation and push credentials pending.

## Milestone 1 (local, no account needed)
- [x] Fork branch from verified upstream HEAD
- [x] GoatTech mono accent (dark-monochrome friendly): `Accent::Mono` in launcher/src/themes.rs,
      desaturates hue, keeps upstream contrast invariants; fork default; tests updated
      (themes/tests.rs hue check skipped when an accent has no hue; backend.rs default test)
- [x] `cargo test -p horizon-theme` 9/9; `cd launcher && cargo test --lib` 320/320 (3 ignored)
- [ ] Brand rename pass: user-visible "Horizon" -> "GoatTech" in launcher strings
      (biggest surfaces: backend.rs 34, main.rs 29, cli.rs 26, native_mods.rs 25, mods_page.rs 14,
      browser.rs 14); keep crate names/API ids stable (horizon-launcher, horizon_theme) so upstream
      merges stay clean; user agent / window title / CLI name change with the strings
- [ ] Window/app identity: title, icon set (crates/horizon-theme/icons), about/credits keep
      upstream attribution per MIT
- [x] Scripted proof (CLI, no sign-in): gt-cli (CLI-only bin) created Vanilla + Fabric 0.20.1
      instances on live Mojang/Fabric metadata, checked + installed ViaFabricPlus 5.1.2 from
      Modrinth (SHA-512 verified); import routing: loader .mrpack -> real Java instance,
      Forge/NeoForge refused with the reason; cross-launcher copies (Prism/CurseForge app)
      verified read-only-source. See gt-cli proof transcript 2026-10-30.

## Milestone 2 (needs an owned Minecraft account + a full-build machine)
- Sign-in (MSA device code), actual Java launch, VFP benchmark: join an older-version server
- MSA client ID: fork needs its own Azure app registration under an account the owner controls
  (HORIZON_MSA_CLIENT_ID build config; upstream's ID is not in the repo). Gated on the account owner.

## Discipline
- Fork-only files under goattech/; upstream-tree edits minimal and additive (easier merges)
- Weekly upstream merge; `cd launcher && cargo test --lib` must stay green
- Nothing public until the repo/audience/"as me" grant lands
