# Historical branch custody, November 6, 2026

This commit keeps retired branch tips reachable from the sole permanent `main` branch. It is an archival-history merge, not a functional merge. Its first parent contains the reviewed current source. The only tracked-file change is this manifest. None of the other parents' runtime trees, workflows, binaries or old upstream code were applied.

All original commits, full trees and evidence remain recoverable by the exact tips below with `git show <tip>:<path>` or a detached checkout. These tips are parents of this archival commit, including redundant ancestors for an explicit custody record. Branch-ref retirement is separate and uses the recorded expected SHA; this manifest does not claim deletion has occurred.

No release, tag, package, deployment, collector activation, account change or publishing permission is implied. Supersession is deliberately not asserted where it was not established.

First parent: `6bb650cdd9ed9d8d2a92745639374bc7605b4036`.

## diag-windows-runtime-20261105

Tip: `313bb245a6397b6e19813a5db3a612c1a2af6f98`

Historical Windows diagnostic. Old diagnostic workflow source preserved inactive in historical tree. Prior first-window evidence is not current runtime qualification.

## import-icon-notes-carry

Tip: `5eec9748117042f3cda27cc6c51ec1ae9c888593`

Diverged upstream/import prototype preserved. Includes divergent upstream ancestry and old icon/notes import implementation. No blanket supersession claimed. Recovery: inspect launcher/src/other_launchers.rs and instances.rs plus their tests at tip. Current runtime import code unchanged.

## launcher-qual-24a9d18b

Tip: `11e4180a6781f60bc1c708f7bc3af7097f26d9c0`

Obsolete qualification trigger. Only unique change launcher/qual-trigger.txt. Not part of runtime.

## launcher-qual-54c03befe

Tip: `870ae7dae4197d028a10934406da071af7c64dfb`

Obsolete qualification trigger. Only unique change launcher/qual-trigger.txt. Not part of runtime.

## launcher-qual-5f15edb69

Tip: `e345b551b241f938a3d652447d7220efaa95f9d2`

Obsolete qualification trigger. Only unique change launcher/qual-trigger.txt. Not part of runtime.

## launcher-qual-61471fa20

Tip: `a353ce530c22e46c8055ba24c12f1c627c1185c4`

Obsolete qualification trigger. Only unique change launcher/qual-trigger.txt. Not part of runtime.

## launcher-qual-7ee8eda57

Tip: `5fc7948db317fade2d6598c513b52987203e254f`

Obsolete qualification trigger. Only unique change launcher/qual-trigger.txt. Not part of runtime.

## launcher-qual-a0d492a51

Tip: `900f8be1d7af8cbc79802460d1910b0a53924f43`

Obsolete qualification trigger. Only unique change launcher/qual-trigger.txt. Not part of runtime.

## mcp-java-play-404d18fa

Tip: `9cebe5fdcb28e2510d7ceb2260baf02887c4ea23`

Unfinished functionality preserved, NOT integrated. Three commits add Java MCP play and exit ownership. Current main explicitly refuses Java MCP play. Recovery: inspect launcher/src/mcp.rs at this tip; must adapt to current per-key sync, locks, running holds and exit lifecycle, then test and independently review before any restoration. This custody commit does not finish that feature.

## qual-bin-5f15edb69

Tip: `02697217aa58d1e56bc3d50800abdc0baab9c3e6`

Historical binary custody. One-off prebuilt qualification binary remains in historical tree only, not copied into current runtime or released.

## qual-bin-suite-46f093c90

Tip: `e1c472f84c7c99c6d4306d50c3c3f82794890ae6`

Obsolete qualification workflow/evidence. One-off workflow or evidence remains historical only. Not reactivated, not copied into active .github/workflows.

## qual-import-icon-notes-5eec97481

Tip: `57f9a1bf821539e4b9e30c7738123765a8106f15`

Diverged upstream/import prototype preserved. Includes divergent upstream ancestry and old icon/notes import implementation. No blanket supersession claimed. Recovery: inspect launcher/src/other_launchers.rs and instances.rs plus their tests at tip. Current runtime import code unchanged.

## qual-telemetry-8e8c12a4f

Tip: `f2654ffcbbd5909f21649efcce7b06102cd3175a`

Obsolete qualification workflow/evidence. One-off workflow or evidence remains historical only. Not reactivated, not copied into active .github/workflows.

## qual-window-icon-2

Tip: `e4367614db45ff88d4a1cab03404e5ea0e8fc8f0`

Prior icon candidate and qualification preserved. Historical goat-head window icon candidate; current logo decision unresolved. No approval or source equivalence inferred; not restored.

## qual-windows-gui-5f15edb69

Tip: `a6522a9a6bfee81ced6c1d0f511846115bdaefc3`

Obsolete qualification workflow/evidence. One-off workflow or evidence remains historical only. Not reactivated, not copied into active .github/workflows.

## shared-options-404d18fa

Tip: `9ba745135725b83e9948904faad216104a6d3761`

Old implementation preserved, NOT integrated. Old last-writer-wins options_sync prototype, not current config_sync per-key baseline/conflict design. Do not restore whole-file behavior. Current per-key implementation remains unchanged.

## telemetry-worker

Tip: `5544e658c28cb11bf339d1acf497847ef0c637db`

Source folded separately. All six infra/telemetry-worker files byte-identical in parent6bb650cdd. Source custody only; no deploy, database creation, cron activation or published release.

## upstream-port-import-icon-notes

Tip: `bf301dade18d3401399b4db9587a625938c7ef7d`

Diverged upstream/import prototype preserved. Includes divergent upstream ancestry and old icon/notes import implementation. No blanket supersession claimed. Recovery: inspect launcher/src/other_launchers.rs and instances.rs plus their tests at tip. Current runtime import code unchanged.

## upstream-sync-20261031

Tip: `5fe01c42627947ad1f0fb60c5569c5780e6995a3`

Prior import/icon prototype preserved. Unique historical import-to-icon.png commit retained as history. No source restoration or equivalence claim.

## window-icon-goattech

Tip: `88262ca56827b29737010d6db69c44760692b408`

Prior icon candidate and qualification preserved. Historical goat-head window icon candidate; current logo decision unresolved. No approval or source equivalence inferred; not restored.
