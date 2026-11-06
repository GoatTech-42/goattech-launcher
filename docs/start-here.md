# GoatTech Launcher: start here

Player guide for the current source. GoatTech Launcher is still in development;
this guide is not a release announcement.

GoatTech Launcher keeps ordinary Java Edition instances and Horizon's Rust engine in one app. Each instance has its own game folder, mods, settings and saves. Java mods run in the Java instance that supports them; importing a modpack into Horizon is a different path with a coverage report, not a promise that Java code runs in Rust.

There is no public GoatTech release to install yet. The website is https://launcher.goattech.us. Do not substitute an upstream Horizon launcher download and call it GoatTech.

## Make a Java instance

Open Instances and create an instance for the Minecraft version you want. Choose the Java software you need. Fabric mods belong in a compatible Fabric instance; a loader's installer support alone does not prove a successful game launch.

[Sign in with a Microsoft account](microsoft-account.md) that owns Minecraft: Java Edition before playing. The launcher guides you through the Microsoft sign-in. Do not paste your password or saved session files into a bug report. Downloading game files and preparing Horizon resources are different operations; the Horizon preparation reference is not the Java-instance setup guide.

Use the instance's Java settings if you need a particular Java installation or memory limit. Updating the launcher does not mean you should change an instance's pinned Minecraft version.

## Share keybinds and ordinary Minecraft settings

Sharing is off until you choose it for an instance. In the instance's edit dialog, open **Synced settings**, enter a group name and choose **Join**.

- A new group starts from that instance's settings.
- If an existing group and the instance differ, choose which side to use. The losing file is backed up before a change.
- **Settings that sync** lets you choose individual keys. Conflicts wait for a choice rather than silently replacing both sides.
- **Leave** removes membership and keeps the instance's current settings.

This shares supported `options.txt` settings, such as keybinds, video and controls. It does not share a client's own configuration files. Different options formats are refused, not converted. A fresh instance without its game's own options file does not receive a made-up one: run the game first, then share its settings.

Changes are checked before launch without pushing local edits then. A clean exit or **Sync now** can move settings both ways. Do not edit or remove sync guard files while a game may still be running.

For command-line use, the binary is still named `horizon-launcher`:

```sh
horizon-launcher instance sync "My Fabric instance" --status
horizon-launcher instance sync "My Fabric instance" --group controls
# If joining reports differences, choose explicitly:
horizon-launcher instance sync "My Fabric instance" --group controls --take mine
# Or take the group's values instead: --take theirs
horizon-launcher instance sync "My Fabric instance" --keys fov,sensitivity
horizon-launcher instance sync "My Fabric instance" --off
```

The generated command reference has the complete argument list. Examples are instructions for a player, not executed qualification evidence.

## Install a local client JAR

Obtain a legitimate Fabric client mod from its publisher yourself, including any terms you need to accept. In Instances, choose **Choose client JAR**. Review the detected name and version, then choose **Install in new instance**. Selecting a file alone does not install it. The GUI leaves existing instances unchanged.

The installer inspects metadata without executing the selected JAR, checks supported Minecraft/Fabric/Java requirements, and refuses unsupported dependency arrangements. Some client mods and Fabric API bundles need dependency resolution this installer does not support. A refusal is not evidence that the client is unsafe or that every version is incompatible.

The CLI can create a new instance or, only when named explicitly, target an existing Fabric instance:

```sh
horizon-launcher instance install-client /path/to/client.jar
horizon-launcher instance install-client /path/to/client.jar --instance "My Fabric instance"
```

This does not download a client for you. No particular Dawn, Lunar or other client release is qualified by this guide. The client's own config is not synced. New instances do not automatically join an options-sharing group; that is a separate choice.

## Data choices

Read the collection screen before saving your choices. The player-count ping, basic performance summaries and the four detailed report kinds are separate paths. Basic summaries default on only for a genuinely new GUI install; existing installs do not silently opt in. The detailed kinds are off unless chosen.

The current production basic-summary release lists are empty. The reviewed collector source and a consent description are not proof of a deployed service. Use the [data-collection document](telemetry.md) for the exact payloads, storage limits and outstanding operational qualifications. Do not describe this build as having no local collection or no startup network requests.

## Current qualification limits

The private Linux Vanilla 26.3 test covered sign-in, saved account reopen, play,
save, full quit and reopening the same world. This is not a claim that every
loader, client mod or platform has passed that test. Local Fabric JAR preview
and backend checks do not qualify a named third-party client. Client-specific
settings sharing remains unfinished. See [release requirements](releases.md).
