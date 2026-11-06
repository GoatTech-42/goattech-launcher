GoatTech Launcher - portable build candidate

A candidate is not a public release or an independent review approval.
Source and player guide: https://github.com/GoatTech-42/goattech-launcher
Project website: https://launcher.goattech.us

Extract the archive to a folder you own. Start horizon-launcher.exe on Windows,
or ./horizon-launcher on a Linux x64 desktop. The internal binary name is kept
for compatibility. Do not rename it to horizon-launcher-setup.exe: this portable
package is not an installer and must not use the inherited Horizon setup path.
There is no Start menu registration, automatic update feed or bundled Java/game.
Linux needs a supported desktop, graphics drivers and the system libraries the
launcher links to. This archive is not an AppImage or a distribution package.

Sign in with your own Microsoft account that owns Minecraft: Java Edition.
Never share your password, account files, tokens, game folder or saved world.
Read the app's data choices before saving them. This package contains no user's
profile, game files, credentials or third-party client JARs.

Minecraft options.txt sharing is opt-in per instance. A client's own settings
are separate and are not synced. A local Fabric client JAR must satisfy the
launcher's supported dependency checks; no named client is guaranteed here.

manifest.json records the source commit, platform and SHA-256 of each packaged
input. SHA256SUMS records the archive checksum, not a code-signing certificate.
Unsigned Windows binaries can show a SmartScreen warning. Do not bypass a
warning on a download whose source and checksum you cannot verify.

Built on Horizon under the MIT license. See LICENSE, THIRD_PARTY_NOTICES.md
and notices/ for included attribution and license texts. This candidate notice
bundle is not yet a complete linked-dependency notice inventory.
GoatTech is a separate project, not an official Minecraft product.
