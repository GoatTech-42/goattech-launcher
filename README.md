# goattech launcher

a minecraft launcher, forked from [horizon](https://github.com/horizon-rust/horizon) (mit): the horizon rust engine and ordinary java edition instances side by side - fabric and quilt mods from modrinth, imports from the launchers you already have, and every instance keeping its own runtime, mods and saves.

**status: in development. no releases, nothing to download yet.**

- website: https://launcher.goattech.us
- upstream engine: https://github.com/horizon-rust/horizon

> not an official minecraft product. not approved by or associated with mojang or microsoft. you need your own legally obtained copy of minecraft: java edition.

## what works today

- the cli creates java (vanilla) and fabric instances end to end
- installs mods from modrinth, checksums verified
- imports instances from other launchers without mutating the source
- 322 library tests pass

## what's not done yet

- the gui passes `cargo check` but the full app hasn't had a real run yet - no installers, no releases
- forge and neoforge aren't supported: there's no installer pipeline for them, and the app says so instead of faking it
- the fork trails upstream horizon and catches up continuously

## credit

the rust engine - the hard part - is upstream horizon's work. this fork keeps its attribution and its mit license and stays in sync with it. what the fork adds is the other half: ordinary java instances with fabric, quilt, modrinth and imports, in the same home.

files that aren't specific to the fork (`docs/`, `ROADMAP.md`, `CONTRIBUTING.md`, the engine sources) are upstream's and describe horizon itself. fork-specific plans and ui maps live in `goattech/`.

## building

it's a rust workspace: `cargo test` runs the library suite, `cargo check` covers the gui crate. when real builds ship they'll be on github releases, verified before install.
