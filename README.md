<!-- draft for the v1 repo overhaul. not published.
     lines marked [flip at release] describe the released state. they are written
     the way they should read the day v1 ships. if a line is not true yet, keep
     the old wording until it is.
     style rule: lowercase, short plain sentences. no ai vocabulary. -->

# goattech launcher

one launcher for every way you play minecraft. the fast horizon rust engine and ordinary java edition instances side by side. fabric and quilt mods from modrinth. imports from the launchers you already have. your settings follow you across all of it.

**[flip at release] v1.0.0 is out. download: https://launcher.goattech.us**

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
- light, useful stats on by default. opt out any time. an anonymous player-count ping runs while you play either way.

## how it ships

every version passes an independent review of the actual running app before it publishes. the bar is a score above 9 out of 10. no exceptions. that is why there is a download button instead of a promise.

## the layout of this repo

- `launcher/` - the launcher itself: its own cargo workspace, its own docs and tests. start with [launcher/README.md](launcher/README.md).
- `docs/` - using the launcher, your account, mods, privacy and data collection, releases and versioning, and how it is developed.
- `docs/engine/` - the engine's documentation, inherited from upstream horizon. it describes horizon itself; [its readme](docs/engine/README.md) says so up front.
- `crates/`, `mods/`, `tools/`, `tests/`, `bench/` - the engine sources and harnesses, upstream's tree.
- `website/` - the source of https://launcher.goattech.us.
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

the engine's own setup notes are in [docs/engine/CONTRIBUTING.md](docs/engine/CONTRIBUTING.md#build-it). releases ship on github releases, verified before install.
