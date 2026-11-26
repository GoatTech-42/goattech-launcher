<!-- draft for the v1 repo overhaul. not published.
     lines marked [flip at release] describe the released state. they are written
     the way they should read the day v1 ships. if a line is not true yet, keep
     the old wording until it is.
     style rule: lowercase, short plain sentences. no ai vocabulary. -->

# goattech launcher

one launcher for every way you play minecraft. the horizon rust engine and ordinary java edition instances side by side. fabric and quilt mods from modrinth. imports from the launchers you already have. your settings follow you across all of it.

v1 is being qualified. a candidate build or a passing source check is not release approval.

- website: https://launcher.goattech.us
- documentation: [docs/README.md](docs/README.md)
- engine upstream: https://github.com/horizon-rust/horizon

> not an official minecraft product. not approved by or associated with mojang or microsoft. you need your own legally obtained copy of minecraft: java edition.

## what it does

- rust instances and java instances, side by side. same library, same accounts, same settings.
- fabric and quilt mods with real modrinth browsing built in.
- config sync: your keybinds, video settings and options shared across profiles. turn it off per profile, or per single setting. nothing changes unless you choose it.
- imports from prism and other launchers. worlds, mods, configs, packs, settings. copied, never moved.
- microsoft sign-in, saved securely on your machine.
- instance icons and banners. make each one yours.
- pinned versions. launcher updates never move your game.
- no ads, ever.
- a real 3d model of your skin on the home screen. drag it to turn it; it sways, looks around and waves on its own.
- basic performance summaries have a separate switch. new gui installs default it on; existing installs keep their choice. release allowlists are empty, so no public-release summary is sent yet. detailed reports stay off unless chosen. the player-count ping is on while the game runs, separate from the other switches. see [data collection](docs/telemetry.md).

## how it ships

before v1 ships, the complete running app must pass independent review above 9 out of 10. scoped source checks, synthetic imports and a candidate package do not clear that gate. windows java/fabric gameplay and packaged-app checks still need current-candidate evidence.

## the layout of this repo

- `launcher/` - the launcher itself: its own cargo workspace, its own docs and tests. start with [launcher/README.md](launcher/README.md).
- `docs/` - using the launcher, your account, mods, privacy and data collection, releases and versioning, and how it is developed.
- `docs/engine/` - the engine's documentation, inherited from upstream horizon. it describes horizon itself; [its readme](docs/engine/README.md) says so up front.
- `crates/`, `bench/`, `goattech/`, `packaging/`, `infra/` - the engine sources, harnesses and this fork's own tooling.
- `site/` - the source of https://launcher.goattech.us. `website/` holds its content rules.
- `ROADMAP.md`, `CONTRIBUTING.md` - the plan and how changes land here.

## credit

the rust engine - the hard part - is upstream horizon's work. this fork keeps its attribution and its mit license and stays in sync with it. what the fork adds is the other half: ordinary java instances with fabric, quilt, modrinth, imports and config sync, in the same home.

## build from source

you need rust (stable), jdk 25, and on linux the alsa and udev headers (`libasound2-dev libudev-dev`). python 3.10+ is only needed when building versions from before horizon-prepare shipped.

from the repository root:

```sh
# the launcher, its own cargo workspace
cargo run --manifest-path launcher/Cargo.toml
cargo test --manifest-path launcher/Cargo.toml
# on nixos: cd launcher && nix-shell, then plain cargo run / cargo test inside it

# the engine: it never downloads the game - it prepares its resources
# from your own copy of minecraft, then runs
cargo run --release -p horizon-prepare -- --prepare --server-jar ~/Downloads/server.jar
cargo run --release -p horizon-client
```

the engine's own setup notes are in [docs/engine/CONTRIBUTING.md](docs/engine/CONTRIBUTING.md#build-it). releases ship on github releases. game downloads are checked before install. launcher builds come with sha256 sums.
