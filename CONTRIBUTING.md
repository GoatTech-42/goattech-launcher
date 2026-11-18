# contributing to goattech launcher

thanks for helping. this is the launcher's own project: ordinary java instances and the horizon rust engine in one home. the engine is upstream horizon's work; this fork keeps its attribution and its mit license and stays in sync with it.

## where things live

- `launcher/` - the launcher itself, its own cargo workspace.
- `docs/` - the launcher's documentation. `docs/engine/` holds the engine's docs, inherited from upstream.
- `website/` - the source of [launcher.goattech.us](https://launcher.goattech.us).
- `crates/`, `mods/`, `tools/`, `tests/`, `roadmap/` - the engine's tree, upstream's.

## how changes land

- keep launcher work in the launcher's own files; keep engine work minimal and upstream-shaped so syncs stay easy.
- working on the engine itself? its own guide is [docs/engine/CONTRIBUTING.md](docs/engine/CONTRIBUTING.md) - the one rule there is that the engine plays exactly like vanilla.
- every launcher string is translatable: a new or changed english string lands in all ten `launcher/lang/` files in the same change, with real translations.
- run the python tests (`python3 -m unittest discover tests`) and the launcher suites (`cargo test --manifest-path launcher/Cargo.toml`) before calling a change done. python 3.10+ is only needed for these tests and for building versions from before horizon-prepare shipped.
- docs moves are structural changes: retarget every file that reads a moved path, not just the markdown links.
- docs and ui copy are lowercase, short plain sentences. black and white, no color accent. if a sentence sounds like a press release, it is wrong.

## releases

every version passes an independent review of the actual running app before it publishes. the bar is a score above 9 out of 10, and it holds for every version, not just v1. releases are signed builds on github releases, tagged `launcher@X.Y.Z`, verified before install. third-party notices regenerate as part of every release.
