# goattech launcher

goattech launcher is its own minecraft launcher project: the horizon rust engine and ordinary java edition instances side by side - fabric and quilt mods from modrinth, imports from the launchers you already have, and every instance keeping its own runtime, mods and saves. own repo, own docs, own site, own release line.

**status: in development. no releases, nothing to download yet.**

- website: https://launcher.goattech.us
- documentation: [docs/README.md](docs/README.md)
- engine upstream: https://github.com/horizon-rust/horizon

> not an official minecraft product. not approved by or associated with mojang or microsoft. you need your own legally obtained copy of minecraft: java edition.

## project status

**what works today:**

- the cli creates java (vanilla) and fabric instances end to end
- installs mods from modrinth, checksums verified
- imports instances from other launchers without mutating the source
- the launcher library and binary test suites pass (`cargo test`)

**not done yet:**

- the gui runs in headless test rigs (virtual display, software rendering) with reviewed frame captures; the home stage renders the account's real skin - verified from the public profile texture in a test rig, not from a new signed-in session in this rig; physical-display qualification still outstanding - no installers, no releases
- forge and neoforge java instances install through the official-installer driver (goattech/FORGE-NEOFORGE.md); `.mrpack` forge/neoforge imports are refused with the reason, while cross-launcher instance copies keep their loader and mods; no forge/neoforge instance has had a signed-in game launch yet
- the fork trails upstream horizon and catches up continuously

## the layout of this repo

- `launcher/` - the launcher itself: its own cargo workspace, its own docs and tests. start with [launcher/README.md](launcher/README.md).
- `docs/` - this project's documentation: using the launcher, your account, mods, privacy and data collection, releases and versioning, and how it's developed.
- `docs/engine/` - the engine's documentation, inherited from upstream horizon and kept in sync with it. it describes horizon itself; [its readme](docs/engine/README.md) says so up front.
- `crates/`, `mods/`, `tools/`, `tests/`, `bench/`, `benchmarks/` - the engine sources and harnesses, upstream's tree.
- `goattech/` - this project's own plans, ui maps and release notes.
- `website/` - the source of https://launcher.goattech.us.
- `ROADMAP.md`, `CONTRIBUTING.md` - this project's own plan and how changes land here.
- `docs/engine/ROADMAP.md`, `docs/engine/CONTRIBUTING.md`, `roadmap/`, `tools/roadmap.py` - the engine's own plans and process, upstream's, kept working from their new home.

## credit

the rust engine - the hard part - is upstream horizon's work. this fork keeps its attribution and its mit license and stays in sync with it. what the fork adds is the other half: ordinary java instances with fabric, quilt, modrinth and imports, in the same home. upstream's dated parity self-audit, kept for attribution rather than as this project's status, lives at [docs/engine/upstream-parity-audit.md](docs/engine/upstream-parity-audit.md).

## build from source

you need rust (stable), python 3.10+, jdk 25, and on linux the alsa and udev headers (`libasound2-dev libudev-dev`).

from the repository root:

```sh
# the launcher, its own cargo workspace
cargo run --manifest-path launcher/Cargo.toml
cargo test --manifest-path launcher/Cargo.toml
# on nixos: cd launcher && nix-shell, then plain cargo run / cargo test inside it

# the engine: it never downloads the game - it prepares its resources
# from your own copy of minecraft 26.3, then runs
cargo run --release -p horizon-prepare -- --prepare --server-jar ~/Downloads/server.jar
cargo run --release -p horizon-client
```

the engine's own setup notes are in [docs/engine/CONTRIBUTING.md](docs/engine/CONTRIBUTING.md#build-it). when real builds ship they'll be on github releases, verified before install.
