# GoatTech UI implementation map (concept source -> GPUI)

Concept reference: 8 screens (design direction from the 2026-10-30 concept pass;
not final-approved UI - he may steer). Generic "Nightfall" player fixture only.

## What already exists in-tree (do not rebuild)
- Fonts bundled in crates/horizon-theme/fonts (SIL OFL): Bricolage Grotesque (display),
  Geist (UI), Geist Mono. theme FONT/MONO consts already point at them.
- sky.rs: generative night landscape (stars, bands, skyline, sun/moon, clouds, parallax
  layers) - the concept's pixel landscapes are the same family; retune, don't rewrite.
- Accent::Mono landed (goattech/main): mono accent is the fork default.
- Theme tokens (theme.rs: page/plate_soft/plate/surface/rule/ink/ink_soft/muted/accent/
  on_accent + night tokens) map 1:1 onto the concept palette.

## Concept palette -> theme tokens (mono night)
  --ground  #0a0a0a  -> page          --line     #222222 -> rule
  --surface #131313  -> surface/panel --text     #ffffff -> ink
  --raised  #1b1b1b  -> plate_soft    --muted    #b4b4b4 -> ink_soft
  --well    #050505  -> well          --faint    #8f8f8f -> muted
  primary button: white bg / black text -> accent #ffffff, on_accent #000000

## Screens -> modules (implementation order)
1. Shell: 232px sidebar (wordmark "GoatTech", nav, account card, version line) + canvas.
   -> game_app.rs root layout; nav items map to Page enum.
2. Home (home.html): big primary Play, instance switcher ("change instance"), patch notes,
   player model area (Nightfall fixture until sign-in), update pill, sign-in.
   -> home.rs (542 lines, rework around existing structures).
3. Instances (instances.html): TWO sections - Rust and Java - side by side; cards with
   icon+banner, version/loader line, per-card play. -> instances_page.rs (881).
4. New instance (new-instance.html): Java instance picks software inline (vanilla/Fabric/
   Quilt; Forge/NeoForge shown unavailable, honest). -> new_instance.rs (361).
5. Instance detail (instance.html): header art, settings rows, java/loader/memory.
   -> instance edit surfaces.
6. Mods (mods.html): Modrinth browser filtered to instance loader+version. -> mods_page.rs
   (1542) + browser*.rs.
7. Import (import.html): drag/drop + per-launcher rows (Prism/MultiMC, official, CurseForge
   app, Modrinth App). Copies never mutate source; Forge-pack jars carried into vanilla
   Java instances get an honest "won't load without Forge" label (routing landed 77b2d8fa0).
   -> import_page.rs + other_launchers.rs.
8. Icon & banner (customize.html): per-instance art, validation, crop, stored in the
   instance folder, carried by export. -> instance icon/banner handling (new).
9. Settings (settings.html): rows incl. account + updater. -> settings surfaces.

## Gates
- Every visual claim needs real rendered pixels from the built app - not yet possible in the
  lead's sandbox (GUI bin link exceeds its limits; source cargo-checks clean, gt-cli proves
  the CLI). Pixel verification lands on the first full-build machine.
- No invented parity/QA metrics, mod stats, or tested-version claims anywhere.
- Lunar/NoRisk inspire layout only: no copied brand, names, art, or code.

## Status semantics rule

Never let color (or lightness) alone carry status. NIGHT_MONO encodes ok/warn/error by lightness, so every status surface must also show an explicit word or icon: list rows keep their icon shapes; page rebuilds keep status text ("Installed", "Error") next to any indicator. Contrast is against the background, never the only difference between two statuses.
