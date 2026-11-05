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

/// Environment-reading checks (`enabled`, a real send) run one at a time under this lock,
/// each with its own HORIZON_LAUNCHER_HOME, restoring the variable after.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn hello(s: &mut Session, release: &str) {
    let v = serde_json::to_string(release).unwrap();
    s.on_line(&format!("HORIZON-SESSION-EVENT {{\"e\":\"hello\",\"v\":{v}}}"));
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
    // A corrupt file where settings would be is doubt, and doubt is not new.
    std::fs::create_dir_all(&launcher).unwrap();
    std::fs::write(launcher.join("settings.json"), b"{not json").unwrap();
    assert!(!Snapshot::take_from(&launcher, &game).fresh());
    std::fs::remove_dir_all(&launcher).unwrap();
    // A dangling symlink is a marker too: it is there, wherever it points.
    std::os::unix::fs::symlink("gone", &launcher).unwrap();
    assert!(!Snapshot::take_from(&launcher, &game).fresh());
}

#[test]
fn the_default_write_preserves_everything_and_never_overwrites() {
    let (launcher, game) = dirs("init");
    let settings = launcher.join("settings.json");
    std::fs::create_dir_all(&launcher).unwrap();
    // Unknown and unrelated keys survive the patch verbatim.
    std::fs::write(&settings, r#"{"future_thing": {"a": 1}, "custom": "keep me"}"#).unwrap();
    init_in(&settings, Snapshot::take_from(&launcher, &game).fresh());
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(after["custom"], "keep me");
    assert_eq!(after["future_thing"]["a"], 1);
    assert_eq!(after["basic_summaries"], false, "an existing install defaults off");
    assert_eq!(after["basic_summaries_cohort"], "existing-default-off");
    // An existing choice in any form is never overwritten - an opt-out survives even a fresh
    // snapshot, and a concurrent first launch that wrote first wins.
    init_in(&settings, true);
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(after["basic_summaries"], false, "the recorded value was overwritten");
    assert_eq!(after["basic_summaries_cohort"], "existing-default-off");
    // A cohort this build can't parse is left alone too.
    std::fs::write(&settings, r#"{"basic_summaries_cohort": "future-cohort"}"#).unwrap();
    init_in(&settings, true);
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(after["basic_summaries_cohort"], "future-cohort");
    assert!(after.get("basic_summaries").is_none(), "doubt must not write a value");
    // A corrupt file is left byte-for-byte as it is.
    std::fs::write(&settings, b"{not json").unwrap();
    init_in(&settings, true);
    assert_eq!(std::fs::read_to_string(&settings).unwrap(), "{not json");
    // A write that cannot happen (settings.json is a directory) changes nothing and
    // does not panic.
    std::fs::remove_file(&settings).unwrap();
    std::fs::create_dir_all(&settings).unwrap();
    init_in(&settings, true);
    assert!(settings.is_dir());
}

#[test]
fn a_session_reads_only_its_own_event_frames() {
    let mut s = Session::start(Instant::now());
    // Log lines, even about frames, are not events, and are not consumed.
    assert!(!s.on_line("Horizon: first frame after 812 ms"));
    // Nothing counts before the game's hello: measurements are bound to the measured build.
    assert!(s.on_line("HORIZON-SESSION-EVENT {\"e\":\"title\"}"));
    assert!(s.title.is_none(), "title accepted before the hello");
    hello(&mut s, "v1.2.3");
    assert!(s.title.is_none(), "a pre-hello title is not recovered");
    // Malformed and unknown-field frames are consumed but rejected.
    assert!(s.on_line("HORIZON-SESSION-EVENT {not json"));
    assert!(s.on_line("HORIZON-SESSION-EVENT {\"e\":\"title\",\"x\":1}"));
    assert!(s.title.is_none(), "unknown fields accepted");
    assert!(s.on_line(&format!("HORIZON-SESSION-EVENT {}", "x".repeat(500))));
    assert!(s.on_line("HORIZON-SESSION-EVENT {\"e\":\"mystery\"}"));
    // A duplicate hello cannot rebind the build.
    hello(&mut s, "v9.9.9");
    assert_eq!(s.hello.as_deref(), Some("v1.2.3"));
    assert!(s.on_line("HORIZON-SESSION-EVENT {\"e\":\"title\"}"));
    assert!(s.title.is_some());
    assert!(s.on_line("HORIZON-SESSION-EVENT {\"e\":\"world\"}"));
    assert!(s.world.is_some());
    // p95 needs its sample count and domain: too few frames, zero and NaN are absent.
    assert!(s.on_line("HORIZON-SESSION-EVENT {\"e\":\"frames\",\"p95_ms\":16.9,\"n\":12}"));
    assert!(s.p95.is_none());
    assert!(s.on_line("HORIZON-SESSION-EVENT {\"e\":\"frames\",\"p95_ms\":0.0,\"n\":300}"));
    assert!(s.p95.is_none());
    assert!(s.on_line("HORIZON-SESSION-EVENT {\"e\":\"frames\",\"p95_ms\":16.9,\"n\":300}"));
    assert_eq!(s.p95, Some(P95::B16_7));
    assert!(s.on_line("HORIZON-SESSION-EVENT {\"e\":\"frames\",\"p95_ms\":1001.0,\"n\":300}"));
    assert_eq!(s.p95, Some(P95::B16_7), "an earlier valid value is kept");
    // The normal-end marker completes the session.
    assert!(!s.ended);
    assert!(s.on_line("HORIZON-SESSION-EVENT {\"e\":\"end\"}"));
    assert!(s.ended);
}

#[test]
fn consumed_frames_stay_out_of_the_log() {
    let mut none: Option<Session> = None;
    // No session: everything is log.
    assert!(!consume_event_line(&mut none, true, "HORIZON-SESSION-EVENT {\"e\":\"end\"}"));
    let mut some = Some(Session::start(Instant::now()));
    // A protocol frame is consumed; an ordinary line is not; another job's line is not.
    assert!(consume_event_line(&mut some, true, "HORIZON-SESSION-EVENT {not json"));
    assert!(!consume_event_line(&mut some, true, "Horizon: the game is starting"));
    assert!(!consume_event_line(&mut some, false, "HORIZON-SESSION-EVENT {\"e\":\"end\"}"));
}

/// The event/exit harness: exactly one send, only for a normal end with a successful exit.
/// Runs the real production send path (loopback seam, fixture release) when the seam is
/// compiled in; without it the empty allowlist already proves nothing sends.
#[test]
fn a_session_sends_exactly_once_and_only_on_a_clean_end() {
    let _guard = SERIAL.lock().unwrap();
    let Some(release) = basic::test_release() else {
        return; // no seam: the empty allowlist blocks every report (telemetry tests pin it)
    };
    let (launcher, _game) = dirs("harness");
    std::fs::create_dir_all(&launcher).unwrap();
    std::fs::write(
        launcher.join("settings.json"),
        r#"{"basic_summaries": true, "basic_summaries_cohort": "fresh-default-on"}"#,
    )
    .unwrap();
    std::env::set_var("HORIZON_LAUNCHER_HOME", &launcher);

    // A loopback collector receives what the production send posts.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (got_tx, got_rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        use std::io::Read;
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = String::new();
            let _ = stream.read_to_string(&mut buf);
            let _ = got_tx.send(buf);
        }
    });
    std::env::set_var(
        "GOATTECH_BASIC_STATS_TEST_URL",
        format!("http://{addr}/v1/basic-stats"),
    );

    let full = |s: &mut Session| {
        hello(s, release);
        s.on_line("HORIZON-SESSION-EVENT {\"e\":\"title\"}");
        s.on_line("HORIZON-SESSION-EVENT {\"e\":\"world\"}");
        s.on_line("HORIZON-SESSION-EVENT {\"e\":\"frames\",\"p95_ms\":16.9,\"n\":300}");
    };

    // A kill (no end marker), a successful exit: nothing.
    let mut killed = Session::start(Instant::now());
    full(&mut killed);
    assert!(killed.finish(true).is_none(), "a killed session made a report");
    // An error exit after a normal end: nothing.
    let mut errored = Session::start(Instant::now());
    full(&mut errored);
    errored.on_line("HORIZON-SESSION-EVENT {\"e\":\"end\"}");
    assert!(errored.finish(false).is_none(), "an error exit made a report");
    // A normal end with a successful exit: exactly one report, named for the measured build.
    let mut clean = Session::start(Instant::now());
    full(&mut clean);
    clean.on_line("HORIZON-SESSION-EVENT {\"e\":\"end\"}");
    assert!(clean.finish(true).is_some());
    clean.session_ended(true);
    let body = got_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the clean session's report never arrived");
    assert!(body.contains("\"release\":\"v0.0.0-test\""), "{body}");
    assert!(body.contains("ttt_ms") && body.contains("ttw_ms") && body.contains("p95_ms"), "{body}");

    std::env::remove_var("GOATTECH_BASIC_STATS_TEST_URL");
    std::env::remove_var("HORIZON_LAUNCHER_HOME");
}

#[test]
fn off_means_no_report_ever_and_a_drop_cannot_be_undone() {
    let _guard = SERIAL.lock().unwrap();
    let (launcher, _game) = dirs("off");
    std::fs::create_dir_all(&launcher).unwrap();
    // No setting at all: off.
    std::env::set_var("HORIZON_LAUNCHER_HOME", &launcher);
    assert!(!enabled());
    let mut s = Session::start(Instant::now());
    hello(&mut s, "v1.2.3");
    s.on_line("HORIZON-SESSION-EVENT {\"e\":\"title\"}");
    s.on_line("HORIZON-SESSION-EVENT {\"e\":\"end\"}");
    assert!(s.finish(true).is_none(), "off assembled a report");
    // A canceled sample is the dropped Session itself; there is nothing to resurrect when
    // the switch comes back on (the consume in session_ended is the only send path).
    drop(s);
    std::fs::write(launcher.join("settings.json"), r#"{"basic_summaries": true}"#).unwrap();
    assert!(enabled());
    let fresh = Session::start(Instant::now());
    assert!(fresh.finish(true).is_none(), "a new session carried old measurements");
    std::env::remove_var("HORIZON_LAUNCHER_HOME");
}
