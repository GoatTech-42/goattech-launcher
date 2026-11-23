# Pinned route survey

Manual capture-only topic. Never merge this harness or its source seam into shipping launcher sources.

Targets are immutable full source commits:

- `p20`: `43c56fa7583e89c453969a1f6e32ceef01fb5418`, baseline and 2560x1440.
- `combined`: `c5ef0ce75222bf4d223f21ece8b90bc609abc153`, s=1 baseline only.
- `p21`: `b47844135a09a1d0990847709590e1d34029ffaa`, baseline and 2560x1440.

The exact and instrumented matrix jobs build separately. Both use fresh isolated homes per route, disabled self-update and telemetry, offline proxy settings, synthetic accounts and local fixture bytes. No real game, native build, download, token, file picker, sign-in, export, invitation, install or purchase is invoked by the default sequence. The configured Microsoft application ID build check is retained; runtime account checks never sign in.

Exact routes: home, instances, mods, modpacks, resourcepacks, shaders, datapacks, servers, versions, notes, settings, new-instance, edit-instance, sign-in, getting-ready, export, launch, skin, worlds, instance-mods, multiplayer, packs, screenshots.

For exact export/launch/skin, the initial frame is calibration, not proof the panel opened. Inspect baseline pixels, then run that single route with bounded actions to scroll and open the intended panel. No guessed clicks are supplied. Every other exact ordinary page also captures one lower scroll position. This is a finite route inventory, not proof that every conditional/error state has been visited.

Instrumented routes: export, launch, skin, console, native-log, instance-mods, packs, world-datapacks, internet, modpack-detail. The pinned SHA256 of `route-capture-only.patch` is checked before applying it inside the runner. It changes no layout methods, only selects render paths and supplies synthetic in-memory rows/logs. Its runtime module is absent from all shipping source pins. Direct panel rendering changes surrounding context, so it proves panel content only, never the navigation/anchor path. Native-log keeps its real full-window overlay; the route's metadata identifies the instrumented source. Internet shows only the safe missing-Tailscale state. Invites/token/policy variants are not covered. Modpack fixture has an empty report, no executable payload or remotely fetched files.

Each run exports per-route frame markers prefixed with exact/instrumented and route. Window-info records workflow SHA, source SHA, binary hash, route, measured client size and DPI. Sources stay pinned, and the harness checkout uses the workflow SHA. No running capture ref is repointed. Frame hashes are in raw logs. Required captures and step count must match; failures are collected and the job fails after evidence export. Rendering/open-state/persistence needs pixel review, labels are not assertions.

The inherited per-route screenshot limits stay 3840px/16MP, raw PNG 4MB and encoded log ceiling 24M chars. One route failure does not stop later route evidence. The 18-minute route step is a hard bound for the finite inventory; a forced runner stop may bypass cleanup. No artifact or cache storage is used.
