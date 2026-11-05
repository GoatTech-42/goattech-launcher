//! Basic performance summaries in the launcher (`horizon_telemetry::basic` has the format):
//! who counts as a genuinely new install, the one-time default the window records, the game
//! session's measurements as they arrive, and the single bounded send when the game ends.
//!
//! The CLI, `status`, completions and the MCP server never apply the default: they read the
//! recorded setting and treat its absence as off. Only the window runs [`init_for_gui`], once,
//! from the snapshot taken at the very start of `main`, before anything writes.
use std::path::Path;
use std::time::Instant;

use horizon_telemetry::basic::{self, P95, Report, Ttt, Ttw};

use crate::backend::{self, Settings};

pub use horizon_telemetry::basic::{CONTROL_OFF, EVENT_PREFIX, EVENTS_ENV, MAX_EVENT_LINE};

/// What the very start of `main` saw, before anything (installer, cleanup, settings, CLI)
/// could create or change it: whether this is a genuinely new install. Any doubt - an
/// unreadable, corrupt or unsupported marker - is not new.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    fresh: bool,
}

impl Snapshot {
    /// The first thing `main` does. Read-only.
    pub fn take() -> Self {
        Self::take_from(&backend::data_dir(), &backend::game_home())
    }

    /// New only when none of the markers exist at all: the launcher's own folder, the game's
    /// folder (existing game state counts), its telemetry folder (consent). Anything present -
    /// even a file this build can't read or parse, or a dangling symlink - means an existing
    /// install (`symlink_metadata`: a link itself is a marker, wherever it points).
    pub fn take_from(launcher_dir: &Path, game_dir: &Path) -> Self {
        fn absent(dir: &Path) -> bool {
            match std::fs::symlink_metadata(dir) {
                Ok(_) => false,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
                // An error that isn't "not there" is doubt, and doubt is not new.
                Err(_) => false,
            }
        }
        let fresh = absent(launcher_dir)
            && absent(game_dir)
            && absent(&game_dir.join("telemetry"));
        Self { fresh }
    }

    pub fn fresh(self) -> bool {
        self.fresh
    }
}

/// How the setting got its first value, kept beside it so the default decision never re-runs.
/// Local only: never part of a report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Cohort {
    /// A genuinely new install: on by default, disclosed on the first-run screen.
    #[serde(rename = "fresh-default-on")]
    FreshDefaultOn,
    /// Anything else: off, and never turned on by an upgrade.
    #[serde(rename = "existing-default-off")]
    ExistingDefaultOff,
}

/// The window's one-time initialization from the snapshot: records the setting explicitly
/// (with its cohort) when the file doesn't say. This is a guarded JSON-object patch, not a
/// typed load-and-save: unknown and unrelated keys survive untouched, a corrupt or
/// non-object file is left alone (doubt is off, and nothing is written), an existing key in
/// ANY form - a player's earlier choice, a concurrent first launch that wrote first, a newer
/// build's cohort this one can't parse - is never overwritten, and the file is replaced
/// atomically. A write failure changes nothing.
pub fn init_for_gui(snapshot: Snapshot) {
    init_in(&backend::data_dir().join("settings.json"), snapshot.fresh());
}

/// The file's object, when it may be patched: a missing file patches from empty; an
/// unreadable or non-UTF-8 file, corrupt JSON, or a non-object is doubt - left exactly as it
/// is, nothing written, the setting stays off. None also when the keys are decided already,
/// in ANY form (a player's earlier choice, another launcher's write, a newer build's cohort):
/// the default never re-runs and never overwrites.
fn patchable(path: &Path) -> Option<serde_json::Map<String, serde_json::Value>> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(_) => return None,
    };
    let value: serde_json::Value = if text.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(&text).ok()?
    };
    let fields = value.as_object()?;
    if fields.contains_key("basic_summaries") || fields.contains_key("basic_summaries_cohort") {
        return None;
    }
    Some(fields.clone())
}

fn init_in(path: &Path, fresh: bool) {
    let Some(dir) = path.parent() else { return };
    // The folder first, so the lock file can exist at all (a genuinely new install has
    // neither). A failure here means the write below fails too, and nothing changes.
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    // The shared settings write lock serializes this patch against typed saves and other
    // first-run inits. If it cannot be taken (another writer holds it), the default simply
    // stays unwritten this launch - off - rather than risk overwriting a recorded choice.
    let Some(_guard) = backend::settings_write_lock(dir) else { return };
    // Under the lock: read and check NOW, so a write that landed between the first look and
    // this one - another launcher recording the player's choice - always wins.
    if let Some(mut fields) = patchable(path) {
        fields.insert("basic_summaries".to_owned(), serde_json::Value::Bool(fresh));
        fields.insert(
            "basic_summaries_cohort".to_owned(),
            serde_json::Value::String(
                if fresh {
                    "fresh-default-on"
                } else {
                    "existing-default-off"
                }
                .to_owned(),
            ),
        );
        // Whole-then-replace, like Settings::save: a half write never becomes the file.
        static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let tmp = dir.join(format!(
            "settings.{}.{}.tmp",
            std::process::id(),
            TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let written = std::fs::write(
            &tmp,
            serde_json::to_string_pretty(&serde_json::Value::Object(fields)).unwrap(),
        )
        .and_then(|()| std::fs::rename(&tmp, path));
        if written.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
    // The OS lock releases when the guard drops; the lock file itself stays.
}

/// Whether basic summaries are on: only an explicit `true` the window recorded. The CLI and
/// every other entry point see a missing field as off and never write it. A corrupt file or
/// a value of the wrong shape parses to no setting (`Settings::parse`), so it is off too.
pub fn enabled() -> bool {
    Settings::load().basic_summaries == Some(true)
}

/// One game session's measurements as its events arrive, bound to the measured game build:
/// the game's hello (its own release) must come first, and the launcher's own clock times the
/// title and world events from the game's start, so the pipe - not the game's say-so -
/// carries them. A session completes only on the game's normal-end marker; a crash or a
/// forced quit never emits one, so it never completes.
pub struct Session {
    started: Instant,
    hello: Option<String>,
    title: Option<Ttt>,
    world: Option<Ttw>,
    p95: Option<P95>,
    ended: bool,
}

impl Session {
    /// The session starts when the launcher starts the game.
    pub fn start(now: Instant) -> Self {
        Self {
            started: now,
            hello: None,
            title: None,
            world: None,
            p95: None,
            ended: false,
        }
    }

    /// One line of the game's output. Returns true when the line is a protocol frame -
    /// consumed, so the caller must keep it out of the user's log and anything shared from
    /// it. Frames are strict and bounded: an exact prefix, a length cap, typed JSON with no
    /// unknown fields, the hello first, duplicates ignored. A malformed frame is consumed
    /// (never logged, never parsed into measurements) all the same.
    pub fn on_line(&mut self, line: &str) -> bool {
        if !line.starts_with(EVENT_PREFIX) {
            return false;
        }
        if line.len() > MAX_EVENT_LINE {
            return true;
        }
        let event = &line[EVENT_PREFIX.len()..];
        let Ok(serde_json::Value::Object(fields)) = serde_json::from_str(event) else {
            return true;
        };
        let elapsed = || self.started.elapsed().as_millis() as u64;
        match fields.get("e").and_then(|e| e.as_str()) {
            // The game introduces itself once: its own release, which the report names and
            // the allowlist gates. Measurements before it are not accepted.
            Some("hello") if fields.len() == 2 && self.hello.is_none() => {
                if let Some(v) = fields.get("v").and_then(|v| v.as_str()) {
                    if v.len() <= 40 {
                        self.hello = Some(v.to_owned());
                    }
                }
            }
            // First title frame drawn: time it from the game's start, here.
            Some("title") if fields.len() == 1 && self.hello.is_some() => {
                if self.title.is_none() {
                    self.title = Ttt::from_ms(elapsed());
                }
            }
            // First world finished loading (its first playing frame).
            Some("world") if fields.len() == 1 && self.hello.is_some() => {
                if self.world.is_none() {
                    self.world = Ttw::from_ms(elapsed());
                }
            }
            // The game's p95 frame time over the session, from its own bounded histogram; only
            // the game sees frame times. Counted over enough frames, in range, else absent.
            Some("frames") if fields.len() == 3 && self.hello.is_some() => {
                let p95 = fields.get("p95_ms").and_then(|v| v.as_f64());
                let n = fields.get("n").and_then(|v| v.as_u64()).unwrap_or(0);
                if n >= basic::MIN_P95_FRAMES && self.p95.is_none() {
                    if let Some(bucket) = p95.and_then(P95::from_frame_ms) {
                        self.p95 = Some(bucket);
                    }
                }
            }
            // The game's normal end: emitted only from its ordinary shutdown path, so a
            // crash or a forced quit leaves the session incomplete, and nothing is sent.
            Some("end") if fields.len() == 1 && self.hello.is_some() => {
                self.ended = true;
            }
            _ => {}
        }
        true
    }

    /// The report this session makes, if any: the process exited successfully AND the game
    /// reached its normal end (both, so an error exit after a marker, or a kill before one,
    /// sends nothing), the switch on, at least one measurement, the measured build's release
    /// published, a named OS. Off means no report was ever assembled.
    pub fn finish(&self, exit_ok: bool) -> Option<Report> {
        if !exit_ok || !self.ended || !enabled() {
            return None;
        }
        Report::build(self.hello.as_deref()?, self.title, self.world, self.p95)
    }

    /// The game ended while the launcher is alive: the one send attempt, `exit_ok` from the
    /// process's exit status. A Session is consumed here, and the launcher makes one per game
    /// run, so a second send is structurally impossible; turning the switch off drops the
    /// Session itself (and tells the game to stop collecting), so a canceled sample cannot
    /// be resurrected by the switch coming back on. The send thread re-reads the setting and
    /// the veto right before dispatch (an in-flight request cannot be unsent), is never
    /// joined and never delays shutdown. No report sends nothing, and a failure or refusal
    /// is dropped: no retry, no outbox, nothing on disk.
    pub fn session_ended(self, exit_ok: bool) {
        let Some(report) = self.finish(exit_ok) else {
            return;
        };
        std::thread::spawn(move || {
            // The switch or the veto could have flipped between the game ending and now;
            // `basic::send` decides the destination and enforces it again at the boundary.
            if enabled() && !basic::transport_vetoed() {
                let _ = basic::send(&report);
            }
        });
    }
}

/// One output line from a job: true when the line carries the reserved protocol prefix -
/// consumed, so it must never be appended to the user's log or to anything shared from it
/// (the console page's Copy/Share). The prefix is stripped whether or not a session is still
/// active: the switch going off drops the Session, not the frames the game had queued, and a
/// frame after the drop is still machine protocol, never log. Only an active session parses.
/// Ordinary log lines return false and flow to the log untouched.
pub fn consume_event_line(session: &mut Option<Session>, job_is_client: bool, line: &str) -> bool {
    if !job_is_client || !line.starts_with(EVENT_PREFIX) {
        return false;
    }
    if let Some(s) = session.as_mut() {
        s.on_line(line);
    }
    true
}

/// The game command gets the events switch when a session will be consumed (on, and transport
/// not vetoed). The Java path never sets it: there is no event plumbing for it, so no metrics
/// and no report.
pub fn command_with_events(cmd: &mut std::process::Command) -> bool {
    if enabled() && !basic::transport_vetoed() {
        cmd.env(EVENTS_ENV, "1");
        return true;
    }
    false
}

#[cfg(test)]
mod tests;
