# Forge / NeoForge support plan

Status: **plan, not started**. Today `loaders::Kind` is `Fabric | Quilt` only
(`launcher/src/loaders.rs`). Imported Forge/NeoForge instances are listed but
blocked with an explicit reason (never silently imported without mods), and
new Java instances can pick Fabric or Quilt only. Luke's "import everything"
scope does not accept dropping Forge/NeoForge mods, so this is owed work.

## Grounded installer behavior (checked 2026-10-31)

Sources:
- https://notes.highlysuspect.agency/neoforge-installer.html (installer internals)
- https://docs.neoforged.net/user/docs/client (official user flow)
- https://docs.rs/minecraft-java-rs-core/latest/src/minecraft_java_rs_core/loader/neoforge.rs.html
  (a working Rust implementation driving the official installer jar)

Both Forge and NeoForge ship an **installer jar** whose main class is
`net.minecraftforge.installer.SimpleInstaller`. Run headless as
`java -jar installer.jar --install-client <target dir>`.

The jar's `install_profile.json` drives everything (`spec: 0` = `Install`,
otherwise `InstallV1`):

- `json`: path inside the jar to the loader's version JSON
  (`inheritsFrom` the vanilla version; main class, game/JVM arguments,
  libraries in piston-meta shape).
- `libraries`: libraries the installer downloads (Maven coordinates +
  repository list).
- `processors`: JVM programs to run at install time (optional `sides`:
  client/server), each with its own classpath and args. They consume data
  files (`BINPATCH`, mappings, the vanilla jar) and produce the patched
  game jar. This is the real work of a Forge-family install; it cannot be
  skipped or reduced to pure downloads.
- `data`: named files inside the jar (`MAPPINGS`, `MOJMAPS`,
  `MERGED_MAPPINGS`, `BINPATCH`, `MC_SLIM`, `MC_EXTRA`, ...).

The official client install: extract version JSON to
`versions/<id>/<id>.json`, locate or download the vanilla jar, download
libraries, run the processors, then edit `launcher_profiles.json` (we do
not need that last step; we keep our own instance model).

Version listing: Maven metadata. NeoForge legacy coordinates
`net.neoforged:forge` (`<mc>-<build>` versions) vs current
`net.neoforged:neoforge` on https://maven.neoforged.net; Forge on
https://maven.minecraftforge.net (`1.x.y-<build>`).

## Chosen route: drive the official installer (option A)

Two routes exist:

- **A. Run the official installer jar headless** with a prepared target
  dir: place the vanilla client jar + version JSON where the installer
  expects, add an empty `launcher_profiles.json`, run
  `--install-client`, then read back `versions/<id>/<id>.json` and the
  downloaded libraries. Used by the reference implementation above;
  tracks official behavior by construction.
- B. Re-implement the processor pipeline in Rust. No Java needed at
  install time, but we would re-execute JVM processors anyway (they are
  JVM programs), so B buys nothing and forks installer internals.

**Choose A.** Processors are JVM code either way, and our Java instances
already manage runtimes (`crate::java`), so Java at install time is not a
new dependency. One install runs once per (loader, version); the result
is cached like the Fabric/Quilt profiles are.

## Integration points

- `loaders::Kind`: add `Forge` and `NeoForge`; `id()` returns `forge` /
  `neoforge` (Modrinth's names, so the mod browser keeps filtering
  correctly); `meta()` replaced by a per-kind version listing (Maven
  metadata for these two).
- `loaders::install` (`src/loaders.rs:175`): for Forge/NeoForge, download
  the installer jar with `fetch_checked`, prepare the scratch dir under
  `minecraft/`, run the installer with a picked Java home, then hand the
  produced version id to the existing `java_launch::resolve` merge. The
  `.downloaded` marker pattern (`backend::java_game`) caches the result.
- `new_instance`: offer all four loaders for Java instances.
- `other_launchers.rs`: replace the Forge/NeoForge block with real import
  once launch works; keep the explicit unsupported reason until then.
  `--no-mods` stays an explicit user choice, never the default.
- Offline: an installed Forge/NeoForge instance must start offline like
  Fabric/Quilt (everything already on disk after the one install).

## Risks / open questions

- Processor runtime and failure surface: first install downloads + runs
  several JVM tools; needs progress reporting on the install bar and a
  clear error, not a hang.
- NeoForge legacy vs current Maven split (version string shape decides).
- Forge's installer for old Minecraft versions (pre-1.13) predates
  `install_profile.json`; decide a cutoff (suggest: support 1.13+ only,
  say so in the UI) before implementing.
- Processor outputs land in the shared `libraries/`; verify isolation
  per instance vs shared cache (Fabric/Quilt share today; same model).
