# goattech launcher: start here

the player guide. goattech launcher keeps ordinary java edition instances and the horizon rust engine in one app. each instance has its own game folder, mods, settings and saves. java mods run in the java instance that supports them; importing a modpack into horizon is a different path with a coverage report, not a promise that java code runs in rust.

<!-- [flip at release] remove this comment and the next line on release day. -->
there is no public goattech release to install yet. the website is https://launcher.goattech.us. do not substitute an upstream horizon launcher download and call it goattech.

## make an instance

open instances and create one for the minecraft version you want. choose the engine: rust for the everyday game, java with fabric when you want mods. fabric mods belong in a compatible fabric instance; a loader's installer support alone does not prove a successful game launch.

[sign in with a microsoft account](microsoft-account.md) that owns minecraft: java edition before playing. the launcher guides you through the sign-in. never paste your password or saved session files into a bug report. downloading game files and preparing horizon resources are different operations; the horizon preparation reference is not the java-instance setup guide.

use the instance's java settings if you need a particular java installation or memory limit. updating the launcher never changes an instance's pinned minecraft version.

## config sync: share keybinds and ordinary minecraft settings

sharing is off until you choose it for an instance. in the instance's edit dialog, open **synced settings**, enter a group name and choose **join**.

- a new group starts from that instance's settings.
- if an existing group and the instance differ, choose which side to use. the losing file is backed up before a change.
- **settings that sync** lets you choose individual keys. conflicts wait for a choice rather than silently replacing both sides.
- **leave** removes membership and keeps the instance's current settings.

this shares supported `options.txt` settings, such as keybinds, video and controls. it does not share a client's own configuration files. different options formats are refused, not converted. a fresh instance without its game's own options file does not receive a made-up one: run the game first, then share its settings.

changes are checked before launch without pushing local edits then. a clean exit or **sync now** can move settings both ways. do not edit or remove sync guard files while a game may still be running.

for command-line use, the binary is still named `horizon-launcher`:

```sh
horizon-launcher instance sync "My Fabric instance" --status
horizon-launcher instance sync "My Fabric instance" --group controls
# if joining reports differences, choose explicitly:
horizon-launcher instance sync "My Fabric instance" --group controls --take mine
# or take the group's values instead: --take theirs
horizon-launcher instance sync "My Fabric instance" --keys fov,sensitivity
horizon-launcher instance sync "My Fabric instance" --off
```

the generated command reference has the complete argument list. examples are instructions for a player, not executed qualification evidence.

## install a local client jar

get a legitimate fabric client mod from its publisher yourself, including any terms you need to accept. in instances, choose **choose client jar**. review the detected name and version, then choose **install in new instance**. selecting a file alone does not install it. the gui leaves existing instances unchanged.

the installer inspects metadata without executing the selected jar, checks supported minecraft/fabric/java requirements, and refuses unsupported dependency arrangements. some client mods and fabric api bundles need dependency resolution this installer does not support. a refusal is not evidence that the client is unsafe or that every version is incompatible.

the cli can create a new instance or, only when named explicitly, target an existing fabric instance:

```sh
horizon-launcher instance install-client /path/to/client.jar
horizon-launcher instance install-client /path/to/client.jar --instance "My Fabric instance"
```

this does not download a client for you. no particular dawn, lunar or other client release is qualified by this guide. the client's own config is not synced. new instances do not automatically join a sync group; that is a separate choice.

## your data choices

read the collection screen before saving your choices. the player-count ping, basic performance summaries and the four detailed report kinds are separate paths. basic summaries default on only for a genuinely new gui install; existing installs do not silently opt in. the detailed kinds are off unless chosen. you can opt out any time.

the [data-collection document](telemetry.md) has the exact payloads, storage limits and outstanding operational qualifications.

## what v1 covers

<!-- [flip at release] rewrite this section as shipped facts on release day. keep it honest: what was tested, on what machines, and what was not. -->

v1 ships only after an independent review of the actual running app scores it above 9 out of 10. until then, qualification is partial: sign-in, play, save, quit and reopen have passed in private testing, but not every loader, client mod or platform has run that test. client-specific settings sharing is not in v1. see [release requirements](releases.md).
