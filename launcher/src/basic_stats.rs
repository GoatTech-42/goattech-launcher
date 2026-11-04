//! Basic performance summaries in the launcher (`horizon_telemetry::basic` has the format):
//! who counts as a genuinely new install, the one-time default the window records, the game
//! session's measurements as they arrive, and the single bounded send when the game ends.
//!
//! The CLI, `status`, completions and the MCP server never apply the default: they read the
//! recorded setting and treat its absence as off. Only the window runs [`init_for_gui`], once,
//! from the snapshot taken at the very start of `main`, before anything writes.
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use horizon_telemetry::basic::{self, P95, Report, Ttt, Ttw};

use crate::backend::{self, Settings};

/// The line prefix the game uses for its session events, only ever emitted when the launcher
/// asked with [`EVENTS_ENV`]: a machine protocol on the game's output pipe, not a log to
/// scrape. Anything else on the pipe is the game's log and is never parsed for this.
pub const EVENT_PREFIX: &str = "HORIZON-SESSION-EVENT ";

/// Set to `1` on the game's environment when a session's events will be consumed.
pub const EVENTS_ENV: &str = "HORIZON_SESSION_EVENTS";

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
    /// even a file this build can't read or parse - means an existing install.
    pub fn take_from(launcher_dir: &Path, game_dir: &Path) -> Self {
        fn absent(dir: &Path) -> bool {
            match std::fs::metadata(dir) {
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
/// (with its cohort) when the file doesn't say, preserving everything else in it. After this
/// the setting is never inferred again. Existing installs and any doubt record `false`.
pub fn init_for_gui(snapshot: Snapshot) {
    let mut settings = Settings::load();
    if settings.basic_summaries.is_some() {
        return;
    }
    let on = snapshot.fresh();
    settings.basic_summaries = Some(on);
    settings.basic_summaries_cohort = Some(if on {
        Cohort::FreshDefaultOn
    } else {
        Cohort::ExistingDefaultOff
    });
    settings.save();
}

/// Whether basic summaries are on: only an explicit `true` the window recorded. The CLI and
/// every other entry point see a missing field as off and never write it.
pub fn enabled() -> bool {
    Settings::load().basic_summaries == Some(true)
}

/// One game session's measurements as its events arrive, with the launcher's own clock: the
/// times are how long after the launcher started the game each event arrived, so the pipe -
/// not the game's say-so - carries them.
pub struct Session {
    started: Instant,
    title: Option<Ttt>,
    world: Option<Ttw>,
    p95: Option<P95>,
    /// The one send for this session went out (or was attempted): there is no second.
    sent: AtomicBool,
    /// What would be sent, between the game ending and the send thread picking it up. Turning
    /// the switch off clears it; nothing here ever touches disk.
    pending: Mutex<Option<Report>>,
}

impl Session {
    /// The session starts when the launcher starts the game.
    pub fn start(now: Instant) -> Self {
        Self {
            started: now,
            title: None,
            world: None,
            p95: None,
            sent: AtomicBool::new(false),
            pending: Mutex::new(None),
        }
    }

    /// A line of the game's output. Only exact [`EVENT_PREFIX`] frames are events; everything
    /// else is the game's log and ignored.
    pub fn on_line(&mut self, line: &str) {
        let Some(event) = line.strip_prefix(EVENT_PREFIX) else {
            return;
        };
        let Ok(serde_json::Value::Object(fields)) = serde_json::from_str(event) else {
            return;
        };
        let elapsed = || self.started.elapsed().as_millis() as u64;
        match fields.get("e").and_then(|e| e.as_str()) {
            // First title frame drawn: time it from the game's start, here.
            Some("title") => {
                if self.title.is_none() {
                    self.title = Ttt::from_ms(elapsed());
                }
            }
            // First world finished loading (its first playing frame).
            Some("world") => {
                if self.world.is_none() {
                    self.world = Ttw::from_ms(elapsed());
                }
            }
            // The game's p95 frame time over the session, from its own bounded histogram; only
            // the game sees frame times. Counted over enough frames, in range, else absent.
            Some("frames") => {
                let p95 = fields.get("p95_ms").and_then(|v| v.as_f64());
                let n = fields.get("n").and_then(|v| v.as_u64()).unwrap_or(0);
                if n >= basic::MIN_P95_FRAMES {
                    self.p95 = p95.and_then(P95::from_frame_ms);
                }
            }
            _ => {}
        }
    }

    /// The game ended. The report it makes, if any: at least one measurement, a published
    /// release, a named OS. Off means no report was ever assembled.
    pub fn finish(&self) -> Option<Report> {
        if !enabled() {
            return None;
        }
        Report::build(backend::VERSION, self.title, self.world, self.p95)
    }

    /// The game ended while the launcher is alive: at most one send attempt, on its own
    /// thread, never joined, so it cannot delay shutdown. The setting is read again right
    /// before dispatch; in flight cannot be unsent. No report (off, or nothing measured)
    /// sends nothing.
    pub fn session_ended(self: &std::sync::Arc<Self>) {
        let Some(report) = self.finish() else {
            return;
        };
        // At most once, whatever races: a second caller finds the flag set.
        if self.sent.swap(true, Ordering::SeqCst) {
            return;
        }
        // The switch could have flipped after the game ended: read it again at dispatch.
        if !enabled() {
            return;
        }
        let Some(url) = basic::endpoint() else {
            return; // the transport veto is on
        };
        *self.pending.lock().unwrap() = Some(report);
        let this = std::sync::Arc::clone(self);
        std::thread::spawn(move || {
            let report = this.pending.lock().unwrap().take();
            if let Some(report) = report {
                let _ = basic::send_to(&url, &report);
            }
        });
    }

    /// The switch was turned off: anything queued is dropped at once.
    pub fn cancel_pending(&self) {
        *self.pending.lock().unwrap() = None;
    }
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
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dirs(name: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "horizon-basic-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        (base.join("launcher"), base.join("game"))
    }

    #[test]
    fn only_a_truly_empty_machine_is_new() {
        let (launcher, game) = dirs("fresh");
        assert!(Snapshot::take_from(&launcher, &game).fresh());
        // The launcher's own folder existing (settings, anything) means existing.
        std::fs::create_dir_all(&launcher).unwrap();
        assert!(!Snapshot::take_from(&launcher, &game).fresh());
        std::fs::remove_dir_all(&launcher).unwrap();
        // Existing game state counts.
        std::fs::create_dir_all(game.join("saves")).unwrap();
        assert!(!Snapshot::take_from(&launcher, &game).fresh());
        std::fs::remove_dir_all(&game).unwrap();
        // A bare telemetry folder (a consent answer) counts too.
        std::fs::create_dir_all(game.join("telemetry")).unwrap();
        assert!(!Snapshot::take_from(&launcher, &game).fresh());
        std::fs::remove_dir_all(&game).unwrap();
        // A corrupt file where settings would be is doubt, and doubt is not new: the folder
        // exists at all, so this is already covered, but pin the file case too.
        std::fs::create_dir_all(&launcher).unwrap();
        std::fs::write(launcher.join("settings.json"), b"{not json").unwrap();
        assert!(!Snapshot::take_from(&launcher, &game).fresh());
    }

    #[test]
    fn a_session_reads_only_its_own_event_frames() {
        let mut s = Session::start(Instant::now());
        // Log lines, even about frames, are not events.
        s.on_line("Horizon: first frame after 812 ms");
        assert!(s.title.is_none());
        s.on_line("HORIZON-SESSION-EVENT {not json");
        assert!(s.title.is_none());
        s.on_line("HORIZON-SESSION-EVENT {\"e\":\"title\"}");
        assert!(s.title.is_some());
        s.on_line("HORIZON-SESSION-EVENT {\"e\":\"world\"}");
        assert!(s.world.is_some());
        // p95 needs its sample count and domain: too few frames, zero and NaN are absent.
        s.on_line("HORIZON-SESSION-EVENT {\"e\":\"frames\",\"p95_ms\":16.9,\"n\":12}");
        assert!(s.p95.is_none());
        s.on_line("HORIZON-SESSION-EVENT {\"e\":\"frames\",\"p95_ms\":0.0,\"n\":300}");
        assert!(s.p95.is_none());
        s.on_line("HORIZON-SESSION-EVENT {\"e\":\"frames\",\"p95_ms\":16.9,\"n\":300}");
        assert_eq!(s.p95, Some(P95::B16_7));
        s.on_line("HORIZON-SESSION-EVENT {\"e\":\"frames\",\"p95_ms\":1001.0,\"n\":300}");
        assert_eq!(s.p95, Some(P95::B16_7), "an earlier valid value is kept");
    }

    #[test]
    fn at_most_one_send_per_session() {
        let s = std::sync::Arc::new(Session::start(Instant::now()));
        s.sent.store(true, Ordering::SeqCst);
        // A second attempt after the flag is a no-op regardless of a report.
        let again = std::sync::Arc::clone(&s);
        again.session_ended();
        assert!(s.pending.lock().unwrap().is_none());
    }
}
