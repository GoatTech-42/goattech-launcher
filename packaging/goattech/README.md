# GoatTech portable candidates

This is build-only preparation, not a release announcement. The independent
whole-app review is 8.8/10 as of November 6, 2026. The release gate remains
strictly above 9/10, and client-specific settings sharing is unfinished. Release scope is not final.

The manual `goattech-package-candidate.yml` builds Windows x64 and Linux x64
portable archives. It has read-only repository permissions, no release job,
no tag, no schedule, no self-hosted runner, no cache and no upload step. Do not
dispatch it before its source and intended output audience are reviewed. With
no upload, outputs expire with the runner. Artifact retention is a separate,
reviewed workflow change, not an excuse to publish a candidate as a release.

## Identity and compatibility

The independent GoatTech candidate version is `0.1.0-rc.1`, injected with
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

No setup executable is produced. The inherited Windows installer still uses
Horizon install folders, registry keys and publisher identity. Renaming it to
`goattech-launcher-setup.exe` would also miss its filename-based setup detection.
A portable package avoids those installer effects; it must not be presented as
an installer. Fork installer branding and install/uninstall qualification remain
unfinished if an installer is included in a later release.

## Contents and evidence

`tools/package_goattech.py` accepts one explicit regular, single-link binary.
It never searches a profile or includes a game directory. Archives contain only
the binary, portable instructions, root license, goat glyph, existing third-party
notices, selected asset/vendor license texts and manifest. The
manifest records exact source commit, platform, version and per-input SHA-256;
`SHA256SUMS` covers the archive. These checksums are not code signing.

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
