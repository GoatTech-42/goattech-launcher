use super::*;
use std::path::PathBuf;

fn dirs(name: &str) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!("horizon-basic-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    (base.join("launcher"), base.join("game"))
}

/// Environment-reading checks (`enabled`, a real send) run one at a time under this lock,
/// each with its own HORIZON_LAUNCHER_HOME, restoring the variable after.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn hello(s: &mut Session, release: &str) {
    let v = serde_json::to_string(release).unwrap();
    s.on_line(&format!(
        "HORIZON-SESSION-EVENT {{\"e\":\"hello\",\"v\":{v}}}"
    ));
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
    // A dangling symlink is a marker too: it is there, wherever it points. (Unix link
    // semantics; the other marker cases above run on every platform.)
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("gone", &launcher).unwrap();
        assert!(!Snapshot::take_from(&launcher, &game).fresh());
        std::fs::remove_file(&launcher).unwrap();
    }
}

#[test]
fn the_default_write_preserves_everything_and_never_overwrites() {
    let (launcher, game) = dirs("init");
    let settings = launcher.join("settings.json");
    std::fs::create_dir_all(&launcher).unwrap();
    // Unknown and unrelated keys survive the patch verbatim.
    std::fs::write(
        &settings,
        r#"{"future_thing": {"a": 1}, "custom": "keep me"}"#,
    )
    .unwrap();
    init_in(&settings, Snapshot::take_from(&launcher, &game).fresh());
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(after["custom"], "keep me");
    assert_eq!(after["future_thing"]["a"], 1);
    assert_eq!(
        after["basic_summaries"], false,
        "an existing install defaults off"
    );
    assert_eq!(after["basic_summaries_cohort"], "existing-default-off");
    // An existing choice in any form is never overwritten - an opt-out survives even a fresh
    // snapshot, and a concurrent first launch that wrote first wins.
    init_in(&settings, true);
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(
        after["basic_summaries"], false,
        "the recorded value was overwritten"
    );
    assert_eq!(after["basic_summaries_cohort"], "existing-default-off");
    // A cohort this build can't parse is left alone too.
    std::fs::write(&settings, r#"{"basic_summaries_cohort": "future-cohort"}"#).unwrap();
    init_in(&settings, true);
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(after["basic_summaries_cohort"], "future-cohort");
    assert!(
        after.get("basic_summaries").is_none(),
        "doubt must not write a value"
    );
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
    std::fs::remove_dir_all(&settings).unwrap();
    // An unreadable (non-UTF-8) existing file is doubt: left byte-for-byte, nothing written.
    std::fs::write(&settings, b"\xff\xfe\x00not settings").unwrap();
    init_in(&settings, true);
    assert_eq!(
        std::fs::read(&settings).unwrap(),
        b"\xff\xfe\x00not settings"
    );
    std::fs::remove_file(&settings).unwrap();
    // A held OS write lock (another writer mid-save) skips the patch entirely; once it is
    // released (drop, or the kernel after a crash) the write lands.
    let held = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(launcher.join("settings.lock"))
        .unwrap();
    held.lock().unwrap();
    init_in(&settings, true);
    assert!(!settings.exists(), "a held lock must not be overwritten");
    drop(held);
    init_in(&settings, true);
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(after["basic_summaries"], true);
    assert_eq!(after["basic_summaries_cohort"], "fresh-default-on");
}

/// The stale-GUI case: window A holds an old Settings; another window records an explicit
/// opt-out; A saves an unrelated change. The recorded choice must survive - only an explicit
/// basic-choice save may change it.
#[test]
fn an_unrelated_save_preserves_the_recorded_basic_choice() {
    let _guard = SERIAL.lock().unwrap();
    let (launcher, _game) = dirs("stale-save");
    std::fs::create_dir_all(&launcher).unwrap();
    std::fs::write(
        launcher.join("settings.json"),
        r#"{"basic_summaries": true, "basic_summaries_cohort": "fresh-default-on"}"#,
    )
    .unwrap();
    std::env::set_var("HORIZON_LAUNCHER_HOME", &launcher);
    let mut a = crate::backend::Settings::load();
    assert_eq!(a.basic_summaries, Some(true));
    // Another window turns it off, explicitly.
    let mut b = crate::backend::Settings::load();
    b.basic_summaries = Some(false);
    b.save_basic_choice().unwrap();
    assert_eq!(
        crate::backend::Settings::load().basic_summaries,
        Some(false)
    );
    // A saves an unrelated change from its stale snapshot: the recorded false survives.
    a.skip_opening = true;
    a.save().unwrap();
    let after = crate::backend::Settings::load();
    assert_eq!(
        after.basic_summaries,
        Some(false),
        "a stale save resurrected the opt-in"
    );
    assert!(after.skip_opening, "the unrelated change was lost");
    // But when A's user actually flips the basic switch, that explicit choice wins.
    a.basic_summaries = Some(true);
    a.save_basic_choice().unwrap();
    assert_eq!(crate::backend::Settings::load().basic_summaries, Some(true));
    std::env::remove_var("HORIZON_LAUNCHER_HOME");
}

/// Fail closed on uncertain bytes: an unrelated save that cannot read the current file -
/// corrupt, invalid UTF-8, or not a settings object - writes NOTHING, because the recorded
/// choice is unknown and a stale snapshot must never replace it.
#[test]
fn an_unrelated_save_refuses_uncertain_existing_bytes() {
    let _guard = SERIAL.lock().unwrap();
    let (launcher, _game) = dirs("uncertain");
    std::fs::create_dir_all(&launcher).unwrap();
    std::env::set_var("HORIZON_LAUNCHER_HOME", &launcher);
    let settings = launcher.join("settings.json");
    // A recorded opt-out, then a stale snapshot from another window still carrying true.
    let mut b = crate::backend::Settings::load();
    b.basic_summaries = Some(false);
    b.save_basic_choice().unwrap();
    let mut stale = crate::backend::Settings {
        basic_summaries: Some(true),
        ..crate::backend::Settings::load()
    };
    for bad in [
        b"\xffnot-json".as_slice(),
        b"[1,2]".as_slice(),
        b"\"str\"".as_slice(),
    ] {
        std::fs::write(&settings, bad).unwrap();
        assert!(
            stale.save().is_err(),
            "an unrelated save wrote over bytes it could not read"
        );
        assert_eq!(
            std::fs::read(&settings).unwrap(),
            bad,
            "uncertain bytes were replaced"
        );
    }
    // A file that parses to no setting is off - never the stale snapshot's true.
    assert_ne!(crate::backend::Settings::load().basic_summaries, Some(true));
    // And the explicit-choice path still works afterwards.
    stale.save_basic_choice().unwrap();
    assert_eq!(crate::backend::Settings::load().basic_summaries, Some(true));
    std::env::remove_var("HORIZON_LAUNCHER_HOME");
}

/// Proven-absent file, explicit policy: an unrelated save records NO basic choice rather
/// than inventing one from a stale snapshot's in-memory value.
#[test]
fn an_unrelated_save_records_no_choice_when_none_is_on_disk() {
    let _guard = SERIAL.lock().unwrap();
    let (launcher, _game) = dirs("gone");
    std::fs::create_dir_all(&launcher).unwrap();
    std::env::set_var("HORIZON_LAUNCHER_HOME", &launcher);
    let mut stale = crate::backend::Settings::load();
    stale.basic_summaries = Some(true); // a stale in-memory opt-in
    stale.save().unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(launcher.join("settings.json")).unwrap())
            .unwrap();
    assert!(
        v.get("basic_summaries").is_none(),
        "an unrelated save invented a choice: {v}"
    );
    assert!(v.get("basic_summaries_cohort").is_none(), "{v}");
    std::env::remove_var("HORIZON_LAUNCHER_HOME");
}

/// The held-lock off, end to end: the click returns at once (no 1s lock wait on the UI),
/// the veto makes every collection gate off NOW while disk still says on, and the queued
/// save lands once the lock is free. A saved ON - the newest choice - lifts the veto.
#[test]
fn an_off_choice_applies_now_and_lands_after_a_held_lock() {
    let _guard = SERIAL.lock().unwrap();
    let (launcher, _game) = dirs("heldoff");
    std::fs::create_dir_all(&launcher).unwrap();
    std::env::set_var("HORIZON_LAUNCHER_HOME", &launcher);
    let settings = launcher.join("settings.json");
    let mut st = crate::backend::Settings::load();
    st.basic_summaries = Some(true);
    st.save_basic_choice().unwrap();
    assert!(enabled());
    // Hold the OS write lock, as another stuck writer would.
    let held = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(launcher.join("settings.lock"))
        .unwrap();
    held.lock().unwrap();
    // The click: veto now (inside the choice recording), persist off the thread.
    let started = Instant::now();
    let mut off = crate::backend::Settings::load();
    off.basic_summaries = Some(false);
    record_choice_async(off);
    assert!(
        started.elapsed() < std::time::Duration::from_millis(500),
        "the click blocked on the stuck lock"
    );
    assert!(
        !enabled(),
        "the veto did not apply while the save was stuck"
    );
    // Free the lock well inside the worker's bounded wait: the queued save lands.
    drop(held);
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        if v.get("basic_summaries") == Some(&serde_json::Value::Bool(false)) {
            break;
        }
        assert!(Instant::now() < deadline, "the queued off never landed");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!enabled(), "a recorded off somehow reads as on");
    assert!(choice_save_failed().is_none(), "{:?}", choice_save_failed());
    // Click order is the write order: a quick off-then-on ends on, and that saved ON -
    // the newest choice - lifts the veto, so this process agrees with the next launch.
    let mut off2 = crate::backend::Settings::load();
    off2.basic_summaries = Some(false);
    record_choice_async(off2);
    let mut on2 = crate::backend::Settings::load();
    on2.basic_summaries = Some(true);
    record_choice_async(on2);
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    while !enabled() {
        assert!(
            Instant::now() < deadline,
            "the newest saved on never lifted the veto"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(crate::backend::Settings::load().basic_summaries, Some(true));
    assert!(choice_save_failed().is_none(), "{:?}", choice_save_failed());
    std::env::remove_var("HORIZON_LAUNCHER_HOME");
}

/// Publication consistency, QA's v6 probe shape: an observer polling the row's failure
/// view must NEVER see a failed status without its error - the completion publishes both
/// as one unit, so the 80ms drain's repaint can only ever land on a complete state.
#[test]
fn a_failed_completion_is_published_as_one_consistent_unit() {
    let _guard = SERIAL.lock().unwrap();
    use std::sync::atomic::{AtomicBool as AB, Ordering as O};
    let stop = std::sync::Arc::new(AB::new(false));
    let stop2 = stop.clone();
    let observer = std::thread::spawn(move || {
        while !stop2.load(O::Relaxed) {
            if let Some((_, error)) = choice_save_failed() {
                assert!(
                    !error.is_empty(),
                    "a failed save was visible without its error"
                );
            }
        }
    });
    for _ in 0..50 {
        let g = CHOICE_GEN.fetch_add(1, Ordering::AcqRel) + 1;
        CHOICE_STATUS.store(pack_status(g, 1), Ordering::Release);
        complete_choice(
            g,
            false,
            Err(std::io::Error::new(std::io::ErrorKind::Other, "disk full")),
        );
    }
    stop.store(true, O::Relaxed);
    observer.join().unwrap();
    // Leave a settled newest state behind for the other tests.
    let g = CHOICE_GEN.fetch_add(1, Ordering::AcqRel) + 1;
    CHOICE_STATUS.store(pack_status(g, 1), Ordering::Release);
    complete_choice(g, true, Ok(()));
    assert!(choice_save_failed().is_none());
}

/// QA's failedOFF-then-failedON sequence: disk says ON throughout (every save fails on a
/// held lock), the newest failure is the ON, and the veto still holds - the row must say
/// "off for this session", never "off for this install", because the persisted record is
/// still ON and the next launch would collect.
#[test]
fn a_failed_on_after_a_failed_off_leaves_the_row_truthful() {
    let _guard = SERIAL.lock().unwrap();
    let (launcher, _game) = dirs("rowtruth");
    std::fs::create_dir_all(&launcher).unwrap();
    std::env::set_var("HORIZON_LAUNCHER_HOME", &launcher);
    let mut st = crate::backend::Settings::load();
    st.basic_summaries = Some(true);
    st.save_basic_choice().unwrap();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(launcher.join("settings.lock"))
        .unwrap();
    held.lock().unwrap();
    // OFF: veto at once; the save fails on the held lock.
    let mut off = crate::backend::Settings::load();
    off.basic_summaries = Some(false);
    record_choice_async(off);
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    while choice_save_failed().is_none() {
        assert!(
            Instant::now() < deadline,
            "the stuck off was never reported"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    // ON retry: also fails. The newest failure is now the ON.
    let mut on = crate::backend::Settings::load();
    on.basic_summaries = Some(true);
    record_choice_async(on);
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Some((was_on, _)) = choice_save_failed() {
            if was_on {
                break;
            }
        }
        assert!(Instant::now() < deadline, "the stuck on was never reported");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    // Disk untouched by both failures; collection is off HERE; the row must not claim
    // "off for this install".
    assert_eq!(crate::backend::Settings::load().basic_summaries, Some(true));
    assert!(!enabled());
    assert_eq!(
        row_state(Some(true)),
        RowState::OffThisSession,
        "the row claimed an install state disk does not have"
    );
    // The lock frees, a fresh ON saves, and only then may the row say "on for this
    // install".
    drop(held);
    let mut on = crate::backend::Settings::load();
    on.basic_summaries = Some(true);
    record_choice_async(on);
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    while !enabled() {
        assert!(Instant::now() < deadline, "the retry never landed");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(row_state(Some(true)), RowState::OnForInstall);
    std::env::remove_var("HORIZON_LAUNCHER_HOME");
}

/// QA's v5 probe, in-repo and deterministic: a stale saved ON - chosen BEFORE a newer
/// OFF - completes after the OFF was recorded. Its completion must neither clear the
/// newer OFF's veto nor overwrite the newer choice's status. Driven straight through
/// `complete_choice`, so no timing luck is involved.
#[test]
fn a_stale_saved_on_never_clears_a_newer_off() {
    let _guard = SERIAL.lock().unwrap();
    // An ON choice's save is in flight...
    let g_on = CHOICE_GEN.fetch_add(1, Ordering::AcqRel) + 1;
    CHOICE_STATUS.store(pack_status(g_on, 1), Ordering::Release);
    // ...and the player turns it OFF before that save completes: veto now, status now.
    let g_off = CHOICE_GEN.fetch_add(1, Ordering::AcqRel) + 1;
    VETO_GEN.store(g_off, Ordering::Release);
    CHOICE_STATUS.store(pack_status(g_off, 1), Ordering::Release);
    // The stale ON completion lands: the newer OFF stands.
    complete_choice(g_on, true, Ok(()));
    assert_eq!(
        VETO_GEN.load(Ordering::Acquire),
        g_off,
        "a stale saved on cleared a newer off's veto"
    );
    assert_eq!(
        CHOICE_STATUS.load(Ordering::Acquire),
        pack_status(g_off, 1),
        "a stale completion overwrote the newer choice's status"
    );
    // A stale FAILURE is just as toothless.
    complete_choice(
        g_on,
        false,
        Err(std::io::Error::new(std::io::ErrorKind::Other, "stale")),
    );
    assert_eq!(CHOICE_STATUS.load(Ordering::Acquire), pack_status(g_off, 1));
    assert!(
        choice_save_failed().is_none(),
        "a stale failure surfaced as the newest"
    );
    // The OFF's own completion settles its own word; the veto it set stays.
    complete_choice(g_off, false, Ok(()));
    assert_eq!(CHOICE_STATUS.load(Ordering::Acquire), pack_status(g_off, 0));
    assert_eq!(
        VETO_GEN.load(Ordering::Acquire),
        g_off,
        "an off's save lifted its own veto"
    );
    // And a NEWER saved ON does lift it - the disk now carries the newest choice.
    let g_on2 = CHOICE_GEN.fetch_add(1, Ordering::AcqRel) + 1;
    CHOICE_STATUS.store(pack_status(g_on2, 1), Ordering::Release);
    complete_choice(g_on2, true, Ok(()));
    assert_eq!(
        VETO_GEN.load(Ordering::Acquire),
        0,
        "the newest saved on kept the veto"
    );
    assert_eq!(CHOICE_STATUS.load(Ordering::Acquire), pack_status(g_on2, 0));
}

/// A failed OFF save is reported for the row and still stays off in this process: the
/// lock outlasts the worker's bounded wait, so the save fails - the veto holds (disk
/// still says on, collection is off here), and the failure is surfaced. Retrying after
/// the lock frees lands it.
#[test]
fn a_failed_off_save_is_surfaced_and_stays_off() {
    let _guard = SERIAL.lock().unwrap();
    let (launcher, _game) = dirs("failedoff");
    std::fs::create_dir_all(&launcher).unwrap();
    std::env::set_var("HORIZON_LAUNCHER_HOME", &launcher);
    let mut st = crate::backend::Settings::load();
    st.basic_summaries = Some(true);
    st.save_basic_choice().unwrap();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(launcher.join("settings.lock"))
        .unwrap();
    held.lock().unwrap();
    let mut off = crate::backend::Settings::load();
    off.basic_summaries = Some(false);
    record_choice_async(off);
    // The worker's wait is bounded (~1s): outlast it, so the save fails for real.
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    while choice_save_failed().is_none() {
        assert!(
            Instant::now() < deadline,
            "the stuck save was never reported"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let (was_on, error) = choice_save_failed().unwrap();
    assert!(!was_on, "the failed choice was the off");
    assert!(!error.is_empty());
    assert!(
        !enabled(),
        "a failed off save left collection on in this process"
    );
    assert_eq!(
        crate::backend::Settings::load().basic_summaries,
        Some(true),
        "disk changed?"
    );
    drop(held);
    // The retry lands; a saved ON then lifts the veto, leaving a clean state.
    let mut off = crate::backend::Settings::load();
    off.basic_summaries = Some(false);
    record_choice_async(off);
    let mut on = crate::backend::Settings::load();
    on.basic_summaries = Some(true);
    record_choice_async(on);
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    while !enabled() {
        assert!(Instant::now() < deadline, "the retry never landed");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(choice_save_failed().is_none(), "{:?}", choice_save_failed());
    std::env::remove_var("HORIZON_LAUNCHER_HOME");
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
    // No session - the switch went off mid-game and dropped it: the game's queued frames are
    // still reserved protocol, never log.
    assert!(consume_event_line(
        &mut none,
        true,
        "HORIZON-SESSION-EVENT {\"e\":\"end\"}"
    ));
    assert!(consume_event_line(
        &mut none,
        true,
        "HORIZON-SESSION-EVENT {not json"
    ));
    assert!(!consume_event_line(
        &mut none,
        true,
        "Horizon: the game is starting"
    ));
    let mut some = Some(Session::start(Instant::now()));
    // A protocol frame is consumed; an ordinary line is not; another job's line is not.
    assert!(consume_event_line(
        &mut some,
        true,
        "HORIZON-SESSION-EVENT {not json"
    ));
    assert!(!consume_event_line(
        &mut some,
        true,
        "Horizon: the game is starting"
    ));
    assert!(!consume_event_line(
        &mut some,
        false,
        "HORIZON-SESSION-EVENT {\"e\":\"end\"}"
    ));
    // Dropping the session mid-stream changes nothing for the log.
    some = None;
    assert!(consume_event_line(
        &mut some,
        true,
        "HORIZON-SESSION-EVENT {\"e\":\"frames\",\"p95_ms\":16.9,\"n\":300}"
    ));
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
    assert!(
        killed.finish(true).is_none(),
        "a killed session made a report"
    );
    // An error exit after a normal end: nothing.
    let mut errored = Session::start(Instant::now());
    full(&mut errored);
    errored.on_line("HORIZON-SESSION-EVENT {\"e\":\"end\"}");
    assert!(
        errored.finish(false).is_none(),
        "an error exit made a report"
    );
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
    assert!(
        body.contains("ttt_ms") && body.contains("ttw_ms") && body.contains("p95_ms"),
        "{body}"
    );

    std::env::remove_var("GOATTECH_BASIC_STATS_TEST_URL");
    std::env::remove_var("HORIZON_LAUNCHER_HOME");
}

/// The recorded choice never loses to a concurrent first-run default: the shared write
/// lock serializes typed saves and the default patch. A writer thread hammers an explicit
/// opt-out through the real Settings::save_basic_choice while the init runs; the opt-out
/// must survive every round.
#[test]
fn a_concurrent_choice_survives_the_default_patch() {
    let _guard = SERIAL.lock().unwrap();
    let (launcher, _game) = dirs("race");
    std::fs::create_dir_all(&launcher).unwrap();
    std::env::set_var("HORIZON_LAUNCHER_HOME", &launcher);
    let settings = launcher.join("settings.json");
    use std::sync::atomic::{AtomicBool, Ordering};
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let opted_out = std::sync::Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let opted_out2 = opted_out.clone();
    let writer = std::thread::spawn(move || {
        while !stop2.load(Ordering::Relaxed) {
            let mut st = crate::backend::Settings::load();
            st.basic_summaries = Some(false);
            // A lost lock race skips that round instead of failing it; the assertion
            // below only makes sense once this writer actually landed its opt-out.
            if st.save_basic_choice().is_err() {
                continue;
            }
            opted_out2.store(true, Ordering::Relaxed);
            // The opt-out just written must still be there: no stale init true over it.
            assert_ne!(
                crate::backend::Settings::load().basic_summaries,
                Some(true),
                "a recorded opt-out was overwritten by the default"
            );
        }
    });
    for _ in 0..50 {
        init_in(&settings, true);
        // Check the flag FIRST, then read: once the opt-out has landed the file can only
        // ever read false again, while a file read BEFORE the flag flips may still be the
        // default patch - that ordering is a test artifact, not an init overwrite.
        if opted_out.load(Ordering::Relaxed) {
            let v: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
            if let Some(b) = v.get("basic_summaries") {
                assert_eq!(
                    b,
                    &serde_json::Value::Bool(false),
                    "init overwrote the choice"
                );
            }
        }
    }
    stop.store(true, Ordering::Relaxed);
    writer.join().unwrap();
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
    std::fs::write(
        launcher.join("settings.json"),
        r#"{"basic_summaries": true}"#,
    )
    .unwrap();
    assert!(enabled());
    let fresh = Session::start(Instant::now());
    assert!(
        fresh.finish(true).is_none(),
        "a new session carried old measurements"
    );
    std::env::remove_var("HORIZON_LAUNCHER_HOME");
}
