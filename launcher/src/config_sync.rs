//! Synced settings across instances: opt-in "sync groups" that share individual `options.txt`
//! settings (keybinds, video, controls) between the instances a player chooses. Settings move
//! one by one, in both directions, each against its own remembered baselines - so the side a
//! change happened on is always known. The file is never copied or linked whole.
//!
//! The protections, each load-bearing:
//! - A group only exchanges values with an instance whose `version` options-format marker
//!   matches the group's. Different formats mean keys and values may mean different things;
//!   nothing moves between them. A side WITHOUT a marker is unknown, and unknown is not
//!   compatible: it refuses too, on every route, including unknown on both sides. The
//!   group's marker rides in the shared file itself, so a crash can never split the format
//!   from the values. (The marker never syncs itself.) A group whose format is not yet
//!   established is untrusted as a whole: a joining instance's real file establishes it by
//!   REPLACING the group's unmarked bytes (backed up, never relabelled compatible) with its
//!   own settings, and only there. A fresh instance that joins receives NOTHING: its game
//!   folder is left untouched until its own game's first run has created a real file with
//!   its own marker, and the group's settings land on the first pass after that, because
//!   values copied before the format is known could be settings the game misreads.
//! - A pull only ever rewrites settings the instance's file already has. Group settings the
//!   file doesn't know are never added - a mod's key reaches the instances that have the mod.
//! - A change on both sides of one setting is a conflict: nothing moves until the player
//!   picks a side. Conflicts belong to the instance that hit them; another instance can
//!   never clear or overwrite them.
//! - Before a launch the sync only pulls, and it neither overwrites nor adopts a local edit
//!   it sees: the change stays pending for the next trusted pass. Pushing happens after a
//!   clean exit (the game has rewritten its file in full) or when the player runs the sync
//!   by hand - so a crashed game's torn or truncated file never reaches the group and is
//!   never forgotten either.
//! - A pass never writes an instance's file while its game is running: the launcher
//!   claims the game folder's mark BEFORE the game starts (an exclusive per-run token -
//!   a live mark is never overwritten or taken over, and a run that cannot claim it says
//!   so instead of pretending to be protected), records the game process's pid once it is
//!   known, and releases the mark at the game's exit. A mark reads as running while EITHER
//!   the launcher or the game process lives, so a launcher that dies while its game keeps
//!   running still blocks writes; only a mark whose processes are both gone is a crashed
//!   launcher's leftover, cleaned up rather than obeyed forever. A synced instance whose
//!   mark cannot be claimed (another live run holds it, or the claim itself fails) does not
//!   launch at all: a second, untracked game in the same folder would be rewritten by a
//!   pass once the tracked run exits. Recording the game's pid is fail-closed too: a mark
//!   that is missing, foreign, unreadable, or unwritable is an error, and the just-started
//!   game is stopped rather than left running with protection that was never recorded.
//!   Stopping is confirmed, never claimed: the mark is released only once the game's exit
//!   is verified, and if stopping or confirming fails the mark becomes a non-expiring hold
//!   (and the GUI keeps tracking that exact run, so a late exit report never touches a
//!   newer run's mark). A failure path claims that hold only after VERIFYING it on disk -
//!   re-establishing it if the mark file was lost, never overwriting another run's claim -
//!   and a guard that cannot be verified is reported as lost, never claimed. A game that
//!   is neither confirmed stopped nor durably guarded is RETAINED by its owner - the stop
//!   and the guard are retried, and if neither lands the owner waits for the game's actual
//!   exit - never dropped live and unguarded.
//! - Every write is written out and flushed to a fresh unique temporary file, then renamed
//!   onto the target; the local file is re-read and compared before it is replaced, and a
//!   file that moved mid-sync is left for the next pass (best-effort, not an atomic
//!   compare-and-swap). Writes are validated in full before anything moves and ordered
//!   local-first, so a mid-pass failure either wrote nothing or says exactly what it wrote.
//!   Before any file is replaced an exact backup with a unique name is kept - the group's
//!   file too, not only the instance's.
//! - A group serializes its writers with an OS advisory lock where the platform has one
//!   (the kernel releases a dead holder's lock; there is no stale-lock guessing), so two
//!   instances or terminals can never interleave a read-change-write.
//! - A file that doesn't parse as options is left exactly as found; values that aren't
//!   UTF-8 round-trip byte-exact. Untouched lines keep their bytes, line endings, blanks
//!   and final-newline state. Corrupt metadata - the conflict record, a remembered
//!   baseline, the instance store - stops the pass with an error before anything moves.
//!   The instance store itself is read strictly (a corrupt file is an error, never a
//!   silent reset to empty), written atomically, and serialized across writers by every
//!   mutating path; a pass's baselines are persisted only over the membership the pass
//!   started from, so a mid-pass join or --off is never silently reverted.
//!
//! Scope, plainly: options.txt settings only (not servers, resource packs or hotbars), CLI
//! only (the window has no picker yet), all keys of a matching-format file, no per-key
//! opt-out list yet.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The file a group syncs, in each instance's game folder.
pub const OPTIONS_FILE: &str = "options.txt";
/// Backups sit next to the file as `options.txt.gtsync-backup-<seconds>-<nanos>-<pid>`.
pub const BACKUP_INFIX: &str = ".gtsync-backup-";
/// Backups older than these, per file, are cleaned up (ours only, by name).
const KEEP_BACKUPS: usize = 5;

/// Which way a pass may move settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    /// Before a launch: pull only. The file on disk may be a crashed game's torn write, so
    /// nothing it holds is trusted into the group.
    PreLaunch,
    /// After a clean exit: the game rewrote its file in full, so changes push and pull.
    CleanExit,
    /// The player ran the sync by hand: both directions.
    Manual,
}

impl Pass {
    fn may_push(self) -> bool {
        !matches!(self, Pass::PreLaunch)
    }
}

/// An instance's membership in a group, kept in its `Meta` in `instances.json`: the group's
/// name and the two baselines the next pass compares against, so a change is attributed to
/// the side it happened on.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sync {
    pub group: String,
    /// The group's shared settings as this instance last saw them.
    #[serde(default)]
    pub shared: BTreeMap<String, Bytes>,
    /// Its own file's values for those settings at the same moment.
    #[serde(default)]
    pub local: BTreeMap<String, Bytes>,
}

/// A setting's raw bytes kept losslessly: text when valid UTF-8, hex otherwise. Parsing hex
/// back is checked (even length, valid digits), never a panic path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Bytes {
    Text(String),
    Hex { hex: String },
}

impl Bytes {
    fn of(raw: &[u8]) -> Bytes {
        match std::str::from_utf8(raw) {
            Ok(text) => Bytes::Text(text.to_string()),
            Err(_) => Bytes::Hex {
                hex: raw.iter().map(|b| format!("{b:02x}")).collect(),
            },
        }
    }
    fn raw(&self) -> Option<Vec<u8>> {
        match self {
            Bytes::Text(text) => Some(text.as_bytes().to_vec()),
            Bytes::Hex { hex } => {
                if hex.len() % 2 != 0 {
                    return None;
                }
                (0..hex.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
                    .collect()
            }
        }
    }
    /// For the player's terminal: text, or a hex note for bytes that aren't text.
    pub fn show(&self) -> String {
        match self {
            Bytes::Text(text) => text.clone(),
            Bytes::Hex { hex } => format!("(bytes: {hex})"),
        }
    }
}

/// One physical line of an options file, byte-exact: untouched lines write back untouched,
/// including their line ending. A `\r` is structure, never part of a value.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Line {
    /// key bytes, value bytes (no `\r`), whether the line ended CRLF.
    Pair(Vec<u8>, Vec<u8>, bool),
    /// Any other line (blank lines included), exactly as read, line ending included.
    Raw(Vec<u8>),
}

/// A parsed options file. Parsing preserves every byte; a file with content but no
/// `key:value` line at all is not an options file and is refused, never rewritten.
#[derive(Clone, Debug)]
struct OptionsFile {
    lines: Vec<Line>,
    /// Whether the file ended with a newline.
    final_newline: bool,
}

impl OptionsFile {
    fn parse(bytes: &[u8]) -> Result<OptionsFile, String> {
        let final_newline = bytes.last() == Some(&b'\n') || bytes.is_empty();
        let body = bytes.strip_suffix(b"\n").unwrap_or(bytes);
        let mut lines = Vec::new();
        let mut pairs = 0;
        for raw in body.split(|&b| b == b'\n') {
            let (content, crlf) = match raw.strip_suffix(b"\r") {
                Some(stripped) => (stripped, true),
                None => (raw, false),
            };
            match content.iter().position(|&b| b == b':') {
                Some(at) if !content[..at].is_empty() => {
                    lines.push(Line::Pair(
                        content[..at].to_vec(),
                        content[at + 1..].to_vec(),
                        crlf,
                    ));
                    pairs += 1;
                }
                _ => lines.push(Line::Raw(raw.to_vec())),
            }
        }
        if !body.is_empty() && pairs == 0 {
            return Err("the file has no key:value lines - not an options file".to_string());
        }
        Ok(OptionsFile {
            lines,
            final_newline,
        })
    }

    /// Key (UTF-8) -> value bytes. A duplicated key's first line wins, as the game reads it.
    fn values(&self) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        for line in &self.lines {
            if let Line::Pair(key, value, _) = line {
                if let Ok(key) = std::str::from_utf8(key) {
                    out.entry(key.to_string()).or_insert_with(|| value.clone());
                }
            }
        }
        out
    }

    /// The `version` line's value (the writer's options-format marker), if present.
    fn format_version(&self) -> Option<Vec<u8>> {
        self.values().get("version").cloned()
    }

    /// The file with `changes` applied to keys IT ALREADY HAS (a duplicate line folds into
    /// the first). No key is ever added here; every untouched line keeps its exact bytes.
    fn with_changes(&self, changes: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
        let mut done: Vec<String> = Vec::new();
        let mut out = Vec::new();
        let mut lines = self.lines.iter().peekable();
        while let Some(line) = lines.next() {
            match line {
                Line::Pair(key, value, crlf) => {
                    let key_text = String::from_utf8_lossy(key).into_owned();
                    let (new_value, mark) = match changes.get(&key_text) {
                        Some(new) if !done.contains(&key_text) => (Some(new), true),
                        Some(_) => (None, true), // a duplicate folds into the first
                        None => (None, false),
                    };
                    if mark {
                        done.push(key_text);
                        if let Some(new) = new_value {
                            out.extend(key.iter());
                            out.push(b':');
                            out.extend(new.iter());
                        } else {
                            continue; // folded duplicate: no line at all
                        }
                    } else {
                        out.extend(key.iter());
                        out.push(b':');
                        out.extend(value.iter());
                    }
                    if *crlf {
                        out.push(b'\r');
                    }
                }
                Line::Raw(raw) => out.extend(raw.iter()),
            }
            if lines.peek().is_some() || self.final_newline {
                out.push(b'\n');
            }
        }
        out
    }
}

/// What a pass did, for the player's terminal and the window's log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// The group was created from this instance's file.
    pub seeded: bool,
    /// Settings written into the instance's file from the group.
    pub pulled: Vec<String>,
    /// Settings written into the group from the instance's file.
    pub pushed: Vec<String>,
    /// Settings changed on both sides since the last pass; neither side was written.
    pub conflicts: Vec<Conflict>,
    /// Things the player should know (a format mismatch holding the group back, a torn
    /// file left alone). Nothing here is an error; the pass completed.
    pub notes: Vec<String>,
}

impl Report {
    /// One line per thing that happened, for the terminal; empty when nothing did.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.seeded {
            out.push("the group started from this instance's settings".to_string());
        }
        if !self.pulled.is_empty() {
            out.push(format!("from the group: {}", self.pulled.join(", ")));
        }
        if !self.pushed.is_empty() {
            out.push(format!("to the group: {}", self.pushed.join(", ")));
        }
        for c in &self.conflicts {
            out.push(format!(
                "conflict on {}: yours {:?}, the group's {:?} - `instance sync ... --resolve` picks one",
                c.key,
                c.local.show(),
                c.shared.show()
            ));
        }
        out.extend(self.notes.iter().cloned());
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub key: String,
    pub local: Bytes,
    pub shared: Bytes,
}

/// Which side of a join's differing settings wins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The instance's own values become the group's (the group's file is backed up first).
    Mine,
    /// The group's values land in the instance (its file is backed up first).
    Theirs,
}

/// A group name is one path piece: letters, digits, space, `-_.`; it never leaves the sync
/// folder.
fn group_dir(data: &Path, group: &str) -> Result<PathBuf, String> {
    let ok = !group.is_empty()
        && group.len() <= 64
        && group
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'));
    if !ok || group.starts_with('.') {
        return Err(format!(
            "A group's name is letters, digits, spaces and - _ . (not {group:?})"
        ));
    }
    Ok(data.join("sync").join(group))
}

fn shared_file(data: &Path, group: &str) -> Result<PathBuf, String> {
    Ok(group_dir(data, group)?.join(OPTIONS_FILE))
}

fn conflicts_file(data: &Path, group: &str) -> Result<PathBuf, String> {
    Ok(group_dir(data, group)?.join("conflicts.json"))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Seconds-nanos-pid, zero-padded so name order is time order (backup pruning relies on
/// it) and two writers in the same second never share a name.
fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    format!("{}-{:018}-{}", now_secs(), nanos, std::process::id())
}

/// A token lock file: the conservative writer serialization on platforms without `flock`.
/// Always compiled so every platform's tests exercise it; production uses it only where the
/// OS has no advisory locks. Only the token's owner may release it; there is no takeover -
/// a leftover lock refuses the pass and names itself.
#[allow(dead_code)] // production uses it on non-unix only; tests use it everywhere
#[derive(Debug)]
pub(crate) struct TokenLock {
    path: PathBuf,
    token: String,
}

#[allow(dead_code)]
impl TokenLock {
    pub(crate) fn take(path: PathBuf) -> Result<TokenLock, String> {
        let token = unique_suffix();
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut f) => {
                if let Err(e) = f.write_all(token.as_bytes()).and_then(|()| f.sync_all()) {
                    let _ = fs::remove_file(&path);
                    return Err(format!("{}: {e}", path.display()));
                }
                Ok(TokenLock { path, token })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(format!(
                "{}: held by another writer (or left by a crash) - nothing was written; \
                 remove it only if no sync is running",
                path.display()
            )),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }
}

#[allow(dead_code)]
impl Drop for TokenLock {
    fn drop(&mut self) {
        // Release only our own token: a lock we no longer own is never deleted.
        if fs::read(&self.path).ok().as_deref() == Some(self.token.as_bytes()) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Serializes a group's writers for one operation. On unix this is an OS advisory lock
/// (`flock`) on the group's `.lock` file: the kernel releases it when the holder's process
/// dies, so there is no stale-lock guessing, no takeover, and no unlink race. A bounded
/// retry (about a second) keeps two players' terminals from queueing on each other. On
/// other platforms a `TokenLock` does the serializing.
#[derive(Debug)]
struct GroupLock {
    #[cfg(unix)]
    _file: fs::File,
    #[cfg(not(unix))]
    _token: TokenLock,
}

#[cfg(unix)]
impl GroupLock {
    fn take(data: &Path, group: &str) -> Result<GroupLock, String> {
        use std::os::unix::io::AsRawFd;
        let dir = group_dir(data, group)?;
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join(".lock"))
            .map_err(|e| format!("{}: {e}", dir.join(".lock").display()))?;
        let fd = file.as_raw_fd();
        for _ in 0..20 {
            // SAFETY: flock on a valid open fd; no memory is touched.
            if (unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) }) == 0 {
                return Ok(GroupLock { _file: file });
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        Err(format!(
            "sync group {group:?} is mid-sync elsewhere; nothing was written - try again in a moment"
        ))
    }
}

#[cfg(not(unix))]
impl GroupLock {
    fn take(data: &Path, group: &str) -> Result<GroupLock, String> {
        let dir = group_dir(data, group)?;
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(GroupLock {
            _token: TokenLock::take(dir.join(".lock"))?,
        })
    }
}

/// The marker a launcher process leaves in the game folder while the game runs. It holds a
/// per-run token, the launcher's pid, and - once known - the game process's pid, so a
/// launcher that dies while its game keeps running still reads as RUNNING.
const RUNNING_FILE: &str = ".horizon-game-running";
/// Second field of a fail-closed mark: an earlier game could not be confirmed stopped, so
/// the folder is held until a person removes the mark after the game has fully quit.
const UNSAFE_MARK: &str = "unsafe";

/// One launcher run's exclusive claim on its game folder's sync mark. Released when the
/// game exits; only the token's owner can update or release it.
#[derive(Debug)]
pub struct RunMark {
    path: PathBuf,
    token: String,
}

impl RunMark {
    /// The mark as written at claim time, BEFORE the game starts: a non-expiring hold.
    /// Protection never depends on writing a new guard at failure time - the durable
    /// conservative state is what exists from the first moment.
    fn pending(&self) -> String {
        format!("{} {UNSAFE_MARK}", self.token)
    }

    /// The mark once the game's pid is durably recorded: stale-cleanable only from here
    /// on, when BOTH named processes are gone.
    fn content(&self, child: u32) -> String {
        format!("{} {} {}", self.token, std::process::id(), child)
    }

    /// Records the game process's pid once it is known (right after spawn), so the mark
    /// stays live even if the launcher itself dies first. This is fail-closed: a mark
    /// that is missing, foreign, unreadable, or unwritable is an ERROR, never a silent
    /// Ok - the caller must stop the just-spawned game rather than let it run with
    /// protection that was never recorded.
    pub fn set_child(&self, pid: u32) -> Result<(), String> {
        let current = fs::read_to_string(&self.path)
            .map_err(|e| format!("{}: sync mark unreadable: {e}", self.path.display()))?;
        if current != self.pending() {
            return Err(format!(
                "{}: the sync mark changed or is held by another run; the game pid was NOT \
                 recorded",
                self.path.display()
            ));
        }
        write_atomic(&self.path, self.content(pid).as_bytes())
            .map_err(|e| format!("{}: game pid not recorded: {e}", self.path.display()))
    }

    /// Returns this run's recorded mark to the non-expiring hold: used when a game whose
    /// pid WAS recorded can no longer be observed. A missing, foreign, unreadable, or
    /// unwritable mark is an ERROR - the caller must surface that durable protection could
    /// not be secured, never drop the guard silently. (A run whose pid was never recorded
    /// needs no call: its mark is the durable hold already.)
    pub fn fail_closed(&self) -> Result<(), String> {
        let current = fs::read_to_string(&self.path)
            .map_err(|e| format!("{}: sync mark unreadable: {e}", self.path.display()))?;
        if current.split_whitespace().next() != Some(self.token.as_str()) {
            return Err(format!(
                "{}: the sync mark changed or is held by another run; the hold was NOT \
                 restored",
                self.path.display()
            ));
        }
        write_atomic(&self.path, self.pending().as_bytes()).map_err(|e| {
            format!(
                "{}: could not restore the sync hold: {e}",
                self.path.display()
            )
        })
    }

    /// Verifies this run's durable hold is really on disk AND protective, rebuilding
    /// it when it is not. Only the EXACT non-expiring pending form verifies.
    /// Anything else carrying our token - the recorded pid form, a torn write, a
    /// numeric-shaped corruption - is rewritten to the pending hold: protection is
    /// never taken from syntax alone, and the rewritten hold never expires on its
    /// own. A missing mark is re-created as that hold with create-new, so another
    /// run's live claim is never overwritten. A foreign, unreadable, or unwritable
    /// mark is an ERROR - a caller must never claim sync is held when this fails.
    pub fn ensure_hold(&self) -> Result<(), String> {
        match fs::read_to_string(&self.path) {
            Ok(text) => {
                if text.split_whitespace().next() != Some(self.token.as_str()) {
                    return Err(format!(
                        "{}: the sync mark is held by another run; this run's hold is lost",
                        self.path.display()
                    ));
                }
                if text == self.pending() {
                    return Ok(());
                }
                write_atomic(&self.path, self.pending().as_bytes()).map_err(|e| {
                    format!(
                        "{}: could not re-establish the sync hold: {e}",
                        self.path.display()
                    )
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let created = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&self.path)
                    .and_then(|mut f| {
                        f.write_all(self.pending().as_bytes())
                            .and_then(|()| f.sync_all())
                    });
                match created {
                    Ok(()) => Ok(()),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        // Lost the race to another claim: only our own exact pending
                        // hold counts.
                        match fs::read_to_string(&self.path) {
                            Ok(text) if text == self.pending() => Ok(()),
                            _ => Err(format!(
                                "{}: the sync mark is held by another run; this run's \
                                 hold is lost",
                                self.path.display()
                            )),
                        }
                    }
                    // A failed create must not leave a partial file a later retry would
                    // "verify": remove it (best effort) and report the failure.
                    Err(e) => {
                        let _ = fs::remove_file(&self.path);
                        Err(format!(
                            "{}: could not re-establish the sync hold: {e}",
                            self.path.display()
                        ))
                    }
                }
            }
            Err(e) => Err(format!(
                "{}: sync mark unreadable: {e}",
                self.path.display()
            )),
        }
    }

    /// Clears the mark at the game's exit. Only this run's own token is ever removed. A
    /// removal failure is an error the caller must surface: the leftover mark holds sync
    /// until a person removes it.
    pub fn release(self) -> Result<(), String> {
        let ours = fs::read_to_string(&self.path)
            .ok()
            .is_some_and(|t| t.split_whitespace().next() == Some(self.token.as_str()));
        if ours {
            match fs::remove_file(&self.path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(format!(
                        "{}: could not remove the sync mark: {e} - sync for this instance \
                         is held until it is removed",
                        self.path.display()
                    ))
                }
            }
        }
        Ok(())
    }
}

/// Claims the game folder's sync mark for this run, BEFORE the game starts. The mark is
/// written as a non-expiring hold from the first moment; it becomes stale-cleanable only
/// once `set_child` has durably recorded the game's pid. Ok(None) means another live run
/// holds it: the caller must refuse the launch, never proceed unprotected.
pub fn claim_running(home: &Path) -> Result<Option<RunMark>, String> {
    let path = home.join(RUNNING_FILE);
    let mark = RunMark {
        path: path.clone(),
        token: unique_suffix(),
    };
    let bytes = mark.pending();
    for attempt in 0..2 {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut f) => {
                if let Err(e) = f.write_all(bytes.as_bytes()).and_then(|()| f.sync_all()) {
                    let _ = fs::remove_file(&path);
                    return Err(format!("{}: {e}", path.display()));
                }
                return Ok(Some(mark));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir_all(home).map_err(|e| format!("{}: {e}", home.display()))?;
                continue;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if attempt == 0 && mark_is_stale(&path)? {
                    let _ = fs::remove_file(&path);
                    continue;
                }
                return Ok(None);
            }
            Err(e) => return Err(format!("{}: {e}", path.display())),
        }
    }
    Ok(None)
}

/// A mark is stale only when NEITHER pid it names is alive: the launcher and the game both
/// gone. On platforms without a liveness check every mark counts as live (conservative).
fn mark_is_stale(path: &Path) -> Result<bool, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    if text.split_whitespace().nth(1) == Some(UNSAFE_MARK) {
        // A fail-closed mark never expires on its own.
        return Ok(false);
    }
    let mut pids = text
        .split_whitespace()
        .skip(1) // the token
        .filter_map(|p| p.parse::<u32>().ok());
    Ok(!pids.any(process_alive))
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    // SAFETY: kill with signal 0 only checks existence; it sends nothing.
    (unsafe { libc::kill(pid as i32, 0) }) == 0
        || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
fn process_alive(_pid: u32) -> bool {
    true // no portable liveness check: any mark counts as running (conservative)
}

/// Refuses the operation while this instance's game is running: the game rewrites
/// options.txt on exit, so writing under it loses one side's changes. A mark whose
/// processes are both gone is a crashed launcher's leftover and is cleaned up.
fn guard_not_running(home: &Path) -> Result<(), String> {
    let path = home.join(RUNNING_FILE);
    if !path.exists() {
        return Ok(());
    }
    if mark_is_stale(&path)? {
        let _ = fs::remove_file(&path);
        return Ok(());
    }
    if fs::read_to_string(&path)
        .ok()
        .as_deref()
        .and_then(|t| t.split_whitespace().nth(1))
        == Some(UNSAFE_MARK)
    {
        return Err(format!(
            "this instance's game is starting or an earlier one was not confirmed stopped. \
             Quit it before syncing - nothing was written. If no game is running, remove \
             {} to lift the hold",
            path.display()
        ));
    }
    Err("this instance's game is running. Quit it before syncing - nothing was written".to_string())
}

/// Writes `bytes` to `path` durably and atomically: a fresh unique same-folder temporary
/// (create-new, never another writer's name), flushed to disk, then renamed over the target.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_file_name(format!(
        "{}.gtsync-tmp-{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        unique_suffix()
    ));
    let written = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| format!("{}: {e}", tmp.display()))?;
        f.write_all(bytes)
            .and_then(|()| f.sync_all())
            .map_err(|e| format!("{}: {e}", tmp.display()))
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("{}: {e}", path.display()));
    }
    Ok(())
}

/// An exact copy of the file next to itself before any sync write touches it, under a
/// unique name (two backups in one second can never overwrite each other). Backups a sync
/// made (by the BACKUP_INFIX name) are pruned to the newest KEEP_BACKUPS per file.
fn backup(path: &Path) -> Result<PathBuf, String> {
    let to = path.with_file_name(format!(
        "{}{BACKUP_INFIX}{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        unique_suffix()
    ));
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    write_atomic(&to, &bytes)?;
    prune_backups(path);
    Ok(to)
}

fn prune_backups(path: &Path) {
    let prefix = format!(
        "{}{BACKUP_INFIX}",
        path.file_name().unwrap_or_default().to_string_lossy(),
    );
    let Some(dir) = path.parent() else { return };
    let mut ours: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .starts_with(&prefix)
                })
                .collect()
        })
        .unwrap_or_default();
    ours.sort();
    while ours.len() > KEEP_BACKUPS {
        let _ = fs::remove_file(ours.remove(0));
    }
}

fn read_shared(data: &Path, group: &str) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let path = shared_file(data, group)?;
    match fs::read(&path) {
        Ok(bytes) => Ok(OptionsFile::parse(&bytes)?.values()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// The group's options-format marker: the shared file's own `version` line. The marker and
/// the values are one file and one atomic write, so a crash can never leave a group whose
/// format is unknown while its values exist.
fn group_format(data: &Path, group: &str) -> Result<Option<Vec<u8>>, String> {
    let path = shared_file(data, group)?;
    match fs::read(&path) {
        Ok(bytes) => Ok(OptionsFile::parse(&bytes)?.format_version()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// The gate every mutation route passes before anything moves. Only markers both present
/// and equal are compatible. A missing marker on either side is UNKNOWN, and unknown never
/// exchanges values: not a different format, not an absent one, not both absent. The one
/// place a format becomes established is `join`, from a real instance file's marker.
fn format_gate(group_v: Option<&[u8]>, local_v: Option<&[u8]>) -> Result<(), String> {
    match (group_v, local_v) {
        (Some(g), Some(l)) if g == l => Ok(()),
        (Some(g), Some(l)) => Err(format!(
            "the group's settings are for options format {}, this instance's file is {} - \
             formats differ, so nothing moves between them",
            String::from_utf8_lossy(g),
            String::from_utf8_lossy(l)
        )),
        (Some(_), None) => Err(
            "this instance's options file has no `version` marker, so its format is unknown \
             and unknown is not compatible with the group's - nothing moves (the game writes \
             the marker on its next run)"
                .to_string(),
        ),
        (None, Some(_)) => Err(
            "the group's options format is not established yet - nothing moves (join the \
             group again from an instance whose file has a `version` marker to establish it)"
                .to_string(),
        ),
        (None, None) => Err(
            "neither the group nor this instance has an options-format marker - nothing \
             moves until the game writes one"
                .to_string(),
        ),
    }
}

/// The single path a sync write reaches the instance's file through. The bytes are
/// re-read and compared with what the pass based its plan on - a file that moved (the
/// game, an editor, another terminal) means nothing is written and the next pass retries.
/// Then an exact unique backup, then the unique-tmp + fsync + rename. This is best-effort
/// against a concurrent writer, not an atomic compare-and-swap; a running game is refused
/// earlier by `guard_not_running`.
fn write_local_guarded(
    path: &Path,
    original: &[u8],
    file: &OptionsFile,
    changes: &BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    let now = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if now != original {
        return Err(format!(
            "{} changed while syncing; nothing was written - the next pass picks it up",
            path.display()
        ));
    }
    backup(path)?;
    write_atomic(path, &file.with_changes(changes))
}

fn write_shared(
    data: &Path,
    group: &str,
    values: &BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    let path = shared_file(data, group)?;
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| format!("{}: {e}", path.display()))?;
    if path.exists() {
        backup(&path)?; // the group's file is data too; never replaced without a copy
    }
    let file = OptionsFile {
        lines: values
            .iter()
            .map(|(k, v)| Line::Pair(k.as_bytes().to_vec(), v.clone(), false))
            .collect(),
        final_newline: true,
    };
    write_atomic(&path, &file.with_changes(&BTreeMap::new()))
}

fn read_local(home: &Path) -> Result<Option<(Vec<u8>, OptionsFile)>, String> {
    let path = home.join(OPTIONS_FILE);
    match fs::read(&path) {
        Ok(bytes) => {
            let file = OptionsFile::parse(&bytes)?;
            Ok(Some((bytes, file)))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// A conflict a group is holding, per instance and setting. Only the instance that hit a
/// conflict can resolve it; another instance's resolve never touches it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictNote {
    pub local: Bytes,
    pub shared: Bytes,
    pub at: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ConflictStore {
    /// instance id -> setting -> what the two sides hold.
    #[serde(default)]
    conflicts: BTreeMap<String, BTreeMap<String, ConflictNote>>,
}

fn load_conflicts(data: &Path, group: &str) -> Result<ConflictStore, String> {
    let path = conflicts_file(data, group)?;
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| {
            format!(
                "{}: unreadable conflict record: {e} - nothing was written",
                path.display()
            )
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ConflictStore::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn save_conflicts(data: &Path, group: &str, store: &ConflictStore) -> Result<(), String> {
    let path = conflicts_file(data, group)?;
    let text = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    write_atomic(&path, text.as_bytes())
}

/// The conflicts one instance is holding in its group, for `instance sync --status`.
pub fn conflicts(
    data: &Path,
    group: &str,
    instance: &str,
) -> Result<BTreeMap<String, ConflictNote>, String> {
    Ok(load_conflicts(data, group)?
        .conflicts
        .remove(instance)
        .unwrap_or_default())
}

/// Joins the instance whose game folder is `home` to `group`. A group that doesn't exist
/// yet is seeded from this instance's file (the joining instance is the starting point) and
/// remembers its options format. Joining an existing group: the formats must match, then -
/// if any setting the file already holds differs from the group's - `take` says which side
/// wins; without it the join reports the differing settings and writes nothing. The group's
/// settings never add new keys to the instance's file.
pub fn join(
    data: &Path,
    home: &Path,
    group: &str,
    take: Option<Side>,
) -> Result<(Sync, Report), String> {
    let _lock = GroupLock::take(data, group)?;
    guard_not_running(home)?;
    let mut report = Report::default();
    let local = read_local(home)?;
    let shared_path = shared_file(data, group)?;
    let mut shared = read_shared(data, group)?;
    if !shared_path.exists() {
        // A new group seeds from this instance's file, format marker and values in ONE
        // write (the `version` line stays in the shared file), so a crash can never leave
        // a group with values but no known format.
        let seed = match &local {
            Some((_, file)) => file.values(),
            None => BTreeMap::new(),
        };
        write_shared(data, group, &seed)?;
        shared = seed;
        report.seeded = true;
    } else {
        let group_v = group_format(data, group)?;
        let local_v = local.as_ref().and_then(|(_, f)| f.format_version());
        // Establishment: a group whose format was never established takes BOTH its format
        // and its values from a joining instance's real file - the old unmarked bytes are
        // never relabelled compatible; they are replaced (with a backup) by the one source
        // whose format is actually known. Nothing else about the group is trusted.
        let establishing = group_v.is_none() && local_v.is_some() && local.is_some();
        if local.is_some() && !establishing {
            // The gate runs before anything moves: a different or unknown format is a
            // refusal, never a conversion and never a blind copy.
            format_gate(group_v.as_deref(), local_v.as_deref())?;
        }
        // EVERYTHING validates before anything is written: the conflict record parses, and
        // a needed side choice was made - "nothing was written" is always true on an error.
        load_conflicts(data, group)?;
        let effective: BTreeMap<String, Vec<u8>> = if establishing {
            local.as_ref().unwrap().1.values()
        } else {
            shared.clone()
        };
        match &local {
            Some((original_bytes, file)) => {
                let values = file.values();
                let differing: Vec<String> = effective
                    .iter()
                    .filter(|(k, _)| k.as_str() != "version")
                    .filter(|(k, v)| values.get(*k).is_some_and(|mine| mine != *v))
                    .map(|(k, _)| k.clone())
                    .collect();
                if !differing.is_empty() {
                    match take {
                        None => {
                            return Err(format!(
                                "the group and this instance differ on: {}. Join with \
                                 --take theirs (the group's values land, yours are backed up) \
                                 or --take mine (your values become the group's, the group's \
                                 file is backed up). Nothing was written.",
                                differing.join(", ")
                            ));
                        }
                        Some(Side::Theirs) => {
                            let changes: BTreeMap<String, Vec<u8>> = differing
                                .iter()
                                .map(|k| (k.clone(), effective.get(k).unwrap().clone()))
                                .collect();
                            let path = home.join(OPTIONS_FILE);
                            write_local_guarded(&path, original_bytes, file, &changes)?;
                            report.pulled = differing;
                        }
                        Some(Side::Mine) => {
                            for k in &differing {
                                shared.insert(k.clone(), values.get(k).unwrap().clone());
                            }
                            write_shared(data, group, &shared)?;
                            report.pushed = differing;
                        }
                    }
                }
                if establishing {
                    write_shared(data, group, &effective)?;
                    shared = effective;
                    report.notes.push(
                        "the group's options format and settings are now established from \
                         this instance's file; the previous unestablished group file was \
                         backed up, not trusted"
                            .to_string(),
                    );
                }
            }
            None => {
                // No file yet (a fresh instance): NOTHING is written into its game folder.
                // Settings land only on the first pass after the game's own first run has
                // created a real options file with its own format marker - values copied
                // before the format is known could be settings the game misreads.
                report.notes.push(
                    "sync holds until the game's first run creates its options file; the \
                     group's settings land on the first pass after that"
                        .to_string(),
                );
            }
        }
    }
    let local_values = read_local(home)?
        .map(|(_, f)| f.values())
        .unwrap_or_default();
    let membership = Sync {
        group: group.to_string(),
        shared: shared
            .iter()
            .filter(|(k, _)| k.as_str() != "version")
            .map(|(k, v)| (k.clone(), Bytes::of(v)))
            .collect(),
        // Every setting in the file is baselined, not only the group's: an untouched
        // personal setting must never read as an edit on the next pass.
        local: local_values
            .iter()
            .filter(|(k, _)| k.as_str() != "version")
            .map(|(k, v)| (k.clone(), Bytes::of(v)))
            .collect(),
    };
    Ok((membership, report))
}

/// One pass for an instance already in a group. Pull only rewrites settings the file
/// already holds; push (CleanExit/Manual only) writes the instance's changed settings into
/// the group. A setting new to the file that the group already holds adopts the group's
/// value rather than overwriting it with a fresh default. Changed on both sides = conflict,
/// nothing moves. A format mismatch holds the whole pass with a note, never a conversion.
pub fn sync(
    data: &Path,
    home: &Path,
    instance: &str,
    membership: &mut Sync,
    pass: Pass,
) -> Result<Report, String> {
    let _lock = GroupLock::take(data, &membership.group)?;
    guard_not_running(home)?;
    let mut report = Report::default();
    let local = read_local(home)?;
    let mut shared = read_shared(data, &membership.group)?;

    // Everything the pass depends on is validated before anything is written: the conflict
    // record must parse and every remembered baseline must decode, or the pass errors with
    // nothing moved.
    let mut conflict_store = load_conflicts(data, &membership.group)?;
    for (key, baseline) in membership.shared.iter().chain(membership.local.iter()) {
        if baseline.raw().is_none() {
            return Err(format!(
                "the remembered baseline for {key:?} is corrupt - nothing was written. \
                 Leaving and rejoining the group rebuilds baselines from the current files."
            ));
        }
    }

    // The format gate holds the whole pass (a held pass is not a failed launch, so this is
    // a note, not an error): different or unknown formats never exchange values.
    let group_v = group_format(data, &membership.group)?;
    let local_v = local.as_ref().and_then(|(_, f)| f.format_version());
    if local.is_some() {
        if let Err(why) = format_gate(group_v.as_deref(), local_v.as_deref()) {
            report
                .notes
                .push(format!("{why} - the group is held for this instance"));
            return Ok(report);
        }
    }

    let local_values: BTreeMap<String, Vec<u8>> =
        local.as_ref().map(|(_, f)| f.values()).unwrap_or_default();
    let keys: std::collections::BTreeSet<String> = shared
        .keys()
        .chain(membership.shared.keys())
        .chain(membership.local.keys())
        .chain(local_values.keys())
        .filter(|k| k.as_str() != "version")
        .cloned()
        .collect();

    let mut shared_changed = false;
    let mut local_changes: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    // A change the pass did not move keeps its baselines, so it stays pending for the next
    // trusted pass instead of being silently adopted (a pre-launch pass defers local edits
    // rather than forgetting them; its bytes may be a crashed game's torn write).
    let mut deferred: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for key in keys {
        let base_shared = membership.shared.get(&key).and_then(Bytes::raw);
        let base_local = membership.local.get(&key).and_then(Bytes::raw);
        let cur_shared = shared.get(&key).cloned();
        let cur_local = local_values.get(&key).cloned();
        let sc = cur_shared != base_shared;
        let lc = cur_local != base_local;
        match (sc, lc) {
            (false, false) => {}
            (true, false) => {
                // Pull only into a setting the file already holds.
                if let (Some(value), Some(mine)) = (cur_shared, cur_local) {
                    if mine != value {
                        local_changes.insert(key.clone(), value);
                        report.pulled.push(key.clone());
                    }
                } // a group-side deletion deletes only the group's copy
            }
            (false, true) => {
                match (base_local, cur_shared, cur_local) {
                    // A key the file just gained (a mod installed, a new game default):
                    // the group's standing value wins, not the fresh default.
                    (None, Some(value), Some(mine)) if mine != value => {
                        local_changes.insert(key.clone(), value);
                        report.pulled.push(key.clone());
                    }
                    (base, _, Some(value)) if pass.may_push() => {
                        let _ = base;
                        shared.insert(key.clone(), value);
                        shared_changed = true;
                        report.pushed.push(key.clone());
                    }
                    // A pre-launch pass never trusts the file upward and never adopts the
                    // change as the new baseline either: the edit stays pending until a
                    // trusted pass (clean exit or manual) can move it.
                    _ => {
                        if !pass.may_push() {
                            deferred.insert(key.clone());
                        }
                    }
                } // a local deletion never empties the group's setting
            }
            (true, true) => {
                if cur_shared == cur_local {
                    continue; // both moved to the same value: converged
                }
                if base_local.is_none() {
                    // The file never held this setting at baseline: it just appeared (a
                    // mod installed, a new game default). The group's standing value is
                    // adopted, not fought over.
                    if let (Some(value), Some(mine)) = (cur_shared, cur_local) {
                        if mine != value {
                            local_changes.insert(key.clone(), value);
                            report.pulled.push(key.clone());
                        }
                    }
                    continue;
                }
                if pass.may_push() {
                    report.conflicts.push(Conflict {
                        key: key.clone(),
                        local: cur_local
                            .as_deref()
                            .map(Bytes::of)
                            .unwrap_or(Bytes::Text(String::new())),
                        shared: cur_shared
                            .as_deref()
                            .map(Bytes::of)
                            .unwrap_or(Bytes::Text(String::new())),
                    });
                } else {
                    // Pre-launch bytes may be torn: they raise no conflict and keep their
                    // baselines, so a trusted pass re-evaluates with the real file.
                    deferred.insert(key.clone());
                }
            }
        }
    }

    // Writes happen local-first, and the local file is re-checked against what the plan
    // was based on before anything is written: if it moved (the game, an editor, another
    // terminal) the pass stops BEFORE the group is touched, so "nothing was written" is
    // true. If the group write then fails, the next pass re-derives the same pulls and
    // pushes from the unchanged baselines - recoverable, never a torn group.
    let path = home.join(OPTIONS_FILE);
    if !local_changes.is_empty() {
        match &local {
            Some((original_bytes, file)) => {
                write_local_guarded(&path, original_bytes, file, &local_changes)?;
            }
            None => return Err(format!("{}: no file", path.display())),
        }
    }
    if shared_changed {
        write_shared(data, &membership.group, &shared).map_err(|e| {
            format!(
                "{e} - the instance's file WAS already updated ({}); the next pass retries \
                 the group write",
                report.pulled.join(", ")
            )
        })?;
    }
    if !report.conflicts.is_empty() {
        let mine = conflict_store
            .conflicts
            .entry(instance.to_string())
            .or_default();
        for c in &report.conflicts {
            mine.insert(
                c.key.clone(),
                ConflictNote {
                    local: c.local.clone(),
                    shared: c.shared.clone(),
                    at: now_secs(),
                },
            );
        }
        save_conflicts(data, &membership.group, &conflict_store)
            .map_err(|e| format!("settings moved but the conflict record failed: {e}"))?;
    }

    // New baselines: what both sides hold now. A conflicted or deferred key keeps its old
    // baselines, so it still reads as pending on the next pass.
    let shared_now = read_shared(data, &membership.group)?;
    let local_now = read_local(home)?
        .map(|(_, f)| f.values())
        .unwrap_or_default();
    let held: std::collections::BTreeSet<&str> = report
        .conflicts
        .iter()
        .map(|c| c.key.as_str())
        .chain(deferred.iter().map(|k| k.as_str()))
        .collect();
    let mut next_shared: BTreeMap<String, Bytes> = membership.shared.clone();
    for (k, v) in &shared_now {
        if !held.contains(k.as_str()) && k.as_str() != "version" {
            next_shared.insert(k.clone(), Bytes::of(v));
        }
    }
    let mut next_local: BTreeMap<String, Bytes> = membership.local.clone();
    for k in shared_now
        .keys()
        .chain(membership.local.keys())
        .chain(local_now.keys())
    {
        if !held.contains(k.as_str()) && k.as_str() != "version" {
            match local_now.get(k) {
                Some(v) => next_local.insert(k.clone(), Bytes::of(v)),
                None => next_local.remove(k),
            };
        }
    }
    membership.shared = next_shared;
    membership.local = next_local;
    if pass == Pass::PreLaunch && !report.pushed.is_empty() {
        unreachable!("a pre-launch pass never pushes");
    }
    Ok(report)
}

/// Picks a side for one of THIS instance's conflicted settings, or all of them with "all".
/// The chosen side is written over the other (an exact backup first either way); another
/// instance's conflicts are never touched.
pub fn resolve(
    data: &Path,
    home: &Path,
    instance: &str,
    membership: &mut Sync,
    key: &str,
    use_local: bool,
) -> Result<Vec<String>, String> {
    let _lock = GroupLock::take(data, &membership.group)?;
    guard_not_running(home)?;
    // The gate covers resolve too: a conflict recorded while the formats matched must not
    // be settled after the instance's file moved to another format.
    let group_v = group_format(data, &membership.group)?;
    let local_v = read_local(home)?.and_then(|(_, f)| f.format_version());
    format_gate(group_v.as_deref(), local_v.as_deref())?;
    // Every input validates before anything is written: the conflict record parses (via
    // `conflicts`) and every remembered baseline decodes.
    for (key, baseline) in membership.shared.iter().chain(membership.local.iter()) {
        if baseline.raw().is_none() {
            return Err(format!(
                "the remembered baseline for {key:?} is corrupt - nothing was written. \
                 Leaving and rejoining the group rebuilds baselines from the current files."
            ));
        }
    }
    let held = conflicts(data, &membership.group, instance)?;
    let keys: Vec<String> = if key == "all" {
        held.keys().cloned().collect()
    } else if held.contains_key(key) {
        vec![key.to_string()]
    } else {
        return Err(format!(
            "No conflicted setting {key:?} on this instance (holding: {})",
            held.keys().cloned().collect::<Vec<_>>().join(", ")
        ));
    };
    if keys.is_empty() {
        return Err("No conflicts are waiting on this instance.".to_string());
    }
    let mut done = Vec::new();
    for key in keys {
        if use_local {
            let value = read_local(home)?
                .and_then(|(_, f)| f.values().get(&key).cloned())
                .ok_or_else(|| format!("The instance's file has no {key:?}"))?;
            let mut shared = read_shared(data, &membership.group)?;
            shared.insert(key.clone(), value);
            write_shared(data, &membership.group, &shared).map_err(|e| {
                if done.is_empty() {
                    e
                } else {
                    format!(
                        "{e} - already settled before this error: {}",
                        done.join(", ")
                    )
                }
            })?;
        } else {
            let shared = read_shared(data, &membership.group)?;
            let value = shared
                .get(&key)
                .ok_or_else(|| format!("The group has no {key:?}"))?
                .clone();
            let path = home.join(OPTIONS_FILE);
            let (original_bytes, file) =
                read_local(home)?.ok_or_else(|| format!("{}: no file", path.display()))?;
            let mut changes = BTreeMap::new();
            changes.insert(key.clone(), value);
            write_local_guarded(&path, &original_bytes, &file, &changes).map_err(|e| {
                if done.is_empty() {
                    e
                } else {
                    format!(
                        "{e} - already settled before this error: {}",
                        done.join(", ")
                    )
                }
            })?;
        }
        // Baselines move to the written result for this key; the conflict is cleared for
        // this instance only.
        let shared_now = read_shared(data, &membership.group)?;
        let local_now = read_local(home)?
            .map(|(_, f)| f.values())
            .unwrap_or_default();
        if let Some(v) = shared_now.get(&key) {
            membership.shared.insert(key.clone(), Bytes::of(v));
        }
        match local_now.get(&key) {
            Some(v) => membership.local.insert(key.clone(), Bytes::of(v)),
            None => membership.local.remove(&key),
        };
        let mut store = load_conflicts(data, &membership.group)?;
        if let Some(mine) = store.conflicts.get_mut(instance) {
            mine.remove(&key);
            if mine.is_empty() {
                store.conflicts.remove(instance);
            }
            save_conflicts(data, &membership.group, &store)?;
        }
        done.push(key);
    }
    Ok(done)
}

/// Whether a membership has anything to move right now, cheaply: the group file or the
/// instance's file differs from its baseline. The window uses this to skip quiet instances.
pub fn pending(data: &Path, home: &Path, membership: &Sync) -> bool {
    let shared = read_shared(data, &membership.group).unwrap_or_default();
    let local = read_local(home)
        .ok()
        .flatten()
        .map(|(_, f)| f.values())
        .unwrap_or_default();
    let keys: std::collections::BTreeSet<&String> = shared
        .keys()
        .chain(membership.shared.keys())
        .chain(membership.local.keys())
        .chain(local.keys())
        .filter(|k| k.as_str() != "version")
        .collect();
    let any = keys.into_iter().any(|k| {
        let cur_s = shared.get(k).cloned();
        let cur_l = local.get(k).cloned();
        cur_s != membership.shared.get(k).and_then(Bytes::raw)
            || cur_l != membership.local.get(k).and_then(Bytes::raw)
    });
    any
}

#[cfg(test)]
mod tests;
