# Windows GUI import qualification

Source head: 1cf14202456e931d4ba1d9479da24d4bf9174dfb.
This packet is synthetic test data and instructions, not completed qualification.

## Scope and safety

Use only the isolated hosted Windows runner and a new launcher/game profile. Keep telemetry off and self-update disabled. No sign-in, gameplay, external account, private files, paid service or release. These world/mod/archive files are marker bytes, not valid playable content. Do not press Play. Network access to the public Minecraft version manifest is expected by the import resolver. An unavailable manifest is a reported blocker, not a reason to bypass the resolver.

Do not replace a real Prism installation. On the fresh runner only, place the supplied PrismLauncher tree under the runner account's actual roaming AppData directory. Launcher discovery uses the OS data directory, not HORIZON_HOME. Record the resolved path. Abort if that directory already contains unrelated data. The packet includes its own PNG icon; no assets or secrets need downloading.

## Evidence before launch

1. Record exact checked-out source SHA, workflow SHA, binary SHA256, OS image, window size, launcher/game-home paths and telemetry-off setting. Require the source SHA above or report the actual newer head and obtain the intended pin from Main.
2. Verify every file against source-manifest.json with SHA256. Record a second manifest after copying fixtures to AppData. Keep that copied source manifest for later comparison.
3. Use a clean launcher profile with optional collection choices explicitly off, through the established test-profile procedure. No production default changes.

## Actual GUI steps

Use real mouse/keyboard input into the unmodified application. Screenshots without the interaction and resulting disk checks do not prove import.

1. Open Instances. Scroll to from other launchers. Click search. Capture rows for qa fabric import, qa no loader import and qa vanished import. Fabric row should offer import. No-loader row has a synthetic jar and should explain missing loader and offer import without mods.
2. Click import on qa fabric import. Wait for completion or a visible error. Capture result. Inspect the saved destination under the isolated game-home instances directory and the isolated launcher-data instances.json. Do not guess the generated instance id. Find it from saved name and recorded result.
3. Verify all source minecraft files except accounts.json, msa_token.json and logs/latest.log exist at matching relative paths and hashes in destination. Verify those three exclusions are absent. Verify icon.png hash matches the source instance-level icon. Verify metadata has Minecraft 1.21.1, Fabric 0.16.9, the fixture name and notes decoded as two lines. Custom kubejs file must be preserved. The source instance.cfg and mmc-pack.json are source metadata, not required game-root copies.
4. Return to Instances. Search again if needed. Click import without mods on qa no loader import. Verify all expected copied files as above except mods/qa.jar must also be absent. No loader must be recorded. Capture the visible choice/result.
5. Search again so qa vanished import is a current row. Outside the app, rename only that fixture's minecraft directory to minecraft-held, then click its already-scanned import without mods button. Capture the visible failure. Verify no usable/listed partial instance is left in the destination/store. Restore minecraft-held back to minecraft before source-hash comparison. This controlled rename is the only intentional source mutation.
6. Close the launcher normally. Reopen using the same isolated homes. Capture Instances and verify both successfully imported instances persist, with names/icons/loader state and destination hashes unchanged. Do not launch the game.
7. Re-hash the complete copied PrismLauncher tree after restoring the vanished fixture. Require byte-for-byte manifest equality and no missing or extra files. Record all comparisons, including exclusions and store state. Cleanup only owned runner fixtures after evidence is retained.

## Pass criteria

All three scenarios must use actual GUI input. Successful own-folder import preserves source bytes and requested game files; credentials/logs are excluded; no-mods choice is honest; missing-source failure is visible and leaves no usable partial instance; saved imports survive restart. Provide raw screenshots, action log, manifests, binary/source hashes and per-check results. Redact nothing by deleting failures. Any unrun step is NOT VERIFIED, not PASS.

This does not prove real mod compatibility, game launch, sign-in, update/install behavior, crash consistency, Windows junction resistance or every launcher format. It is the Prism-format GUI qualification slice only. The source test matrix covers additional shared-folder branches separately.
