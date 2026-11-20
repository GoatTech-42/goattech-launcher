# GoatTech portable candidates

This is build-only preparation, not a release announcement. The independent
whole-app review is 8.8/10 as of November 6, 2026. The release gate remains
strictly above 9/10. V1 covers the core launcher and Minecraft settings sharing.
Client-specific settings and verified named-client installs are post-v1 work.

The manual `goattech-package-candidate.yml` builds Windows x64 and Linux x64
portable archives. It has read-only repository permissions, no release job,
no tag, no schedule, no self-hosted runner and no cache. Its one upload is off by
default (`keep_artifact`). Turned on, it keeps the candidate folder as a workflow
artifact for 7 days; this repository is public, so any signed-in GitHub user could
download it, which makes it a distribution of the build. Turn it on only once that
audience is admitted. Nothing is tagged, published or linked from the site, and
publishing is a separate, approved step.

## Identity and compatibility

The independent GoatTech candidate version is `1.0.0-rc.1`, injected with
`HORIZON_LAUNCHER_VERSION`. The Cargo version and inherited release line are not
changed because they describe upstream tooling. No upstream release train is
enabled, and this does not establish an update feed.

The archive and manifest say GoatTech. The executable remains
`horizon-launcher[.exe]`, as the current backend and command guide expect.
`goattech.svg` is the same goat glyph already used in the launcher, not a new
logo proposal. The executable still embeds the inherited Horizon Windows icon;
that is a known packaging identity limit, not a new approved GoatTech logo.
Changing the embedded icon needs a separate reviewed build-resource change and
inspection of the resulting executable.

On Windows the packager also writes `horizon-launcher-setup.exe`: the same
executable as in the zip, under the name `launcher/src/installer.rs` (`is_setup_exe`)
recognises (a stem starting `horizon-launcher-setup`). By that source at the packaged
commit, the setup uses APP_NAME "GoatTech Launcher", publisher "GoatTech", the keys
`...\Uninstall\GoatTechLauncher` and `Software\GoatTech\Launcher`, and reads the old
Horizon names only to take over an earlier install. That is a reading of the source:
no Windows install/uninstall run has qualified it, so installer identity is unverified in
behavior. The embedded exe icon is still the inherited one. The portable zip is the
primary package and must not be presented as an installer.

The setup exe ships alone, so it has no notices of its own. The packager writes
`horizon-launcher-setup.NOTICES.txt` beside it (listed in `SHA256SUMS`). It names the
zip as the source of the license, third-party notices, dependency notices and the
MPL-2.0 covered source. Delivery plan: the setup exe is only ever distributed with the
zip (or those same files) beside it. Distribution stays held until reviewed.

## Contents and evidence

`tools/package_goattech.py` accepts one explicit regular, single-link binary.
It never searches a profile or includes a game directory. Archives contain only
the binary, portable instructions, root license, goat glyph, existing third-party
notices, selected asset/vendor license texts and manifest. The
manifest records exact source commit, platform, version and per-input SHA-256.
On Windows the candidate folder also holds `horizon-launcher-setup.exe`, the same
binary under the name the per-user installer recognises, as a separate asset.
`SHA256SUMS` covers the archive and, on Windows, the setup exe. These checksums
are not code signing.

The package tests use synthetic binary headers to check archive behavior, modes,
manifest hashes, traversal/version rejection, symlink rejection and no-overwrite.
They do not qualify an actual PE/ELF, sign-in, gameplay, installation, or driver
compatibility. The workflow builds the real configured executable and checks
its version, embedded configuration and Windows system imports before packing.
Linux is a desktop-linked tar archive, not an AppImage or a promise of universal
distribution compatibility. A retained real candidate still needs extraction,
manifest/checksum and packaged-launch verification before release.

## Notice coverage limits

The bundle preserves the root third-party notice file and the existing Geist,
Bricolage Grotesque, Pixelify Sans, Lucide, Pixelarticons, naga and wgpu-hal
license texts. These are copied as published in this tree, not rewritten.
The root notice file also identifies the embedded Quilt/NeoForge logos and their
license URLs. This is not a complete linked-crate notice bundle or legal clearance.

A local locked/offline metadata inventory on the initial candidate found 445
Windows and 542 Linux normal/build-reachable packages. Metadata reachability is
not final linker inclusion; 68 Windows and 37 Linux package roots lacked a
top-level file matching the inventory's license filename patterns. This is not
proof that license text is absent elsewhere or that a package is unlicensed.
Before distributing a real binary, reconcile its dependency/asset license
expressions, copyright notices and required texts, including AND expressions.
Do not substitute a generic MIT license for every dependency or fabricate
missing text. Crate notice reconciliation remains unfinished.

## Existing dependency texts

The package workflow now collects existing license/notice files from locked
normal/build dependency metadata for its target. It searches only top-level
license filenames plus an explicit package license-file path, within the
checkout and Cargo registry source roots. It does not search profiles, follow
license symlinks, fetch missing texts or execute dependency code.

The archive's dependency-notices/index.json preserves each package's version,
public crates.io source identifier (other sources are redacted) and license
expression, lists copied texts and marks every entry as
review required. Local paths and unverified git/registry URLs are omitted. Files are read with a
1 MiB plus one-byte descriptor bound and metadata postcheck; this is not a
claim of complete hostile-filesystem race resistance. Missing texts stay explicitly missing.
All copied bytes are included in the package manifest hashes. This improves
notice retention; it does not establish final linker inclusion, satisfy all
license obligations or clear a distribution gate.
