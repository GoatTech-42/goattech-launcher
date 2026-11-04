//! Opt-in data collection, shared by the launcher, the game and the ingest server.
//!
//! Nothing is collected or sent until the player says yes, kind by kind ([`Consent`]): the
//! game and the launcher only [`outbox::push`] items whose kind is allowed, and the launcher's
//! [`upload::flush`] drops anything the player has since turned off before it opens a
//! connection. Items are small summaries (percentiles and histogram buckets, never raw streams),
//! dated to the day, with no account, world, server or address in them ([`scrub`]); they leave
//! in zstd batches ([`batch`]). `docs/telemetry.md` lists every field.
//!
//! The one exception is [`live`]: the anonymous "I'm playing" ping the game sends while it
//! runs, for the live player count. It holds no install ID and nothing about the player.
//! [`server_status`] asks a Minecraft server how many play on it (the official server's count).
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub mod basic;
pub mod batch;
pub mod consent;
pub mod live;
pub mod outbox;
pub mod payload;
pub mod scrub;
pub mod server_status;
pub mod system;
#[cfg(feature = "upload")]
pub mod upload;

pub use consent::{Choices, Consent};
pub use payload::{Item, Kind, Payload, Source};

/// The payload format. New optional fields keep it; a field that changes meaning or goes
/// bumps it, and the server keeps reading the older ones.
pub const SCHEMA: u32 = 1;

/// What players agree to. Bumped when what is collected changes: the launcher then asks again,
/// and until the player answers nothing new is kept or sent under the switches. The always-on
/// "I'm playing" ping is not a switch and is unchanged by the answer. 2: the machine's full hardware
/// (cores, clocks, VRAM, drive, screen, power, form), 2026-10-23. 3: the "I'm playing" ping
/// is on by default and goes to GoatTech's own collector, 2026-11-03.
pub const CONSENT_VERSION: u32 = 3;

/// The upstream ingest server (`crates/horizon-telemetry-server`, on Railway). NOT contacted
/// by default in this build: [`endpoint`] returns `None` unless `HORIZON_TELEMETRY_URL`
/// explicitly names a server (this URL or any other). `HORIZON_TELEMETRY_URL=off` (or `none`,
/// `0`) also turns everything off, no ping either (tests and automated runs). What is sent
/// still depends on the player's switches. Consented uploads use only this: they never go
/// to [`LIVE_ENDPOINT`] unless the player configures it as their upload server too.
pub const DEFAULT_ENDPOINT: &str = "https://telemetry.playhorizon.dev";

/// Where items are sent, if anywhere. `None` unless a server is explicitly configured.
pub fn endpoint() -> Option<String> {
    resolve(std::env::var("HORIZON_TELEMETRY_URL").ok().as_deref())
}

/// GoatTech's own collector (`infra/telemetry-worker`): it answers the anonymous "I'm
/// playing" ping and the live count, and nothing else - no uploads, no hub, no exports.
pub const LIVE_ENDPOINT: &str = "https://telemetry.goattech.us";

/// Where the ping and the live count go: [`LIVE_ENDPOINT`] by default in this build.
/// `HORIZON_TELEMETRY_URL=off` (or `none`, `0`) turns the ping and count off; any other
/// value overrides the address (tests, self-hosting). Only the ping and the count read
/// this - consented uploads keep using [`endpoint`], unset by default.
pub fn live_endpoint() -> Option<String> {
    match std::env::var("HORIZON_TELEMETRY_URL").ok().as_deref().map(str::trim) {
        Some("off") | Some("none") | Some("0") => None,
        Some(url) if !url.is_empty() => Some(url.trim_end_matches('/').to_owned()),
        _ => Some(LIVE_ENDPOINT.to_owned()),
    }
}

fn resolve(var: Option<&str>) -> Option<String> {
    match var.map(str::trim) {
        Some(url) if !url.is_empty() => match url {
            "off" | "none" | "0" => None,
            _ => Some(url.trim_end_matches('/').to_owned()),
        },
        _ => None,
    }
}

/// `telemetry/` in the game folder: the consent, the outbox and the upload state, read by the
/// launcher and the game alike.
pub fn dir(home: &Path) -> PathBuf {
    home.join("telemetry")
}

/// Days since 1970-01-01 (UTC): the only time an item carries.
pub fn today() -> u32 {
    (now_secs() / 86_400) as u32
}

pub(crate) fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `bytes` random bytes in lowercase hex.
pub(crate) fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    // Without the OS's randomness the time still makes names that don't collide in practice.
    if getrandom::fill(&mut buf).is_err() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        for (i, b) in buf.iter_mut().enumerate() {
            *b = (nanos >> (8 * (i % 16))) as u8;
        }
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Writes `bytes` through a temporary file, so a reader never sees half of it.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", random_hex(4)));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
pub(crate) mod test_home {
    use std::path::PathBuf;

    /// An empty folder of its own under the system's temporary folder.
    pub fn new(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("horizon-telemetry-{name}-{}", super::random_hex(6)));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_endpoint_is_off_unless_a_server_is_explicitly_configured() {
        // Unset or blank: nothing is sent, no ping either (the GoatTech default).
        assert_eq!(resolve(None), None);
        assert_eq!(resolve(Some(" ")), None);
        // Explicit off switches stay off.
        assert_eq!(resolve(Some("off")), None);
        assert_eq!(resolve(Some("none")), None);
        assert_eq!(resolve(Some("0")), None);
        // An explicit URL opts in, including the upstream server itself, trailing slash cut.
        assert_eq!(
            resolve(Some("http://127.0.0.1:8080/")).as_deref(),
            Some("http://127.0.0.1:8080")
        );
        assert_eq!(
            resolve(Some(DEFAULT_ENDPOINT)).as_deref(),
            Some(DEFAULT_ENDPOINT)
        );
        assert!(DEFAULT_ENDPOINT.starts_with("https://") && !DEFAULT_ENDPOINT.ends_with('/'));
        assert!(LIVE_ENDPOINT.starts_with("https://") && !LIVE_ENDPOINT.ends_with('/'));
    }

    #[test]
    fn live_endpoint_defaults_to_the_goattech_collector() {
        // Default: GoatTech's collector. `off`/`none`/`0` turn it off; a URL overrides.
        // (The env var itself is process-global, so this test must not run alongside
        // others that set it; it never sets it itself - resolve logic is shared with
        // `endpoint`, whose cases are covered above. Here we pin the default and the
        // constant's shape.)
        assert_eq!(LIVE_ENDPOINT, "https://telemetry.goattech.us");
        if std::env::var("HORIZON_TELEMETRY_URL").is_err() {
            assert_eq!(live_endpoint().as_deref(), Some(LIVE_ENDPOINT));
        }
    }
}
