# GoatTech Launcher releases

GoatTech Launcher is in development. There is no public release or installer yet.
Its source, website and release identity are separate from upstream Horizon.
An upstream Horizon download is not a GoatTech Launcher download.

## Before publishing

Every version must receive an independent, unbiased review of the actual working
app scoring **strictly over 9/10**. A passing build, a consent-screen review or a
successful game test alone does not clear that gate. The release must also have
evidence that it works on Windows, not only on the Linux test server.

Requested features must be finished or explicitly moved out of the version's
scope by Luke. Client-specific settings sharing is still unfinished; it is not
silently deferred or satisfied by Minecraft `options.txt` sharing. The
[start guide](start-here.md) states the current distinction.

Merging source is not publishing. Tags, release assets, installers and download
links are separate release steps after the review and scope requirements are met.
The inherited automatic Nightly train is not this fork's publication policy.

## Updates and version numbers

Upstream engine versions are not GoatTech launcher release numbers. The
[inherited versioning reference](versioning.md) describes Horizon's history, not
a GoatTech version announcement.

The current source reads upstream's engine release feed. GoatTech self-replacement
is blocked while its launcher feed points upstream, so an upstream launcher must
not silently replace GoatTech. A future GoatTech update feed needs checked
version and asset identities, signatures and platform packaging before use.
This document does not enable it.

## Data choices

Release preparation must not silently enable telemetry. Keep the consent text,
build configuration and deployed behavior consistent. The
[data collection reference](telemetry.md) describes the separate player-count
ping, basic summaries and optional detailed reports. A source review is not
proof that a collection service is deployed or that deletion has been tested.

## Developer references

[Upstream release flow](release-flow.md) and [upstream versioning](versioning.md)
remain available with their anchors intact for inherited engine tooling. Their
maintainer instructions do not authorize a GoatTech release.
