# Forge / NeoForge support plan

Status: **implemented and verified**. `loaders::Kind` covers all four
loaders; Forge and NeoForge install through the official-installer driver in
`launcher/src/forge_install.rs` (route A below, chosen and built), for
Minecraft 1.13 and newer. New Java instances offer all four loaders, and
Forge/NeoForge imports carry their loader and mods. What a working install
does not prove is a signed-in game launch: that check waits on Mojang's
approval of the launcher's app registration, as does every launch.

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

## Verification (2026-11-01, goattech/main 45d60125f)

The ignored end-to-end tests in `launcher/src/loaders/tests.rs` were run for
real on Linux against the official installers and Mojang's services:

- `neoforge_installs_for_real`: installed NeoForge 21.1.265 for Minecraft
  1.21.1 (79 libraries); produced profile, on-disk libraries, `classpath()`
  and `java_launch::resolve` against the staged vanilla version all checked.
- `forge_installs_for_real`: installed Forge 1.21.1-52.1.20 for Minecraft
  1.21.1 (61 libraries); same checks.
- `legacy_neoforge_installs_for_real`: installed NeoForge 1.20.1-47.1.106 -
  the legacy `net.neoforged:forge` artifact - for Minecraft 1.20.1 (83
  libraries, Java 17); same checks.

All passed. This is the "installer driver is verified" precondition the
`installable()` doc comment names; the flip itself (plus the new-instance
cards and the import block lifting) stays gated until release sign-off.

## Integration points (current state)

- `loaders::Kind`: done - `Forge` / `NeoForge` with Modrinth ids and Maven
  coordinates; `meta()` is `None` for both, version listing comes from Maven
  metadata.
- `loaders::install`: done - Forge/NeoForge route to `forge_install::install`,
  which downloads the installer jar, prepares the scratch dir, picks or
  downloads the Java the version needs, runs `--install-client`, and returns
  the produced version id for the existing `java_launch::resolve` merge.
  `backend::java_game`'s `.downloaded` marker caches the result; the next
  start is offline, as for Fabric/Quilt.
- `new_instance`: done - all four loaders are real cards (Forge/NeoForge
  render without a service logo, as only Fabric and Quilt have one).
- `other_launchers.rs`: done - Forge/NeoForge imports carry their loader and
  mods; the recorded bare Forge build is named with its Minecraft prefix for
  the Maven installer download. A Forge/NeoForge whose Minecraft predates the
  1.13 layout keeps the explicit block, and `--no-mods` stays an explicit
  user choice.
- Offline start after install: covered by the same profile/library cache as
  Fabric/Quilt.

## Risks / open questions

- Processor runtime and failure surface: first install downloads + runs
  several JVM tools; needs progress reporting on the install bar and a
  clear error, not a hang.
- NeoForge legacy vs current Maven split; resolved: 1.20.1 builds come from
  the legacy `net.neoforged:forge` artifact, Minecraft-prefixed like Forge's
  (`neoforge_legacy`, `maven_for`, `maven_version`); 1.20.2+ uses the
  `neoforge` artifact unprefixed.
- Forge's installer for old Minecraft versions (pre-1.13) predates
  `install_profile.json`; resolved: the cutoff is 1.13, named up front by
  `forge_install::install` before any download.
- Processor outputs land in the shared `libraries/`; resolved: the shared
  model stands, as for Fabric/Quilt, and the for-real installs verified the
  jars land where `classpath()` looks.
