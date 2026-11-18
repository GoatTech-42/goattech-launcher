# goattech launcher releases

<!-- [flip at release] the first section is written for the day v1 ships. -->

goattech launcher v1.0.0 is out: signed builds on github releases and the download button on [launcher.goattech.us](https://launcher.goattech.us). its source, website and release identity are separate from upstream horizon. an upstream horizon download is not a goattech launcher download.

## how a version earns release

every version must pass an independent, unbiased review of the actual running app, scoring strictly above 9 out of 10. a passing build, a consent-screen review or a successful game test alone does not clear that gate. the release must also have evidence that it works on windows, not only on the linux test machines.

this gate holds for every version, not just v1. merging source is not publishing: tags, release assets, installers and download links are separate steps after the review passes. the inherited automatic nightly train is not this fork's publication policy.

features scoped to a version must be finished or explicitly moved out of that version's scope. client-specific settings sharing is not in v1; it is the first thing after. the [start guide](start-here.md) states the current distinction.

## updates and version numbers

goattech versions count from 1.0.0 on their own line, tagged `launcher@X.Y.Z`. upstream engine versions are not goattech release numbers. the [inherited versioning reference](versioning.md) describes horizon's history, not a goattech version announcement.

launcher updates come from goattech's own github releases feed, with checked version and asset identities, signatures and platform packaging. an upstream launcher must never replace a goattech install.

## data choices

release preparation must not silently enable telemetry. keep the consent text, build configuration and deployed behavior consistent. the [data collection reference](telemetry.md) describes the separate player-count ping, basic summaries and optional detailed reports. a source review is not proof that a collection service is deployed or that deletion has been tested.

## third-party notices

every release regenerates the third-party notices from the lockfiles and reconciles them against the checked-in notices before publishing. the vendored-works section is hand-written and stays.

## developer references

[upstream release flow](release-flow.md) and [upstream versioning](versioning.md) remain available with their anchors intact for inherited engine tooling. their maintainer instructions do not authorize a goattech release.
