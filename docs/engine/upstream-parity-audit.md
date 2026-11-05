# Upstream parity self-audit (archived)

this page preserves upstream horizon's own published self-audit as it stood when goattech launcher forked. it is kept for attribution and comparison: it is not goattech launcher's current status, and this project has not re-audited the engine. the figures carry their own dates. `tools/parity_gaps.py readme --write` refreshes the gap block and badge below from upstream's tracker.

<p>
  <img src="https://img.shields.io/badge/upstream%20tested%20parity%20oct%2013%202026-100%25-F0795A" alt="Upstream-tested parity, oct 13 2026: 100%">
  <a href="#project-status"><img src="https://img.shields.io/badge/upstream%20gaps%20oct%2030%202026%20snapshot-833%20open-F0795A" alt="Upstream gaps, oct 30 2026 snapshot"></a>
</p>

historical upstream self-audit, shown for attribution - not a current GoatTech parity claim; this fork has not re-audited the engine. details under [project status](#project-status).

## Project status

archived upstream status: everything in this section is upstream horizon's self-audit, preserved for attribution and comparison. it is not this fork's current status, and the fork has not re-audited the engine; the figures carry their own dates.

**Player-visible gaps: 833 open** of the 833 the [port ledger](../../tools/port_ledger.py) found, reading every vanilla 26.3 method against Horizon's code: vanilla behaviour a player would notice that Horizon doesn't have yet. By area:  Screens, HUD and client packets [197](https://github.com/horizon-rust/horizon/issues/3518); Entity and block rendering, models [129](https://github.com/horizon-rust/horizon/issues/3519); Animals [110](https://github.com/horizon-rust/horizon/issues/3512); Server and players on the server [106](https://github.com/horizon-rust/horizon/issues/3520); Blocks, redstone and fluids [86](https://github.com/horizon-rust/horizon/issues/3516); Entities, projectiles, vehicles and decorations [64](https://github.com/horizon-rust/horizon/issues/3514); Monsters and bosses [53](https://github.com/horizon-rust/horizon/issues/3513); Mob AI, villagers and raids [35](https://github.com/horizon-rust/horizon/issues/3521); World, world generation and saves [32](https://github.com/horizon-rust/horizon/issues/3517); Items, enchantments and inventories [21](https://github.com/horizon-rust/horizon/issues/3515). Upstream's tracker snapshot (horizon-rust/horizon#3512-#3521), oct 30, 2026; `tools/parity_gaps.py` refreshes the counts from that tracker.

the table below is upstream's own audit of each subsystem, as published at the fork. its scores and "None" gap cells describe only the comparisons that audit ran: "None" means no difference found in the cases compared, not proof none exists, and 100% means 100% of what the harnesses compare, not proof that nothing else differs. the differences upstream still knew of close the section.

### Java parity: 100% (upstream self-audit, 2026-10-13)

Each subsystem scores 0 (absent), 0.25 (started, not playable), 0.5 (playable,
major vanilla features missing), 0.75 (playable, mostly complete) or 1 (complete
and verified against vanilla by differential tests - complete over the cases
those tests compare, not complete vanilla coverage). The percentage is the
average; `tests/test_parity.py` keeps it in sync with this table. upstream
reported this audit on 2026-10-13: all 281 harnesses passing on Linux, each
rerun whenever one of its inputs changed, and every one also running on its
Mac mini (Rust uses Java's own float maths on each platform).

the harnesses measured behaviour. upstream's second measure was method by method: the [port ledger](../../tools/port_ledger.py)
lists every method of vanilla 26.3 next to the Rust functions that port it, with a verdict for each one
(ported, folded into another function, missing, or needing no port), and the untested ports. Run
`python3 tools/port_ledger.py build`, then `coverage`, with the vanilla data in `.local/`.

| Subsystem | Score | Main gaps |
| --- | ---: | --- |
| Window, input and game loop | 1 | None: the game loop, options, camera, auto-jump, toggle sprint/sneak, local input and the frame limiter match vanilla |
| Assets and resources | 1 | None: item models match vanilla over 196,290 stacks and the client draws every item through them; the resource pack screen matches vanilla's drawing over 3,000 sessions of clicks, scrolling and searching, its incompatibility confirmation included |
| Network protocol and multiplayer | 1 | None: joins vanilla servers; the dedicated server (`--dedicated`) matches vanilla's files, status ping, login, configuration, play, chunks, chat, recipes, advancements and command tree, a client built from vanilla's packet code joins it, Horizon joins a vanilla server, and LAN discovery matches over 462 cases (offline mode only) |
| Chunk storage and palettes | 1 | None: client and saves share paletted containers, identical to vanilla over 266 scenarios |
| Section meshing | 1 | None: 112 scenes and over a million quads identical to vanilla |
| Rendering | 1 | None: entity models, block entity renderers (407 cases, End portals, gateways and the loom's banner preview included) and their animations (296 cases, End gateway beams included), the HUD overlays (2,200 states), the recipe book (600 states), the player list, subtitles and the world border match vanilla |
| Lighting | 1 | None: saved and relit light matches vanilla, slabs and stairs included |
| World generation | 1 | None: 2,352 whole chunks over 225 structure sites, in all three dimensions, identical to vanilla |
| Region files and saves | 1 | None: saves round-trip with vanilla, and worlds from 1.8 to 26.2 upgrade as vanilla's DataFixers do, superflat worlds included; old options.txt and hotbar.nbt files are upgraded too |
| Entities and physics | 1 | None: attributes, dimensions, death loot, spawn rules, natural spawning (15,500 cases), raids and patrols (1,900 cases), spawned mobs' gear, enchantments and state (`finalizeSpawn`, 24,000 cases) and mob AI (92 scenarios: villager breeding and gossip, the warden, ghastlings and copper golems included) match vanilla |
| Interaction and inventory | 1 | None: menu clicks match over 8,000 click sequences, the loom and the crafter included; recipe book placement (6,000 cases), its buttons (1,200), the saved book (1,500) and the loom and crafter screens (1,600) match vanilla |
| Menus, HUD and chat | 1 | None: every command runs as vanilla (758 commands over 39,685 parsed inputs), scoreboards and teams match (81,000 operations), the player list matches over 872,000 draw calls with heads drawn from players' skins (authlib's textures, SkinManager's lookup, legacy skin conversion), and the chat matches focused and scrolled; the resource pack screen (selection over 4,000 cases, drawing and its incompatibility confirmation over 3,000 sessions) and the chat input with its command suggestions, usage lines, Tab completion and history (3,000 sessions on vanilla's client command tree) match vanilla |
| Audio | 1 | None: the sound engine, sound events, music, situational music, ambience and subtitles match vanilla |
| Block behaviours and ticking | 1 | None: block updates, random ticks, redstone, dispensers (6,000 cases), crafters (4,000 crafts), block entity removal (containers spill, 4,000 cases), copper golem chest sorting and the sculk family (435 scenarios: shrieker warnings and warden summoning included) match vanilla |
### Known issues (upstream's list at the fork)

upstream listed three known issues when this fork's readme was written; the fork has not compiled its own list. Skins served over https show the default skin unless already cached; pack icons show the unknown-pack texture in game; item tooltips lack attribute lines.
