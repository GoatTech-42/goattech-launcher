#!/bin/sh
# GoatTech Launcher milestone-1 proof - run on a machine with a full build (2+ CPU, 4GB+ RAM).
# Exercises the REAL launcher CLI (same code the app calls), no sign-in, scratch homes only.
# Prerequisites: rustup stable >= 1.90; Linux: libasound2-dev libudev-dev libxcb1 libxkbcommon0
#   libxkbcommon-x11-0 (or extracted .debs + RUSTFLAGS="-L <dir>"); then: cd launcher && cargo build
set -eu
cd "$(dirname "$0")/../launcher"
BIN=target/debug/horizon-launcher
SCRATCH="$(mktemp -d)"
export HORIZON_LAUNCHER_HOME="$SCRATCH/launcher" HORIZON_HOME="$SCRATCH/game" \
       XDG_DATA_HOME="$SCRATCH/data" XDG_CONFIG_HOME="$SCRATCH/config" \
       HORIZON_LAUNCHER_NO_SELF_UPDATE=1 HORIZON_NO_AUDIO=1
echo "scratch: $SCRATCH"

echo "== java version catalog =="
$BIN instance versions | head -5

echo "== create Vanilla Java instance (latest release) =="
LATEST="$($BIN instance versions --json | python3 -c 'import json,sys; print(json.load(sys.stdin)[0]["id"])')"
$BIN instance create "GT Vanilla" --minecraft "$LATEST"

echo "== create Modded Fabric instance (latest release, newest Fabric loader) =="
$BIN instance create "GT Fabric" --minecraft "$LATEST" --loader fabric

echo "== ViaFabricPlus benchmark install: Modrinth jar into the Fabric instance =="
$BIN mods install viafabricplus --instance "GT Fabric"

echo "== instance isolation check =="
$BIN instance list
FAB_JAR="$(find "$SCRATCH/game/instances/gt-fabric/mods" -iname 'viafabricplus*.jar' | head -1)"
test -n "$FAB_JAR" || { echo "FAIL: ViaFabricPlus jar missing from gt-fabric/mods"; exit 1; }
if find "$SCRATCH/game/instances/gt-vanilla" -iname 'viafabricplus*.jar' | grep -q .; then
  echo "FAIL: ViaFabricPlus jar leaked into gt-vanilla"; exit 1
fi
echo "isolation OK: $FAB_JAR (gt-vanilla clean)"

echo "== assemble launch command (download version json + libraries + runtime, no assets) =="
$BIN instance launch "GT Fabric" --dry-run 2>/dev/null || \
  echo "NOTE: no --dry-run flag; use: $BIN instance launch \"GT Fabric\" and expect auth prompt"

echo "MILESTONE-1 PROOF OK - scope: version catalog, Java instance create (vanilla + Fabric), Modrinth install with CLI checksum, instance isolation. Launch/auth/game start NOT covered; integration gate stays open. (scratch $SCRATCH kept)"
