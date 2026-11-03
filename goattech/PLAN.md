# GoatTech Launcher (Horizon fork) - milestone plan

(2026-11-03 status refresh: this file's branch/publication/branding/MSA lines were
corrected to the verified current state; it is a scoped status refresh, not a full
docs review.)

Fork of horizon-rust/horizon (MIT). Started on branch `goattech/main` off verified upstream
origin/main HEAD 98b6a44 (2026-10-30); work now integrates on `main` of
github.com/GoatTech-42/goattech-launcher (`goattech/main` is the stale initial branch, kept
for history). Direction (locked 2026-10-30): hybrid launcher -
keep BOTH runtimes: Rust (Horizon engine) and Java (Vanilla/Modded Fabric/Quilt instances,
upstream PR #4399, merge a827346dd5a29f65c4315d94d5ee63965404ccc2). Fabric mod support via
the Java lane; ViaFabricPlus as the running benchmark. Free-only. The code lives in a private development repository; publication is not authorized.

## Milestone 1 (local, no account needed)
- [x] Fork branch from verified upstream HEAD
- [x] GoatTech mono accent (dark-monochrome friendly): `Accent::Mono` in launcher/src/themes.rs,
      desaturates hue, keeps upstream contrast invariants; fork default; tests updated
      (themes/tests.rs hue check skipped when an accent has no hue; backend.rs default test)
- [x] `cargo test -p horizon-theme` 9/9; `cd launcher && cargo test --lib` 320/320 (3 ignored)
- [ ] Brand rename pass: user-visible "Horizon" -> "GoatTech" in launcher strings.
      Major surfaces verified in actual gui frames 2026-11-03 (window title, sidebar brand);
      the Play page's big heading is the selected instance's name ("Horizon", the
      upstream-engine instance), not a leftover brand string. Remaining user-facing
      "Horizon" strings still to audit, e.g. backend.rs download/sign-in errors
      ("Horizon tried to download", "Horizon then"), owner_page.rs sign-in text
      ("Horizon downloads it from Mojang"), proxy.rs. Crate names/API ids stay stable
      (horizon-launcher, horizon_theme) so upstream merges stay clean.
- [ ] Window/app identity: title, icon set (crates/horizon-theme/icons), about/credits keep
      upstream attribution per MIT
- [x] Scripted proof (CLI, no sign-in): gt-cli (CLI-only bin) created Vanilla + Fabric 0.20.1
      instances on live Mojang/Fabric metadata, checked + installed ViaFabricPlus 5.1.2 from
      Modrinth (SHA-512 verified); import routing: loader .mrpack -> real Java instance,
      Forge/NeoForge refused with the reason; cross-launcher copies (Prism/CurseForge app)
      verified read-only-source. See gt-cli proof transcript 2026-10-30.

## Milestone 2 (needs an owned Minecraft account + a full-build machine)
- Sign-in (MSA device code), actual Java launch, VFP benchmark: join an older-version server
- MSA client ID: own app registration reported approved 2026-11-02; device-code sign-in
  reported passing the ownership check 2026-11-03 (project status reports; session persistence
  unverified). Remaining here: actual Java launch and the VFP benchmark on an older-version
  server.

## Discipline
- Fork-only files under goattech/; upstream-tree edits minimal and additive (easier merges)
- Weekly upstream merge; `cd launcher && cargo test --lib` must stay green
- Nothing public until the repo/audience/"as me" grant lands
