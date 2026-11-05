//! Basic performance summaries: a few coarse numbers after a game session, on by default on
//! genuinely new installs ("the nice statistics on by default... not very invasive"), with
//! their own switch beside the detailed, opt-in kinds.
//!
//! Everything here is deliberately narrow:
//! - one report per game session, only while the launcher is alive when the game ends;
//! - six possible keys at most, three of them coarse bucket labels, never a measurement itself;
//! - no IDs of any kind (no install, session, account or hardware ID), no raw streams;
//! - one bounded background POST to GoatTech's own collector, or nothing: no retries, no
//!   outbox, nothing on disk, and shutdown is never delayed for it.
//!
//! The destination is fixed: [`ENDPOINT`], and nothing else. `HORIZON_TELEMETRY_URL` can only
//! turn this off (`off`/`none`/`0`, a veto that covers every kind of telemetry); no value of
//! it can redirect these reports. A loopback destination exists only as a test seam, compiled
//! into debug builds with `telemetry-test-seam` and impossible in a release build.
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The payload's format marker.
pub const SCHEMA_ID: &str = "goattech-basic-stats/1";

/// The one destination: GoatTech's own collector. See the module docs: fixed, https only.
pub const ENDPOINT: &str = "https://telemetry.goattech.us/v1/basic-stats";
pub const ENDPOINT_HOST: &str = "telemetry.goattech.us";
pub const ENDPOINT_PATH: &str = "/v1/basic-stats";

/// The largest report body the launcher sends and the collector reads.
pub const MAX_BODY_BYTES: usize = 512;

/// A p95 frame time is reported only over at least this many rendered frames.
pub const MIN_P95_FRAMES: u64 = 240;

/// The one send's timeout: connect, send and answer together.
pub const SEND_TIMEOUT: Duration = Duration::from_secs(3);

/// The public GoatTech launcher releases a report may name: the running build's own tag must
/// be one of these, so a dev build, an unpublished tag or a custom string never sends. The
/// release process maintains this list; it is empty until the first public release, so nothing
/// sends before one exists.
pub const PUBLISHED_RELEASES: &[&str] = &[];

/// Whether `tag` is a public release a report may name.
pub fn release_allowed(tag: &str) -> bool {
    PUBLISHED_RELEASES.contains(&tag)
}

/// Whether `HORIZON_TELEMETRY_URL` vetoes all telemetry transport (`off`/`none`/`0`): the one
/// effect that variable can have on basic summaries. It never redirects them.
pub fn vetoed(var: Option<&str>) -> bool {
    matches!(
        var.map(str::trim),
        Some(v) if v.eq_ignore_ascii_case("off") || v.eq_ignore_ascii_case("none") || v == "0"
    )
}

/// The veto, read from the environment.
pub fn transport_vetoed() -> bool {
    vetoed(std::env::var("HORIZON_TELEMETRY_URL").ok().as_deref())
}

/// Where a report goes, if anywhere: [`ENDPOINT`], unless the transport veto is on. In debug
/// builds with the `telemetry-test-seam` feature a loopback URL can stand in for it; a release
/// build never reads that variable.
pub fn endpoint() -> Option<String> {
    if transport_vetoed() {
        return None;
    }
    #[cfg(all(debug_assertions, feature = "telemetry-test-seam"))]
    if let Some(url) = seam_endpoint(std::env::var("GOATTECH_BASIC_STATS_TEST_URL").ok().as_deref())
    {
        return Some(url);
    }
    Some(ENDPOINT.to_owned())
}

/// A test destination, when the seam is compiled in: loopback only, no credentials, so a test
/// collector on this machine can receive a report and nothing else can.
#[cfg(all(debug_assertions, feature = "telemetry-test-seam"))]
fn seam_endpoint(var: Option<&str>) -> Option<String> {
    let url = var?.trim().trim_end_matches('/');
    if url.is_empty() {
        return None;
    }
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))?;
    if rest.contains('@') {
        return None;
    }
    let host = rest
        .split(['/', '?', '#'])
        .next()?
        .split(':')
        .next()?
        .trim_matches(['[', ']']);
    matches!(host, "127.0.0.1" | "localhost" | "::1").then(|| url.to_owned())
}

/// Why the fixed [`ENDPOINT`] is not what it must be, as a static check and a unit test.
#[cfg(test)]
fn fixed_endpoint_error(url: &str) -> Option<&'static str> {
    let Some(rest) = url.strip_prefix("https://") else {
        return Some("https only");
    };
    if rest.contains('@') {
        return Some("no userinfo");
    }
    if url.contains(['?', '#']) {
        return Some("no query or fragment");
    }
    let Some((authority, path)) = rest.split_once('/').map(|(a, p)| (a, format!("/{p}")))
    else {
        return Some("the one path");
    };
    let (host, port) = match authority.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (authority, None),
    };
    if host != ENDPOINT_HOST {
        return Some("the host is the collector's own");
    }
    if port.is_some_and(|p| p != "443") {
        return Some("the default https port only");
    }
    if path != ENDPOINT_PATH {
        return Some("the one path");
    }
    None
}

/// This machine's OS as a report may name it; anything else sends nothing.
pub fn os() -> Option<&'static str> {
    match std::env::consts::OS {
        "windows" => Some("windows"),
        "macos" => Some("macos"),
        "linux" => Some("linux"),
        _ => None,
    }
}

/// One coarse range for a time-to-title in ms; the measurement itself never leaves the
/// machine. Ranges are contiguous and half-open, the last closed at the valid-range cap;
/// outside it is None: the field is then absent from the report, never zeroed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ttt {
    #[serde(rename = "lt500")]
    Lt500,
    #[serde(rename = "500_1000")]
    B500,
    #[serde(rename = "1000_2000")]
    B1000,
    #[serde(rename = "2000_4000")]
    B2000,
    #[serde(rename = "4000_8000")]
    B4000,
    #[serde(rename = "8000_16000")]
    B8000,
    #[serde(rename = "ge16000")]
    Ge16000,
}

impl Ttt {
    /// Valid range [0, 120000] ms.
    pub fn from_ms(ms: u64) -> Option<Self> {
        Some(match ms {
            0..=499 => Self::Lt500,
            500..=999 => Self::B500,
            1000..=1999 => Self::B1000,
            2000..=3999 => Self::B2000,
            4000..=7999 => Self::B4000,
            8000..=15999 => Self::B8000,
            16000..=120000 => Self::Ge16000,
            _ => return None,
        })
    }
}

/// The same, for a time-to-first-world in ms. Valid range [0, 300000].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ttw {
    #[serde(rename = "lt1000")]
    Lt1000,
    #[serde(rename = "1000_3000")]
    B1000,
    #[serde(rename = "3000_6000")]
    B3000,
    #[serde(rename = "6000_12000")]
    B6000,
    #[serde(rename = "12000_30000")]
    B12000,
    #[serde(rename = "ge30000")]
    Ge30000,
}

impl Ttw {
    pub fn from_ms(ms: u64) -> Option<Self> {
        Some(match ms {
            0..=999 => Self::Lt1000,
            1000..=2999 => Self::B1000,
            3000..=5999 => Self::B3000,
            6000..=11999 => Self::B6000,
            12000..=29999 => Self::B12000,
            30000..=300000 => Self::Ge30000,
            _ => return None,
        })
    }
}

/// The same, for a p95 frame time in ms. Domain (0, 1000]: zero, negative, NaN, inf and
/// anything over a second are invalid, not clamped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum P95 {
    #[serde(rename = "lt8_4")]
    Lt8_4,
    #[serde(rename = "8_4_16_7")]
    B8_4,
    #[serde(rename = "16_7_33_4")]
    B16_7,
    #[serde(rename = "33_4_66_7")]
    B33_4,
    #[serde(rename = "66_7_100_0")]
    B66_7,
    #[serde(rename = "ge100_0")]
    Ge100,
}

impl P95 {
    /// The bucket a p95 frame time falls in: contiguous half-open ranges from just above 0,
    /// the last closed at 1000.
    pub fn from_frame_ms(ms: f64) -> Option<Self> {
        if !ms.is_finite() || ms <= 0.0 || ms > 1000.0 {
            return None;
        }
        Some(match ms {
            m if m < 8.4 => Self::Lt8_4,
            m if m < 16.7 => Self::B8_4,
            m if m < 33.4 => Self::B16_7,
            m if m < 66.7 => Self::B33_4,
            m if m < 100.0 => Self::B66_7,
            _ => Self::Ge100,
        })
    }
}

/// One session's report: the format version, the public release and OS, and whichever of the
/// three coarse measurements the session produced (a missing one is absent, never zero).
/// At least one measurement is required; an empty report is not sent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema: String,
    pub release: String,
    pub os: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttt_ms: Option<Ttt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttw_ms: Option<Ttw>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p95_ms: Option<P95>,
}

impl Report {
    /// The report for a session, if there is anything to say and this build may say it:
    /// `release` is the launcher's own version, which must be a published public release, and
    /// the OS one reports name. With no measurement, None: nothing is sent.
    pub fn build(
        release: &str,
        ttt: Option<Ttt>,
        ttw: Option<Ttw>,
        p95: Option<P95>,
    ) -> Option<Self> {
        if ttt.is_none() && ttw.is_none() && p95.is_none() {
            return None;
        }
        if !release_allowed(release) {
            return None;
        }
        let report = Self {
            schema: SCHEMA_ID.to_owned(),
            release: release.to_owned(),
            os: os()?.to_owned(),
            ttt_ms: ttt,
            ttw_ms: ttw,
            p95_ms: p95,
        };
        report.check().ok()?;
        Some(report)
    }

    /// Why the collector would refuse this report, if it would: the same allowlist and size
    /// limit, checked before a byte leaves the machine.
    pub fn check(&self) -> Result<(), &'static str> {
        if self.schema != SCHEMA_ID {
            return Err("schema: goattech-basic-stats/1");
        }
        if !release_allowed(&self.release) {
            return Err("release: not a published public release");
        }
        if !matches!(self.os.as_str(), "windows" | "macos" | "linux") {
            return Err("os: windows, macos or linux");
        }
        if self.ttt_ms.is_none() && self.ttw_ms.is_none() && self.p95_ms.is_none() {
            return Err("at least one measurement");
        }
        let body = serde_json::to_vec(self).map_err(|_| "not JSON")?;
        if body.len() > MAX_BODY_BYTES {
            return Err("over 512 bytes");
        }
        Ok(())
    }
}

/// Sends one report, once, to `url` ([`endpoint`]'s answer): a bounded POST with no redirects
/// followed, no cookies, no proxy, and no retry - a failure or refusal is simply dropped.
#[cfg(feature = "basic")]
pub fn send_to(url: &str, report: &Report) -> Result<(), String> {
    report.check().map_err(str::to_owned)?;
    let body = serde_json::to_vec(report).map_err(|e| e.to_string())?;
    // No cookie store is compiled in (the crate has no cookies feature), and no proxy is set:
    // ureq only uses one when the caller names it, which nothing here does.
    let agent = ureq::AgentBuilder::new()
        .timeout(SEND_TIMEOUT)
        .redirects(0)
        .build();
    match agent
        .post(url)
        .set("Content-Type", "application/json")
        .send_bytes(&body)
    {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_veto_reads_only_off_none_and_zero() {
        for v in ["off", "OFF", " Off ", "none", "None", "0"] {
            assert!(vetoed(Some(v)), "{v}");
        }
        assert!(vetoed(Some("off "))); // trims to "off"
        for v in [None, Some(""), Some("  "), Some("1"), Some("https://evil.example")] {
            assert!(!vetoed(v), "{v:?}");
        }
        // A URL is never a destination for these reports, veto or not.
        assert!(!vetoed(Some("http://127.0.0.1:8080")));
    }

    #[test]
    fn the_fixed_endpoint_is_exactly_the_collector() {
        assert_eq!(fixed_endpoint_error(ENDPOINT), None);
        assert!(fixed_endpoint_error("http://telemetry.goattech.us/v1/basic-stats").is_some());
        assert!(fixed_endpoint_error("https://telemetry.goattech.us:8443/v1/basic-stats").is_some());
        assert!(fixed_endpoint_error("https://user@telemetry.goattech.us/v1/basic-stats").is_some());
        assert!(fixed_endpoint_error("https://telemetry.goattech.us/v1/basic-stats?x=1").is_some());
        assert!(fixed_endpoint_error("https://telemetry.goattech.us/v1/basic-stats#f").is_some());
        assert!(fixed_endpoint_error("https://evil.example/v1/basic-stats").is_some());
        assert!(fixed_endpoint_error("https://telemetry.goattech.us/v1/other").is_some());
        assert_eq!(fixed_endpoint_error("https://telemetry.goattech.us:443/v1/basic-stats"), None);
    }

    #[test]
    fn no_release_is_publishable_before_the_first_release() {
        assert!(!release_allowed("0.2.0"));
        assert!(!release_allowed("v0.2.0"));
        assert!(!release_allowed("dev 0.1.0"));
        assert!(PUBLISHED_RELEASES.is_empty());
    }

    #[test]
    fn bucket_ranges_are_contiguous_and_capped() {
        // Time to title: [0, 120000], half-open rungs.
        assert_eq!(Ttt::from_ms(0), Some(Ttt::Lt500));
        assert_eq!(Ttt::from_ms(499), Some(Ttt::Lt500));
        assert_eq!(Ttt::from_ms(500), Some(Ttt::B500));
        assert_eq!(Ttt::from_ms(999), Some(Ttt::B500));
        assert_eq!(Ttt::from_ms(1000), Some(Ttt::B1000));
        assert_eq!(Ttt::from_ms(15999), Some(Ttt::B8000));
        assert_eq!(Ttt::from_ms(16000), Some(Ttt::Ge16000));
        assert_eq!(Ttt::from_ms(120000), Some(Ttt::Ge16000));
        assert_eq!(Ttt::from_ms(120001), None);
        // Time to first world.
        assert_eq!(Ttw::from_ms(0), Some(Ttw::Lt1000));
        assert_eq!(Ttw::from_ms(2999), Some(Ttw::B1000));
        assert_eq!(Ttw::from_ms(3000), Some(Ttw::B3000));
        assert_eq!(Ttw::from_ms(300000), Some(Ttw::Ge30000));
        assert_eq!(Ttw::from_ms(300001), None);
    }

    #[test]
    fn p95_accepts_only_the_frame_time_domain() {
        // Zero is not a frame time; neither are NaN, inf, negatives or over a second.
        assert_eq!(P95::from_frame_ms(0.0), None);
        assert_eq!(P95::from_frame_ms(-1.0), None);
        assert_eq!(P95::from_frame_ms(f64::NAN), None);
        assert_eq!(P95::from_frame_ms(f64::INFINITY), None);
        assert_eq!(P95::from_frame_ms(1000.1), None);
        assert_eq!(P95::from_frame_ms(1000.0), Some(P95::Ge100));
        // No gaps between rungs.
        assert_eq!(P95::from_frame_ms(8.39), Some(P95::Lt8_4));
        assert_eq!(P95::from_frame_ms(8.4), Some(P95::B8_4));
        assert_eq!(P95::from_frame_ms(16.6999), Some(P95::B8_4));
        assert_eq!(P95::from_frame_ms(16.7), Some(P95::B16_7));
        assert_eq!(P95::from_frame_ms(33.4), Some(P95::B33_4));
        assert_eq!(P95::from_frame_ms(66.7), Some(P95::B66_7));
        assert_eq!(P95::from_frame_ms(99.99), Some(P95::B66_7));
        assert_eq!(P95::from_frame_ms(100.0), Some(P95::Ge100));
    }

    fn report() -> Report {
        Report {
            schema: SCHEMA_ID.to_owned(),
            release: "v0.0.0-test".to_owned(),
            os: "linux".to_owned(),
            ttt_ms: Some(Ttt::B1000),
            ttw_ms: None,
            p95_ms: None,
        }
    }

    #[test]
    fn a_report_names_exactly_its_keys() {
        let json = serde_json::to_value(report()).unwrap();
        let mut keys: Vec<&str> = json.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["os", "release", "schema", "ttt_ms"]);
        assert_eq!(json["ttt_ms"], "1000_2000");
        // Unknown fields are refused on the way in (the collector's own check).
        let extra = r#"{"schema":"goattech-basic-stats/1","release":"v1.0.0","os":"linux","ttt_ms":"lt500","note":"x"}"#;
        assert!(serde_json::from_str::<Report>(extra).is_err());
        assert!(serde_json::from_str::<Report>(
            r#"{"schema":"goattech-basic-stats/1","release":"v1.0.0","os":"linux","ttt_ms":"999"}"#
        )
        .is_err());
    }

    #[test]
    fn a_report_needs_a_measurement_and_fits_its_size() {
        let mut empty = report();
        empty.ttt_ms = None;
        assert!(empty.check().is_err());
        let full = Report { ttw_ms: Some(Ttw::Ge30000), p95_ms: Some(P95::Ge100), ..report() };
        // The allowlist is empty before the first release, so check() refuses on release here;
        // size is what this test pins.
        let body = serde_json::to_vec(&full).unwrap();
        assert!(body.len() <= MAX_BODY_BYTES, "{body:?}");
    }

    #[test]
    fn nothing_is_built_without_a_published_release_or_a_measurement() {
        assert!(Report::build("v1.0.0", None, None, None).is_none());
        assert!(Report::build("dev 0.1.0", Some(Ttt::Lt500), None, None).is_none());
    }

    #[cfg(all(debug_assertions, feature = "telemetry-test-seam"))]
    #[test]
    fn the_test_seam_is_loopback_only() {
        assert_eq!(
            seam_endpoint(Some("http://127.0.0.1:8787/v1/basic-stats")).as_deref(),
            Some("http://127.0.0.1:8787/v1/basic-stats")
        );
        assert_eq!(
            seam_endpoint(Some("http://localhost:8787/")).as_deref(),
            Some("http://localhost:8787")
        );
        assert!(seam_endpoint(Some("https://telemetry.evil.example")).is_none());
        assert!(seam_endpoint(Some("http://user@127.0.0.1:8787")).is_none());
        assert!(seam_endpoint(Some("http://10.0.0.5:8787")).is_none());
        assert!(seam_endpoint(None).is_none());
    }

    /// A 3xx is a drop, never a follow: the redirect target is never touched.
    #[cfg(all(debug_assertions, feature = "telemetry-test-seam"))]
    #[test]
    fn redirects_are_not_followed() {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let hits = std::sync::Arc::new(AtomicUsize::new(0));
        let target = TcpListener::bind("127.0.0.1:0").unwrap();
        let target_addr = target.local_addr().unwrap();
        let hits2 = std::sync::Arc::clone(&hits);
        std::thread::spawn(move || {
            for stream in target.incoming() {
                hits2.fetch_add(1, Ordering::SeqCst);
                drop(stream);
            }
        });
        let source = TcpListener::bind("127.0.0.1:0").unwrap();
        let source_addr = source.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = source.accept() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                // Read the request head before answering.
                for line in reader.lines().map_while(Result::ok) {
                    if line.is_empty() {
                        break;
                    }
                }
                let body = "redirect";
                let _ = write!(
                    stream,
                    "HTTP/1.1 301 Moved Permanently\r\nLocation: http://{target_addr}/trap\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        let url = format!("http://{source_addr}/v1/basic-stats");
        assert_eq!(seam_endpoint(Some(&url)).as_deref(), Some(url.as_str()));
        let report = Report {
            schema: SCHEMA_ID.to_owned(),
            release: "v0.0.0-test".to_owned(),
            os: "linux".to_owned(),
            ttt_ms: Some(Ttt::Lt500),
            ttw_ms: None,
            p95_ms: None,
        };
        // check() refuses the unpublished test tag, so build the body directly here: this
        // test pins the HTTP behavior, not the allowlist.
        let body = serde_json::to_vec(&report).unwrap();
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(3))
            .redirects(0)
            .build();
        let _ = agent
            .post(&url)
            .set("Content-Type", "application/json")
            .send_bytes(&body);
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(hits.load(Ordering::SeqCst), 0, "the redirect target was hit");
    }

    /// A release build has no test seam at all: the variable is never read.
    #[test]
    fn a_release_build_cannot_be_redirected() {
        if cfg!(debug_assertions) {
            return; // the seam's own tests cover debug builds
        }
        // Only this test touches the variable, and in a release test run the seam tests are
        // not compiled, so nothing else reads it concurrently.
        unsafe { std::env::set_var("GOATTECH_BASIC_STATS_TEST_URL", "http://127.0.0.1:9/") };
        if std::env::var("HORIZON_TELEMETRY_URL").is_err() {
            assert_eq!(endpoint().as_deref(), Some(ENDPOINT));
        }
    }
}
