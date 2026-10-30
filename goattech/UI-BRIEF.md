# GoatTech Launcher - UI overhaul brief (third-role packet)

Branch boundary: work on `goattech/ui` off `goattech/main`. You own launcher visual layer:
`launcher/src/theme.rs`, `themes.rs`, `sky.rs`, `art.rs`, page modules (`home.rs`, `instances_page.rs`,
`new_instance.rs`, `mods_page.rs`, `browser*.rs`), icons/fonts under `crates/horizon-theme`.
Lead (this agent) owns: `backend.rs`, `cli.rs`, `instances.rs`, `loaders.rs`, `java*.rs`,
`account.rs`, `mojang.rs`, `other_launchers.rs`, `import*.rs`, `goattech/PLAN.md`.
Qualification lane owns: import format fixes, version/loader matrix fixtures, on a
complementary branch - coordinate merges through the maintainer.
Gate for every visual claim: real rendered pixels from the built app (screenshot), not lib tests.

## Locked direction decisions (2026-10-30, in order)
1. Hybrid launcher: BOTH engines - Rust (Horizon) and Java - in one app. Rust stays intact.
2. Two sections/tabs: **Rust** and **Java**. No separate Fabric/Forge sections.
3. Software is chosen INSIDE a Java instance: vanilla, Fabric, Quilt today; Forge/NeoForge NOT
   currently supported upstream (loaders.rs is fabric/quilt only) - show as unavailable/flagged
   until loader support lands; never a fake option, never silently dropped on import.
4. Java should support the full version catalog; untested/unsupported versions are labeled, not hidden.
5. Modpack + instance imports from other launchers (Prism/MultiMC, official, CurseForge app,
   Modrinth App, .mrpack/CF zip). Never overwrites existing instances; preserves
   worlds/saves/configs/custom mods.
6. Instance isolation: each instance keeps its own version, runtime, mods, configs, saves.
   Rust, vanilla Java and Fabric setups side by side without mixing.
7. Custom instance icons AND banners per instance: image validation, local file handling,
   crop, default fallback, persists, exports with the instance, imports carry source art
   (copy, never mutate the source launcher).
8. Home screen: 3D player model preview (like NoRisk/Lunar). Only the signed-in account's own
   skin via legit APIs; a clearly labeled local fixture skin for previews before sign-in.
9. Full UI overhaul inspired by Lunar Client / NoRisk - the FEEL (clean dark home, big play
   action, player model, card grid), NOT their brand, names, art or any copied assets/code.
   Base style: GoatTech dark monochrome (Accent::Mono already landed on goattech/main).
10. Bar: "best launcher of all time" - home worth opening, painless instances/imports, both
    engines actually working. Ambition is the bar, not a promise.

## Reference grounding (public sources, for the designer)
- Lunar Client launcher: dark home, left nav, large centered play area, account card with skin
  render top corner, news/changelog cards.
- NoRisk Client launcher: dark minimal, prominent 3D player model on home, instance grid with
  custom icons/banners, version+loader picker inline on the instance card.
Fetch current official screenshots (lunarclient.com, norisk.gg or their GitHub) before laying
out; do not trace or copy their art. Our mono palette + these layout ideas = the GoatTech look.

## Existing upstream surface to reuse (do not reimplement)
- Accent/theme system: crates/horizon-theme + launcher/src/themes.rs (Mono accent in place).
- Instance kinds: new_instance.rs (Horizon/Vanilla/Modded tabs - re-skin into Rust/Java sections;
  Java kind carries loader choice already).
- Icons per instance: instance_icons.rs exists (kind-based glyphs) - extend to custom images.
- Night look: themes render dark via night::NIGHT; DAWN stays for light players.

## First deliverables for this role
1. Rust/Java two-section instances page (reorganized from existing kinds; software picker on
   Java cards: vanilla/Fabric/Quilt now, Forge flagged unavailable).
2. Home with player-model panel (fixture skin labeled "preview" until account sign-in lands).
3. Custom instance icon + banner upload/crop/persist with safe image handling + fallback.
4. Screenshots of each from the real built app, dark mono theme, attached for review.
