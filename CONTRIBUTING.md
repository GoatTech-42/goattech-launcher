# contributing to goattech launcher

thanks for helping. this is the launcher's own project: ordinary java instances and the horizon rust engine in one home. the engine is upstream horizon's work; this fork keeps its attribution and its mit license and stays in sync with it.

## where things live

- `launcher/` - the launcher itself, its own cargo workspace.
- `docs/` - the launcher's documentation. `docs/engine/` holds the engine's docs, inherited from upstream.
- `website/` - the source of [launcher.goattech.us](https://launcher.goattech.us).
- `goattech/` - this project's own plans, ui maps and release notes.
- `crates/`, `mods/`, `tools/`, `tests/`, `roadmap/` - the engine's tree, upstream's.

## how changes land

- keep launcher work in the launcher's own files; keep engine work minimal and upstream-shaped so syncs stay easy.
- working on the engine itself? its own guide is [docs/engine/CONTRIBUTING.md](docs/engine/CONTRIBUTING.md) - the one rule there is that the engine plays exactly like vanilla.
- every launcher string is translatable: a new or changed english string lands in all ten `launcher/lang/` files in the same change, with real translations.
- run the python tests (`python3 -m unittest discover tests`) and the launcher suites (`cargo test --manifest-path launcher/Cargo.toml`) before calling a change done.
- docs moves are structural changes: retarget every file that reads a moved path, not just the markdown links.

## releases

nothing publishes - no installers, no tags, no site launch - until an independent review of the actual running app scores it above 9/10. when releases do start, they'll be signed builds on github releases, tagged `launcher@X.Y.Z`, verified before install.
