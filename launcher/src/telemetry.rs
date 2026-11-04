//! Opt-in data collection from the launcher (`horizon_telemetry`): the player's answer, what
//! would be sent, deleting it, bug reports and benchmark uploads, and sending the outbox when
//! the launcher starts and closes. The window, the CLI and the MCP server all go through here.
//! Also the live count's side here: what the "I'm playing" ping holds, in words, and reading
//! how many play now (`horizon_telemetry::live`).
use std::path::{Path, PathBuf};

use horizon_telemetry::payload::{Bug, Crash, Perf, System, Timings};
use horizon_telemetry::scrub::Scrubber;
use horizon_telemetry::upload::{self, Flush};
use horizon_telemetry::{outbox, Choices, Consent, Item, Kind, Payload, Source};

use crate::{account::Accounts, backend};

/// Each switch: its name, and in plain words exactly what it sends.
pub const SWITCHES: [(Kind, &str, &str); 4] = [
    (
        Kind::Perf,
        "Performance statistics",
        "After each session: frame-time percentiles and a 14-bucket histogram, tick times, peak memory and startup times, with your machine (OS, CPU and its cores and clocks, memory size, GPU, its memory and driver, drive type, screen size and refresh rate, laptop or desktop, on battery or not) and the settings that change performance.",
    ),
    (
        Kind::Crash,
        "Crash reports",
        "When the game crashes: the error, where in Horizon's code it happened, your graphics adapter and settings. No log, no world or server, no user name in paths.",
    ),
    (
        Kind::Bug,
        "Bug reports",
        "Only the reports you write and send yourself, with the game's log if you add it after reading it.",
    ),
    (
        Kind::Benchmark,
        "Benchmark uploads",
        "The results of benchmarks you run, for the community results on playhorizon.dev.",
    ),
];

/// Never sent, whatever the switches say.
pub const NEVER: &str = "Never sent: your account name or UUID, world names, server addresses, chat, or your IP address. Each report is dated to the day, under a random install ID you can delete.";

/// The anonymous "I'm playing" ping. In this build it is ALWAYS ON: the game sends it to
/// GoatTech's own collector (`horizon_telemetry::LIVE_ENDPOINT`) whatever the switches say;
/// `HORIZON_TELEMETRY_URL=off` turns it off. Shown on the first-run question, in Settings,
/// and once on the Play page.
pub const PING_NAME: &str = "\"I'm playing\" ping";

/// Shown in PING's place when `HORIZON_TELEMETRY_URL=off` (or `none`, `0`) turns the ping off,
/// so the text never claims "always on" beside the chip's "Off by default". The full payload and
/// retention disclosure stays in PING for the default on state.
pub const PING_OFF: &str = "The anonymous player-count ping is off in this configuration: no ping is sent and no live player count is shown. The optional switches work the same either way.";

/// The ping's description: PING while the ping is on (the default), PING_OFF when it is off.
pub fn ping_text() -> &'static str {
    if horizon_telemetry::live_endpoint().is_some() {
        PING
    } else {
        PING_OFF
    }
}
pub const PING: &str = "The game sends an anonymous \"I'm playing\" ping once a minute while you play, so the launcher can show how many people play right now. The ping is always on and goes to GoatTech's own counter (telemetry.goattech.us), which stores only each session's anonymous row while you play and removes it about an hour after you stop; cleanup can take longer if the service hits its daily limit. The ping holds: a random number made when the game starts and forgotten when it closes, the version, your OS (Linux, macOS or Windows), and whether you have a world open to LAN or friends. It doesn't hold: your account, your install ID, world or server names, or anything you do in the game, and the server doesn't keep your IP address. It is sent whatever the switches above say.";

/// How many play now (cached a few seconds), if GoatTech's collector says. The count and the ping share
/// [`horizon_telemetry::live_endpoint`]: the collector by default, off with
/// `HORIZON_TELEMETRY_URL=off`. Never the upload endpoint.
pub fn playing_now() -> Option<u64> {
    let endpoint = count_endpoint()?;
    horizon_telemetry::live::fetch(&endpoint)
        .ok()
        .map(|count| count.playing)
}

/// Where the live count is read: the same place the ping goes.
fn count_endpoint() -> Option<String> {
    horizon_telemetry::live_endpoint()
}

#[cfg(test)]
mod live_count_tests {
    #[test]
    fn the_count_reads_where_the_ping_goes() {
        // Binds the consumer, not only the helper: with no override the count asks
        // GoatTech's collector; `off` turns it off.
        if std::env::var("HORIZON_TELEMETRY_URL").is_err() {
            assert_eq!(
                super::count_endpoint().as_deref(),
                Some(horizon_telemetry::LIVE_ENDPOINT)
            );
        }
    }
}

/// The page with the count over time and its peaks, on the website `links.json` names; None when
/// it names none.
pub fn stats_url(website: &str) -> Option<String> {
    let site = website.trim().trim_end_matches('/');
    site.starts_with("https://")
        .then(|| format!("{site}/stats"))
}

/// Sends [`playing_now`] to the window as `Msg::Playing` now and every minute, on its own
/// thread. Nothing at all when sending is turned off.
pub fn watch_playing(tx: std::sync::mpsc::Sender<backend::Msg>) {
    if horizon_telemetry::live_endpoint().is_none() {
        return;
    }
    std::thread::spawn(move || {
        while tx.send(backend::Msg::Playing(playing_now())).is_ok() {
            std::thread::sleep(horizon_telemetry::live::INTERVAL);
        }
    });
}

/// The official server's address from links.json once read (`set_official_server`; empty: none),
/// the built-in one until then.
static OFFICIAL_SERVER: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

/// The official server [`watch_official`] asks from now on: `Links::official_server`.
pub fn set_official_server(address: &str) {
    *OFFICIAL_SERVER.write().unwrap_or_else(|e| e.into_inner()) = Some(address.trim().to_owned());
}

fn official_server() -> String {
    OFFICIAL_SERVER
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_else(|| crate::server_list::OFFICIAL_ADDRESS.to_owned())
}

/// How many play on the official server now: its server-list status ping, as the game's
/// Multiplayer list sends it (through the address's SRV record), with a 3-second timeout. None
/// when it doesn't answer or there is no official server.
pub fn official_players(address: &str) -> Option<u64> {
    let (host, port) = horizon_telemetry::server_status::split_address(address)?;
    let (host, port) = horizon_protocol::server_redirect(&host, port);
    horizon_telemetry::server_status::players_at(
        address,
        (&host, port),
        std::time::Duration::from_secs(3),
    )
    .ok()
    .map(|players| players.online)
}

/// Sends [`official_players`] to the window as `Msg::OfficialPlayers` now and every minute, on
/// its own thread. Nothing when `HORIZON_TELEMETRY_URL=off` (tests and automated runs) turns the
/// live count off too.
pub fn watch_official(tx: std::sync::mpsc::Sender<backend::Msg>) {
    if horizon_telemetry::endpoint().is_none() {
        return;
    }
    std::thread::spawn(move || {
        while tx
            .send(backend::Msg::OfficialPlayers(official_players(
                &official_server(),
            )))
            .is_ok()
        {
            std::thread::sleep(horizon_telemetry::live::INTERVAL);
        }
    });
}

pub fn home() -> PathBuf {
    backend::game_home()
}

pub fn consent() -> Consent {
    Consent::load(&home())
}

/// Records the player's answer. Waiting items of kinds now off are deleted at once.
pub fn answer(choices: Choices) -> Result<Consent, String> {
    let home = home();
    let mut consent = Consent::load(&home);
    consent.answer(choices);
    consent.save(&home).map_err(|e| e.to_string())?;
    outbox::retain_allowed(&home);
    Ok(consent)
}

/// The exact JSON the next upload would send, under a line saying what it is. With nothing
/// waiting, one example of each kind, made on this machine.
pub fn preview() -> (String, String) {
    let home = home();
    let waiting = outbox::retain_allowed(&home).len();
    if waiting > 0 {
        let what = format!(
            "{waiting} item{} waiting to be sent, exactly as they will leave (compressed):",
            if waiting == 1 { "" } else { "s" }
        );
        return (what, upload::preview(&home));
    }
    let system = with_example_gpu(System::detect_in(Some(&home)));
    let examples = vec![
        Item::new(
            Source::Game,
            "v2026.10.15",
            Payload::Perf(Perf {
                system: system.clone(),
                seconds: 1800,
                frames: Timings::new().summary(),
                ..Perf::default()
            }),
        ),
        Item::new(
            Source::Game,
            "v2026.10.15",
            Payload::Crash(Crash {
                system: system.clone(),
                error: "panicked at crates/horizon-client/src/game.rs:120:9: …".into(),
                stack: "horizon_client::game::Game::tick (crates/horizon-client/src/game.rs:120:9)"
                    .into(),
                thread: "Render thread".into(),
                details: vec![("Graphics".into(), "Apple M4 Pro (Metal)".into())],
            }),
        ),
        Item::new(
            Source::Launcher,
            backend::VERSION,
            Payload::Bug(Bug {
                system,
                text: "What you write.".into(),
                log: None,
            }),
        ),
    ];
    let json = serde_json::to_string_pretty(&serde_json::json!({
        "schema": horizon_telemetry::SCHEMA,
        "install_id": consent().install_id,
        "items": examples,
    }))
    .unwrap_or_default();
    (
        "Nothing is waiting. One example of each kind, from this machine; the game fills in the GPU and screen it uses where these say example (a benchmark upload is the report itself, see docs/benchmark-report.md):".into(),
        json,
    )
}

/// What only the game knows (the GPU it draws with, the screen its window is on), as an
/// example: the launcher draws with neither.
pub fn with_example_gpu(mut system: System) -> System {
    system.gpu = "example: the GPU the game draws with".into();
    system.gpu_vendor = "nvidia".into();
    system.gpu_kind = "discrete".into();
    system.vram_mb = 8192;
    system.driver = "example: 570.86".into();
    system.backend = "Vulkan".into();
    system.display = Some(horizon_telemetry::payload::Display {
        width: 1920,
        height: 1080,
        refresh_hz: 60,
        scale: 1.0,
    });
    system
}

/// "Delete my data": the server deletes everything sent under this install's ID, the waiting
/// items go, and the install gets a new ID. If the server can't be reached the ID is kept, so
/// the deletion can be asked again. With no server configured (the default, or `off`) nothing
/// remote can be asked: the waiting items still go, but the install ID is KEPT so a later ask
/// with the server configured (`HORIZON_TELEMETRY_URL`) can still delete what was sent before.
pub fn delete_my_data() -> Result<String, String> {
    let home = home();
    let mut consent = Consent::load(&home);
    outbox::clear(&home).map_err(|e| e.to_string())?;
    if consent.install_id.is_empty() {
        // An empty local ID proves nothing about the past (metadata could have been reset):
        // state only what is true here now.
        return Ok("This install has no telemetry ID recorded, and nothing is waiting to be sent.".into());
    }
    let Some(url) = horizon_telemetry::endpoint() else {
        return Ok(concat!(
            "No telemetry server is configured, so no server was asked. The items waiting here ",
            "are deleted. Your install ID is kept: if anything was sent before, set ",
            "HORIZON_TELEMETRY_URL to that server and ask again."
        ).into());
    };
    upload::delete_remote(&url, &consent.install_id)
        .map_err(|e| format!("The server could not be reached ({e}). Try again later."))?;
    consent.reset_install_id();
    consent.save(&home).map_err(|e| e.to_string())?;
    Ok("The server deleted everything sent from here. Nothing is waiting, and this install has a new ID.".into())
}

/// Hides the player's account names and UUIDs and their worlds' folder names, besides what
/// `Scrubber` finds on its own.
pub fn scrubber() -> Scrubber {
    let mut s = Scrubber::new();
    for account in Accounts::load().accounts {
        s = s.term(account.name).term(account.uuid);
    }
    let saves = std::fs::read_dir(home().join("saves"))
        .into_iter()
        .flatten()
        .flatten();
    for world in saves {
        s = s.term(world.file_name().to_string_lossy());
    }
    s
}

/// The end of the game's log as a bug report would carry it: scrubbed, without chat.
pub fn log_for_report() -> Option<String> {
    let (_, text) = backend::read_logs(300, false).ok()?;
    let kept: Vec<&str> = text.lines().filter(|l| !l.contains("[CHAT]")).collect();
    let mut log = scrubber().scrub(&kept.join("\n"));
    // Within the item's size, the newest lines first to stay.
    while log.len() > 64 * 1024 {
        let cut = log.find('\n').map_or(log.len(), |i| i + 1);
        log.drain(..cut);
    }
    Some(log)
}

/// A bug report, ready to be read before it is kept.
pub fn bug_report(text: &str, with_log: bool) -> Result<Item, String> {
    let item = Item::new(
        Source::Launcher,
        backend::VERSION,
        Payload::Bug(Bug {
            system: System::detect_in(Some(&home())),
            text: scrubber().scrub(text.trim()),
            log: with_log.then(log_for_report).flatten(),
        }),
    );
    item.validate()?;
    Ok(item)
}

/// An error unless the player allows `kind`.
pub fn ensure_allowed(kind: Kind) -> Result<(), String> {
    if consent().allows(kind) {
        return Ok(());
    }
    Err(format!(
        "{} are off: turn them on in Settings → Data collection first.",
        label(kind)
    ))
}

/// Keeps `item` and sends the outbox; says what happened.
pub fn submit(item: &Item) -> Result<String, String> {
    ensure_allowed(item.kind())?;
    if !outbox::push(&home(), item).map_err(|e| e.to_string())? {
        return Err("Today's limit for these is reached; try again tomorrow.".into());
    }
    Ok(match send() {
        Flush::Sent { .. } => "Sent. Thank you.".into(),
        Flush::Nothing if horizon_telemetry::endpoint().is_none() => {
            "Kept; no upload server is configured, so uploads are not sent.".into()
        }
        _ => "Kept; it will be sent the next time the launcher starts.".into(),
    })
}

/// A benchmark report file (`docs/benchmark-report.md`), scrubbed, as an upload.
pub fn benchmark_report(path: &Path) -> Result<Item, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let report: serde_json::Value = serde_json::from_str(&scrubber().scrub(&text))
        .map_err(|e| format!("{}: not a JSON report ({e})", path.display()))?;
    let item = Item::new(
        Source::Launcher,
        backend::VERSION,
        Payload::Benchmark(report),
    );
    item.validate()?;
    Ok(item)
}

pub fn label(kind: Kind) -> &'static str {
    SWITCHES.iter().find(|s| s.0 == kind).map_or("", |s| s.1)
}

/// Sends what is waiting, if there is a server and the player allows it.
pub fn send() -> Flush {
    match horizon_telemetry::endpoint() {
        Some(url) => upload::flush(&home(), &url),
        None => Flush::Nothing,
    }
}

/// [`send`] on its own thread, at start and when the window closes.
pub fn send_in_background() -> std::thread::JoinHandle<()> {
    std::thread::spawn(|| drop(send()))
}

/// The status as `telemetry status` and the MCP tool give it.
pub fn status_json() -> serde_json::Value {
    let consent = consent();
    serde_json::json!({
        "asked": !consent.needs_asking(),
        "consent_version": consent.version,
        "answered": consent.answered,
        "choices": consent.choices,
        "install_id": consent.install_id,
        "waiting": outbox::pending(&home()).len(),
        "server": horizon_telemetry::endpoint(),
        "ping": ping_text(),
    })
}

/// [`send`] as the launcher closes, waiting at most a few seconds for it.
pub fn send_before_exit() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        send();
        let _ = tx.send(());
    });
    let _ = rx.recv_timeout(std::time::Duration::from_secs(3));
}

/// The kinds named as the CLI and MCP take them: `performance`, `crashes`, `bugs`,
/// `benchmarks` (or `perf`, `crash`…), or `all`.
pub fn parse_kinds(names: &[String]) -> Result<Vec<Kind>, String> {
    let mut kinds = Vec::new();
    for name in names {
        match name.as_str() {
            "all" => kinds.extend(Kind::ALL),
            "performance" => kinds.push(Kind::Perf),
            "crashes" => kinds.push(Kind::Crash),
            "bugs" => kinds.push(Kind::Bug),
            "benchmarks" => kinds.push(Kind::Benchmark),
            other => kinds.push(Kind::from_name(other).ok_or_else(|| {
                format!("{other}: one of performance, crashes, bugs, benchmarks, all")
            })?),
        }
    }
    Ok(kinds)
}

/// Turns `kinds` on or off, keeping the other switches as they are (off if never asked).
pub fn set(kinds: &[Kind], on: bool) -> Result<Consent, String> {
    let current = consent();
    let mut choices = if current.needs_asking() {
        Choices::NONE
    } else {
        current.choices
    };
    for &kind in kinds {
        choices.set(kind, on);
    }
    answer(choices)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The official server's count through its SRV record, from the live server (network):
    /// `cargo test --lib official_count -- --ignored`. Its port 25565 is another server.
    #[test]
    #[ignore = "asks the live official server"]
    fn official_count_follows_the_srv_record() {
        let (host, port) =
            horizon_protocol::server_redirect(crate::server_list::OFFICIAL_ADDRESS, 25565);
        assert_eq!(
            (host.trim_end_matches('.'), port),
            ("play.playhorizon.dev", 24454)
        );
        assert!(official_players(crate::server_list::OFFICIAL_ADDRESS).is_some());
        assert_eq!(official_players(""), None);
    }

    #[test]
    fn the_stats_page_is_on_the_website() {
        assert_eq!(
            stats_url("https://playhorizon.dev").as_deref(),
            Some("https://playhorizon.dev/stats")
        );
        assert_eq!(
            stats_url("https://example.org/horizon/").as_deref(),
            Some("https://example.org/horizon/stats")
        );
        assert_eq!(stats_url(""), None);
        assert_eq!(stats_url("javascript:alert(1)"), None);
    }

    #[test]
    fn kinds_are_named_in_plain_words_or_short() {
        let names = |n: &[&str]| parse_kinds(&n.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(
            names(&["performance", "crash"]).unwrap(),
            [Kind::Perf, Kind::Crash]
        );
        assert_eq!(names(&["all"]).unwrap(), Kind::ALL);
        assert!(names(&["everything"]).is_err());
    }

    /// "View what would be sent" and `data show` list every hardware field.
    #[test]
    fn the_example_shows_every_hardware_field() {
        let json = serde_json::to_string(&with_example_gpu(System::detect())).unwrap();
        let fields = "os os_version arch cpu threads cores cpu_base_mhz cpu_boost_mhz ram_gb gpu \
            gpu_vendor gpu_kind vram_mb driver backend storage width height refresh_hz scale power form";
        for field in fields.split_whitespace() {
            assert!(
                json.contains(&format!("\"{field}\":")),
                "{field} missing from {json}"
            );
        }
    }

    #[test]
    fn every_kind_has_a_switch() {
        assert!(Kind::ALL.iter().all(|&k| !label(k).is_empty()));
    }

    /// Env-var tests share the process: one at a time.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// With no telemetry server configured (the default, or explicit `off`), "delete my data"
    /// deletes the waiting items locally but KEEPS the install ID, so a later ask with the
    /// server set can still delete what was sent before; the message says no server was asked.
    #[test]
    fn delete_my_data_without_a_server_keeps_the_install_id() {
        let _g = ENV_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("horizon-delete-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("HORIZON_HOME", &dir);
        std::env::remove_var("HORIZON_TELEMETRY_URL");

        let mut consent = Consent::load(&dir);
        consent.answer(Choices::ALL);
        consent.save(&dir).unwrap();
        let kept = consent.install_id.clone();
        assert!(!kept.is_empty());

        let message = delete_my_data().unwrap();
        assert!(message.contains("no server was asked"), "{message}");
        assert!(message.contains("deleted"), "{message}");

        let after = Consent::load(&dir);
        assert_eq!(after.install_id, kept, "install ID must survive a local-only delete");

        std::env::remove_var("HORIZON_HOME");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A fresh install that never sent anything is told so, and stays ID-less.
    #[test]
    fn delete_my_data_on_a_fresh_install_sends_and_changes_nothing() {
        let _g = ENV_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("horizon-delete-fresh-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("HORIZON_HOME", &dir);
        std::env::remove_var("HORIZON_TELEMETRY_URL");

        let message = delete_my_data().unwrap();
        assert!(message.contains("no telemetry ID recorded"), "{message}");
        assert!(Consent::load(&dir).install_id.is_empty());

        std::env::remove_var("HORIZON_HOME");
        std::fs::remove_dir_all(&dir).ok();
    }
}
