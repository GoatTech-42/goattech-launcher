#!/usr/bin/env bash
# auto-prepare-sync.sh - auto-PREPARE upstream sync batches for review (goattech/ARCHITECTURE-SYNC.md, option C).
#
# What it does, in order:
#   1. Fetch upstream horizon-rust/horizon main into refs/remotes/upstream/main.
#   2. Decide: no movement / already prepared (an existing upstream-sync-* branch
#      contains the tip) / new batch needed.
#   3. For a new batch: cut upstream-sync-<date> off main, merge upstream main
#      (--no-ff), run the gates (lib suite, bin suite, gt-cli live proof via
#      goattech/proof-m1.sh), then write a PENDING batch summary into
#      goattech/UPSTREAM-TRACKING.md and commit it ON THE SYNC BRANCH.
#   4. Report a summary on stdout for the operator/review pipeline.
#
# Hard rules (UPSTREAM-TRACKING.md practice): never merges into main, never
# pushes anything anywhere, never resolves conflicts beyond stopping and
# reporting them. The merge decision stays with review.
#
# Usage:
#   goattech/tools/auto-prepare-sync.sh --check-only   # fetch + decide, no branch, no builds
#   goattech/tools/auto-prepare-sync.sh                # full prepare (builds; needs a full toolchain box)
# Env: UPSTREAM_URL (default https://github.com/horizon-rust/horizon.git),
#      RUSTFLAGS/PATH as the box needs, SKIP_PROOF=1 to skip the gt-cli live proof.
set -euo pipefail

UPSTREAM_URL="${UPSTREAM_URL:-https://github.com/horizon-rust/horizon.git}"
UPSTREAM_REF="refs/remotes/upstream/main"
CHECK_ONLY=0
[ "${1:-}" = "--check-only" ] && CHECK_ONLY=1

cd "$(dirname "$0")/../.."
DATE="$(date +%Y%m%d)"
BRANCH="upstream-sync-${DATE}"
LOG="/tmp/auto-prepare-${DATE}.log"
STATUS=""

say() { echo "auto-prepare: $*"; }

say "fetching upstream main from $UPSTREAM_URL"
git fetch --quiet "$UPSTREAM_URL" "main:${UPSTREAM_REF}"
TIP="$(git rev-parse "${UPSTREAM_REF}")"
say "upstream tip: $TIP"

if git merge-base --is-ancestor "$TIP" main; then
  say "NO MOVEMENT: upstream tip is already contained in main. Nothing to do."
  exit 0
fi

while read -r ref; do
  if git merge-base --is-ancestor "$TIP" "$ref"; then
    say "ALREADY PREPARED: $ref contains upstream tip $TIP (batch pending review). Nothing to do."
    exit 0
  fi
done < <(git for-each-ref --format='%(refname)' 'refs/heads/upstream-sync-*' 'refs/remotes/origin/upstream-sync-*' | awk '{print $1}')

BASE="$(git merge-base main "${UPSTREAM_REF}")"
COUNT="$(git rev-list --count "${BASE}..${UPSTREAM_REF}")"
FILES="$(git diff --name-only "${BASE}...${UPSTREAM_REF}" | wc -l | tr -d ' ')"
say "new batch: $COUNT commits since $BASE, $FILES files touched"
[ "$CHECK_ONLY" = 1 ] && { say "check-only: would cut $BRANCH off main, merge $TIP, run gates, write PENDING entry. Stopping here."; exit 0; }

if git show-ref --verify --quiet "refs/heads/${BRANCH}"; then
  say "branch $BRANCH already exists; refusing to reuse it (a same-day rerun needs a human look)."
  exit 1
fi

git checkout -q -b "$BRANCH" main
GATES="not run"
if ! git merge --no-ff --no-edit "${UPSTREAM_REF}" >>"$LOG" 2>&1; then
  git merge --abort || true
  git checkout -q main
  say "MERGE CONFLICT: merge of $TIP onto main conflicts (log: $LOG). Merge aborted; $BRANCH left in place (points at main) for inspection - this tool never deletes a branch or worktree. Review merges by hand. Stopping."
  exit 2
fi
say "merged $TIP into $BRANCH"

run_gate() { # name, command...
  local name="$1"; shift
  say "gate: $name"
  if "$@" >>"$LOG" 2>&1; then GATES="${GATES}; ${name} PASS"; else
    GATES="${GATES}; ${name} FAIL"
    STATUS="BLOCKED at gate: ${name}"
  fi
}

run_gate "lib-suite" cargo test --manifest-path launcher/Cargo.toml --lib
run_gate "bin-suite" cargo test --manifest-path launcher/Cargo.toml --bins
if [ -z "$STATUS" ] && [ "${SKIP_PROOF:-0}" != "1" ]; then
  [ -x launcher/target/debug/horizon-launcher ] || run_gate "cli-build" cargo build --manifest-path launcher/Cargo.toml
  [ -z "$STATUS" ] && run_gate "gt-cli-proof" sh goattech/proof-m1.sh
fi

ENTRY_STATUS="PENDING REVIEW (gates${GATES})"
[ -n "$STATUS" ] && ENTRY_STATUS="BLOCKED - ${STATUS} (gates${GATES}); do not merge"
ENTRY="auto-prepared batch (${DATE}): upstream ${TIP}, ${COUNT} commits since ${BASE}, ${FILES} files. ${ENTRY_STATUS}. Gate log: ${LOG}."
TMPENTRY="$(mktemp)"
awk -v entry="$ENTRY" '
  /^Pending batch/ && !done { print "Pending batches:\n\n- " entry; done=1; next }
  /^Last MERGED upstream batch:/ && !done { print; print ""; print "Pending batches:\n\n- " entry; done=1; next }
  { print }
' goattech/UPSTREAM-TRACKING.md > "$TMPENTRY"
mv "$TMPENTRY" goattech/UPSTREAM-TRACKING.md
git add goattech/UPSTREAM-TRACKING.md
git commit -q -m "auto-prepare: record ${DATE} upstream batch (${TIP:0:9}) as ${ENTRY_STATUS%% (*}"

say "done: $BRANCH prepared. $ENTRY"
say "next: review, then a reviewed merge decision. This tool never merges to main and never pushes."
[ -n "$STATUS" ] && exit 3 || exit 0
