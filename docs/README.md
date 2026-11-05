# goattech launcher documentation

everything about the launcher: using it, your account, your data, and how it is built and released. the engine it installs has its own documentation under [engine/](engine/README.md) - inherited from upstream horizon and kept in sync with it.

## using the launcher

- [the launcher itself](../launcher/README.md): what it does, each page, and the cli.
- [microsoft account sign-in and online-mode play](microsoft-account.md).
- [the mod browser](mod-browser.md): modrinth and curseforge sources, checksums, updates.
- [modpacks and imports](engine/modpacks.md): coverage reports and what ports today.
- [resource preparation](launcher-preparation.md): what `prepare` does with your own minecraft copy.
- [every launcher command](launcher-commands.md): the cli and mcp tools, generated from the clap definition.

## your data

- [data collection](telemetry.md): the consent dialog, exactly what is and is not sent, and where it goes.
- [benchmark report format](benchmark-report.md): the versioned json every benchmark writes, the local log and, with consent, the upload.

## developing and releasing

- [release flow](release-flow.md): channels, sign-off and publishing.
- [versioning](versioning.md): how releases are numbered.
- [ci runners](ci-runners.md): which jobs run on github's runners and which on ours, and why.
- [design system](design-system.md): the shell the launcher's ui and site follow.
- [agent control](agent-control.md): what ai agents can drive in the launcher and the game, and the control-socket rules.

## the engine

- [engine documentation](engine/README.md): architecture, world generation, servers, modding, parity and benchmarks - upstream horizon's own docs, describing horizon itself.
- [upstream parity self-audit (archived)](engine/upstream-parity-audit.md): upstream's dated self-audit, kept for attribution; not this project's current status.
