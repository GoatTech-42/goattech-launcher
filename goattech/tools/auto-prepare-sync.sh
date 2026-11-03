#!/usr/bin/env bash
# auto-prepare-sync.sh - auto-PREPARE upstream sync batches for review (goattech/ARCHITECTURE-SYNC.md, option C).
#
# What it does, in order:
#   1. Take an exclusive lock (one instance at a time; second run exits).
#   2. Fetch upstream horizon-rust/horizon main into refs/remotes/upstream/main.
#   3. Decide: NO MOVEMENT (tip already in main) / ALREADY PREPARED (an existing
#      upstream-sync-* branch contains the tip AND its UPSTREAM-TRACKING.md entry
#      records all gates passed) / CONTAINED-BUT-UNVERIFIED (contained, but gate
#      or tracking state missing or not clean) / new batch needed.
#   4. For a new batch: pin base (main sha) and tip (upstream sha), create an
#      OWNED detached worktree inside .git/auto-prepare-wt/ (the caller's
#      checkout, index and staged files are never touched), cut
#      upstream-sync-<date> there, merge upstream main (--no-ff), run the gates
#      (lib suite, bin suite, fresh CLI build, gt-cli live proof), then write a
#      PENDING/BLOCKED batch summary into goattech/UPSTREAM-TRACKING.md and
#      commit ONLY that file, only if main has not moved since the pin.
#   5. Report a summary on stdout for the operator/review pipeline.
#
# Hard rules (UPSTREAM-TRACKING.md practice): never merges into main, never
# pushes anything, never stashes/resets/deletes caller or pre-existing work.
# Cleanup removes only a worktree this run itself created, after verifying
# path, marker, ref and clean state; anything unexpected is left in place and
# reported. The merge decision stays with review.
#
# Usage:
#   goattech/tools/auto-prepare-sync.sh --check-only   # fetch + decide, no worktree, no builds
#   goattech/tools/auto-prepare-sync.sh                # full prepare (builds; needs a full toolchain box)
# Env: UPSTREAM_URL (default https://github.com/horizon-rust/horizon.git),
#      RUSTFLAGS/PATH as the box needs, SKIP_PROOF=1 to skip the gt-cli live
#      proof (the tracking entry is then marked BLOCKED/incomplete, not PENDING).
set -euo pipefail

UPSTREAM_URL="${UPSTREAM_URL:-https://github.com/horizon-rust/horizon.git}"
UPSTREAM_REF="refs/remotes/upstream/main"
CHECK_ONLY=0
[ "${1:-}" = "--check-only" ] && CHECK_ONLY=1

cd "$(dirname "$0")/../.."
GIT_DIR_ABS="$(git rev-parse --git-common-dir)"
DATE="$(date +%Y%m%d)"
BRANCH="upstream-sync-${DATE}${SYNC_BRANCH_SUFFIX:-}"
LOG="/tmp/auto-prepare-${DATE}-$$.log"
WT_ROOT="${GIT_DIR_ABS}/auto-prepare-wt"
WT="${WT_ROOT}/${BRANCH}-$$"
STATUS=""

say() { echo "auto-prepare: $*"; }

on_interrupt() {
  say "INTERRUPTED: leaving everything in place. Owned worktree (if created): $WT (state unknown, verify before any manual cleanup). Log: $LOG"
  exit 130
}
trap on_interrupt INT TERM

# Exclusive single-instance lock (fd 9 held for the whole run).
mkdir -p "$WT_ROOT"
exec 9>"${GIT_DIR_ABS}/auto-prepare.lock"
if ! flock -n 9; then
  say "another auto-prepare instance holds ${GIT_DIR_ABS}/auto-prepare.lock; exiting."
  exit 1
fi

say "log: $LOG"
say "fetching upstream main from $UPSTREAM_URL"
git fetch --quiet "$UPSTREAM_URL" "main:${UPSTREAM_REF}"
TIP="$(git rev-parse "${UPSTREAM_REF}")"
say "upstream tip: $TIP"

if git merge-base --is-ancestor "$TIP" main; then
  say "NO MOVEMENT: upstream tip is already contained in main. Nothing to do."
  exit 0
fi

# Containment alone is not "prepared": the branch's tracking entry must record
# this tip with all gates passed.
while read -r ref; do
  if git merge-base --is-ancestor "$TIP" "$ref"; then
    MATCHES="$(git show "${ref}:goattech/UPSTREAM-TRACKING.md" 2>/dev/null | grep -cF "$TIP" || true)"
    if [ "$MATCHES" != "1" ]; then
      say "CONTAINED-BUT-UNVERIFIED: $ref contains $TIP but has $MATCHES tracking lines naming it (need exactly one canonical record). Not preparing a duplicate; route $ref for review. Stopping."
      exit 4
    fi
    ENTRY_LINE="$(git show "${ref}:goattech/UPSTREAM-TRACKING.md" 2>/dev/null | grep -F "$TIP")"
    ALL_PASS=1
    for gate in lib-suite bin-suite cli-build gt-cli-proof; do
      printf '%s' "$ENTRY_LINE" | grep -qF "${gate} PASS" || ALL_PASS=0
    done
    if [ "$ALL_PASS" = 1 ] && ! printf '%s' "$ENTRY_LINE" | grep -qiE "SKIPPED|NOT RUN|FAIL|BLOCKED|incomplete"; then
      say "ALREADY PREPARED: $ref contains $TIP and its canonical tracking entry records all four gates passed. Nothing to do."
      exit 0
    fi
    say "CONTAINED-BUT-UNVERIFIED: $ref contains $TIP but its tracking entry is missing or does not record all gates passed. Not preparing a duplicate batch; route $ref for review. Stopping."
    exit 4
  fi
done < <(git for-each-ref --format='%(refname)' 'refs/heads/upstream-sync-*' 'refs/remotes/origin/upstream-sync-*')

BASE="$(git merge-base main "${UPSTREAM_REF}")"
COUNT="$(git rev-list --count "${BASE}..${UPSTREAM_REF}")"
FILES="$(git diff --name-only "${BASE}...${UPSTREAM_REF}" | wc -l | tr -d ' ')"
say "new batch: $COUNT commits since $BASE, $FILES files touched"
[ "$CHECK_ONLY" = 1 ] && { say "check-only: would prepare $BRANCH from pinned main in an owned worktree. Stopping here."; exit 0; }

if git show-ref --verify --quiet "refs/heads/${BRANCH}"; then
  say "branch $BRANCH already exists and its origin is unresolved (tool leftover or human work). Not touching it. For a deliberate same-day rerun, set SYNC_BRANCH_SUFFIX (e.g. SYNC_BRANCH_SUFFIX=-r2) to cut a new uniquely named owned branch."
  exit 1
fi

PIN_MAIN="$(git rev-parse main)"
say "pinned base: main=$PIN_MAIN; tip=$TIP"

# Owned isolated worktree: caller checkout, index and staged files untouched.
# Ownership token lives OUTSIDE the working tree as INERT metadata (common-dir,
# worktree path, branch ref, pins, pid). It is never sourced or executed.
git worktree add --quiet -b "$BRANCH" "$WT" "$PIN_MAIN" >>"$LOG" 2>&1
TOKEN="${WT}.owned"
{
  echo "common_dir=$(cd "$GIT_DIR_ABS" && pwd -P)"
  echo "worktree=$(cd "$WT" && pwd -P)"
  echo "branch=$BRANCH"
  echo "pin_main=$PIN_MAIN"
  echo "tip=$TIP"
  echo "pid=$$"
  echo "created=$(date -Is)"
} > "$TOKEN"

wt_git() { git -C "$WT" "$@"; }

if [ -n "$(wt_git status --porcelain)" ]; then
  say "UNEXPECTED: fresh owned worktree $WT is not clean before starting. Leaving it in place; investigate before any cleanup."
  exit 5
fi

GATES=""
if ! wt_git merge --no-ff --no-edit "$TIP" >>"$LOG" 2>&1; then
  if wt_git status --porcelain | grep -qE '^(UU|AA|DD|U.|.U)'; then KIND="MERGE CONFLICT"; else KIND="MERGE ERROR (not a conflict)"; fi
  if ! wt_git merge --abort >>"$LOG" 2>&1; then
    say "$KIND and merge --abort FAILED. Worktree $WT left exactly as-is; do not touch it until reviewed. Log: $LOG. Hard stop."
    exit 6
  fi
  say "$KIND: merge of $TIP aborted cleanly. Branch $BRANCH stays at main in $WT for inspection; nothing deleted. Review merges by hand. Log: $LOG."
  exit 2
fi
EXPECTED_HEAD="$(wt_git rev-parse HEAD)"
say "merged $TIP into $BRANCH (HEAD $EXPECTED_HEAD)"

run_gate() { # name, command...
  local name="$1"; shift
  say "gate: $name"
  if ( cd "$WT" && "$@" ) >>"$LOG" 2>&1; then GATES="${GATES}; ${name} PASS"; else
    GATES="${GATES}; ${name} FAIL"
    STATUS="BLOCKED at gate: ${name}"
  fi
}

run_gate "lib-suite" cargo test --manifest-path launcher/Cargo.toml --lib
run_gate "bin-suite" cargo test --manifest-path launcher/Cargo.toml --bins
if [ -z "$STATUS" ]; then
  # Always build fresh before the proof: cargo no-ops when up to date, and a
  # stale debug binary can never bypass source that moved.
  run_gate "cli-build" cargo build --manifest-path launcher/Cargo.toml
  if [ -z "$STATUS" ] && [ "${SKIP_PROOF:-0}" != "1" ]; then
    run_gate "gt-cli-proof" sh goattech/proof-m1.sh
  elif [ -z "$STATUS" ]; then
    GATES="${GATES}; gt-cli-proof SKIPPED"
    STATUS="BLOCKED: gt-cli live proof not run (SKIP_PROOF=1); batch incomplete"
  fi
fi

# Pin + HEAD binding: main must not have moved, and the worktree/branch must
# still sit on the exact merge HEAD the gates ran against (flock only excludes
# other instances of THIS script, not other operators).
if [ "$(git rev-parse main)" != "$PIN_MAIN" ]; then
  say "main moved during this run ($PIN_MAIN -> $(git rev-parse main)). Leaving $BRANCH in $WT uncommitted-of-tracking; review before reuse. Log: $LOG."
  exit 7
fi
if [ "$(wt_git rev-parse HEAD)" != "$EXPECTED_HEAD" ] || [ "$(git rev-parse "$BRANCH")" != "$EXPECTED_HEAD" ]; then
  say "HEAD binding failed: worktree or $BRANCH moved off the gated merge HEAD $EXPECTED_HEAD during the run. Leaving $WT in place; nothing committed."
  exit 7
fi

ENTRY_STATUS="PENDING REVIEW (gates:${GATES})"
[ -n "$STATUS" ] && ENTRY_STATUS="${STATUS} (gates:${GATES}); do not merge"
ENTRY="auto-prepared batch (${DATE}): upstream ${TIP}, ${COUNT} commits since ${BASE}, ${FILES} files. ${ENTRY_STATUS}. Gate log: ${LOG}."

# Only the tracking file may be committed, and only if nothing else changed.
TMPENTRY="$(mktemp)"
awk -v entry="$ENTRY" '
  /^Pending batch/ && !done { print "Pending batches:\n\n- " entry; done=1; next }
  /^Last MERGED upstream batch:/ && !done { print; print ""; print "Pending batches:\n\n- " entry; done=1; next }
  { print }
' "${WT}/goattech/UPSTREAM-TRACKING.md" > "$TMPENTRY"
mv "$TMPENTRY" "${WT}/goattech/UPSTREAM-TRACKING.md"

DIRTY="$(wt_git status --porcelain)"
if [ "$DIRTY" != " M goattech/UPSTREAM-TRACKING.md" ]; then
  say "UNEXPECTED worktree state before commit: [$DIRTY]. Expected only the tracking file modified. Leaving $WT in place; nothing committed."
  exit 5
fi
wt_git commit -q -m "auto-prepare: record ${DATE} upstream batch (${TIP:0:9}) as ${ENTRY_STATUS%% (*}" -- goattech/UPSTREAM-TRACKING.md >>"$LOG" 2>&1

# No automatic cleanup: git status cannot see ignored files, so no read proves
# the worktree holds only this run's files, and sourcing metadata would execute
# mutable content. The owned worktree and its inert token are left in place;
# removal is a later explicit human/review action with both locations known.
say "owned worktree left in place (explicit cleanup only): $WT"
say "inert ownership metadata (never sourced): $TOKEN"

say "done: $BRANCH prepared. $ENTRY"
say "next: review, then a reviewed merge decision. This tool never merges to main and never pushes."
[ -n "$STATUS" ] && exit 3 || exit 0
