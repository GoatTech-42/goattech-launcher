//! Local Fabric client-JAR inspection and installation. Client-specific config formats are
//! not inferred from loader metadata; this module does not sync or download client files.
use std::fs;
use std::io::Read;
use std::path::Path;

use serde_json::Value;

use crate::instances;
use crate::loaders::{self, Kind, Loader};
use crate::minecraft_versions::Versions;
use crate::sources::Fetch;

/// A `fabric.mod.json` is a few kilobytes; refuse to read more than this from any JAR.
const MAX_META_BYTES: u64 = 1 << 20;
/// Bounded archive inspection: larger archives need a separate review.
const MAX_ENTRIES: usize = 8192;

/// Which side of the game a mod declares itself for (`environment` in `fabric.mod.json`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// `"*"` or no `environment` at all: both.
    Any,
    Client,
    Server,
}

impl Side {
    pub fn label(self) -> &'static str {
        match self {
            Side::Any => "client and server",
            Side::Client => "client",
            Side::Server => "server",
        }
    }
}

/// What a local client JAR says about itself, per Fabric's contract
/// (<https://docs.fabricmc.net/develop/loader/fabric-mod-json>).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientJar {
    /// Its mod id (`"dawn"`), the name the loader knows it by.
    pub mod_id: String,
    /// Its display name (`name`, else its id).
    pub name: String,
    pub version: String,
    /// Its license, if it says one.
    pub license: Option<String>,
    pub side: Side,
    /// Whether it declares entrypoints (not a complete measure of a mod's behavior).
    pub entrypoints: bool,
    /// The version range it wants of Fabric Loader (`depends.fabricloader`), raw.
    pub loader_wants: Option<String>,
    /// The Minecraft versions it says it runs on (`depends.minecraft`), raw.
    pub minecraft_wants: Option<String>,
    /// The Java version it wants (`depends.java`), raw.
    pub java_wants: Option<String>,
    /// Its other declared dependencies, mod id to raw range (`fabric-api` and friends).
    pub mod_wants: Vec<(String, String)>,
}

/// Whether `wanted` (a Fabric version range) allows `version`. None: the range or the version
/// is outside the forms this reads (snapshots, odd pre-release names), so no claim either way.
///
/// The forms, per Fabric's contract: `*`; an exact version (`1.20.1`); a wildcard (`1.20.x`);
/// `~1.20` (that minor's releases); `>=`, `>`, `<=`, `<`; a space-separated list ANDs; a JSON
/// array of ranges ORs (kept here as `;`-separated alternatives).
pub fn range_allows(wanted: &str, version: &str) -> Option<bool> {
    if wanted.split(';').any(|alt| alt.trim().is_empty()) {
        return None;
    }
    let alts: Vec<Option<bool>> = wanted
        .split(';')
        .map(|alt| {
            for token in alt.split_whitespace() {
                if !token_allows(token, version)? {
                    return Some(false);
                }
            }
            Some(true)
        })
        .collect();
    if alts.contains(&Some(true)) {
        Some(true)
    } else if alts.contains(&None) {
        None
    } else {
        Some(false)
    }
}

/// One range token against a version.
fn token_allows(token: &str, version: &str) -> Option<bool> {
    if token == "*" {
        return Some(true);
    }
    let version = version.split_once('+').map_or(version, |(v, _)| v);
    let token = token.split_once('+').map_or(token, |(v, _)| v);
    if let Some(prefix) = token
        .strip_suffix(".x")
        .or_else(|| token.strip_suffix(".*"))
    {
        let want: Vec<&str> = prefix.split('.').collect();
        let have: Vec<&str> = version.split('.').collect();
        if want.len() > have.len() {
            return None;
        }
        let mut ok = Some(true);
        for (w, h) in want.iter().zip(have.iter()) {
            match (w.parse::<u64>(), h.parse::<u64>()) {
                (Ok(w), Ok(h)) if w == h => {}
                (Ok(_), Ok(_)) => return Some(false),
                _ => ok = None,
            }
        }
        return ok;
    }
    for (mark, ord) in [
        (">=", std::cmp::Ordering::Greater),
        ("<=", std::cmp::Ordering::Less),
        (">", std::cmp::Ordering::Greater),
        ("<", std::cmp::Ordering::Less),
    ] {
        if let Some(bound) = token.strip_prefix(mark) {
            let cmp = compare_versions(version, bound)?;
            let inclusive = mark.len() == 2;
            return Some(cmp == ord || (inclusive && cmp == std::cmp::Ordering::Equal));
        }
    }
    if let Some(base) = token.strip_prefix('~') {
        // ~1.20: the 1.20.x line; ~1.20.1: from 1.20.1 to before 1.21.
        let parts: Vec<&str> = base.split('.').collect();
        let upper: Option<String> = match parts.as_slice() {
            [major, minor] => Some(format!(
                "{}.{}",
                major,
                minor.parse::<u64>().ok()?.checked_add(1)?
            )),
            [major, minor, _patch, ..] => Some(format!(
                "{}.{}",
                major,
                minor.parse::<u64>().ok()?.checked_add(1)?
            )),
            _ => None,
        };
        let low = compare_versions(version, base)?;
        let high = compare_versions(version, &upper?)?;
        return Some(
            matches!(low, std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
                && high == std::cmp::Ordering::Less,
        );
    }
    // A bare version is an exact pin.
    match compare_versions(version, token) {
        Some(std::cmp::Ordering::Equal) => Some(true),
        Some(_) => Some(false),
        None => None,
    }
}

/// Component-wise numeric compare (missing numeric components are zero). None: a component neither
/// side has as a number (a snapshot name, a pre-release suffix), so no order is claimed.
fn compare_versions(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let pa: Vec<&str> = a.split('.').collect();
    let pb: Vec<&str> = b.split('.').collect();
    if pa
        .iter()
        .chain(pb.iter())
        .any(|s| s.parse::<u64>().is_err())
    {
        return None;
    }
    for i in 0..pa.len().max(pb.len()) {
        let na = pa.get(i).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        let nb = pb.get(i).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        if pa.get(i).is_some_and(|s| s.parse::<u64>().is_err())
            || pb.get(i).is_some_and(|s| s.parse::<u64>().is_err())
        {
            return None;
        }
        match na.cmp(&nb) {
            std::cmp::Ordering::Equal => {}
            other => return Some(other),
        }
    }
    Some(std::cmp::Ordering::Equal)
}

/// Whether a mod id is the shape Fabric demands: `^[a-z][a-z0-9-_]{1,63}$`.
fn mod_id_ok(id: &str) -> bool {
    let mut chars = id.chars();
    let ok_first = chars.next().is_some_and(|c| c.is_ascii_lowercase());
    ok_first
        && (2..=64).contains(&id.len())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// The ranges a `depends` value names, as `;`-separated alternatives: a string is one
/// alternative, an array of strings is a choice between them.
fn depends_range(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => (!s.contains(';')).then(|| s.clone()),
        Value::Array(all) => {
            let parts: Option<Vec<&str>> = all.iter().map(|v| v.as_str()).collect();
            parts
                .filter(|p| p.iter().all(|s| !s.contains(';')))
                .map(|p| p.join(";"))
        }
        _ => None,
    }
}

/// Reads `path`'s `fabric.mod.json` (never runs the JAR) and checks it against Fabric's
/// contract: schemaVersion 1, a well-shaped id, a version. The metadata read is capped and
/// nothing is extracted to disk.
pub fn inspect(path: &Path) -> Result<ClientJar, String> {
    if path.extension().and_then(|e| e.to_str()) != Some("jar") {
        return Err(format!(
            "{} is not a .jar file. install-client takes a client mod JAR.",
            path.display()
        ));
    }
    let bytes = read_jar(path)?;
    inspect_bytes(path, &bytes)
}

const MAX_JAR_BYTES: u64 = 256 * 1024 * 1024;

fn read_jar(path: &Path) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("A client JAR must be a regular file.".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_JAR_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_JAR_BYTES {
        return Err("Client JAR exceeds the 256 MiB inspection limit.".into());
    }
    Ok(bytes)
}

fn inspect_bytes(path: &Path, bytes: &[u8]) -> Result<ClientJar, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|_| format!("{} is not a readable JAR (a zip).", path.display()))?;
    if zip.len() > MAX_ENTRIES {
        return Err(format!(
            "{} names {} entries, over the {MAX_ENTRIES} a client mod can have here.",
            path.display(),
            zip.len()
        ));
    }
    if zip
        .file_names()
        .filter(|name| name.as_ref().is_ok_and(|n| n == "fabric.mod.json"))
        .count()
        > 1
    {
        return Err("JAR has duplicate fabric.mod.json entries.".into());
    }
    if zip.by_name("fabric.mod.json").is_err() {
        let what = if zip.by_name("quilt.mod.json").is_ok() {
            "a Quilt mod (quilt.mod.json, no fabric.mod.json): make its instance a Quilt one"
        } else {
            "no mod metadata at all: it is not a Fabric client mod"
        };
        return Err(format!("{} has {what}.", path.display()));
    }
    let mut entry = zip
        .by_name("fabric.mod.json")
        .map_err(|e| format!("{}'s fabric.mod.json: {e}", path.display()))?;
    if entry.size() > MAX_META_BYTES {
        return Err(format!(
            "{}'s fabric.mod.json is over 1 MiB: not a real mod manifest.",
            path.display()
        ));
    }
    let mut text = String::new();
    entry
        .by_ref()
        .take(MAX_META_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|e| format!("{}'s fabric.mod.json: {e}", path.display()))?;
    if text.len() as u64 > MAX_META_BYTES {
        return Err(format!(
            "{}'s fabric.mod.json is over 1 MiB: not a real mod manifest.",
            path.display()
        ));
    }
    let json: Value = serde_json::from_str(&text)
        .map_err(|e| format!("{}'s fabric.mod.json is not JSON: {e}", path.display()))?;
    if json["schemaVersion"] != 1 {
        return Err(format!(
            "{}'s fabric.mod.json has schemaVersion {}, not the 1 this reads.",
            path.display(),
            json["schemaVersion"]
        ));
    }
    // shortcut: nested/provided mods and hard conflicts need Fabric's full resolver;
    // refuse them until that resolver is integrated rather than asserting compatibility.
    for key in ["jars", "provides", "breaks"] {
        if json.get(key).is_some_and(|v| match v {
            Value::Array(a) => !a.is_empty(),
            Value::Object(o) => !o.is_empty(),
            _ => true,
        }) {
            return Err(format!("fabric.mod.json {key} needs dependency resolution not supported by this installer yet."));
        }
    }
    let mod_id = json["id"]
        .as_str()
        .ok_or_else(|| format!("{}'s fabric.mod.json has no id.", path.display()))?;
    if !mod_id_ok(mod_id) {
        return Err(format!(
            "{}'s fabric.mod.json id {mod_id:?} is not a Fabric mod id (lowercase, digits, - or _).",
            path.display()
        ));
    }
    let version = json["version"]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("{}'s fabric.mod.json has no version.", path.display()))?
        .to_string();
    if json.get("environment").is_some_and(|v| !v.is_string()) {
        return Err("fabric.mod.json environment must be a string.".into());
    }
    let side = match json["environment"].as_str() {
        None | Some("*") => Side::Any,
        Some("client") => Side::Client,
        Some("server") => Side::Server,
        Some(other) => {
            return Err(format!(
                "{}'s fabric.mod.json environment {other:?} is not one of *, client, server.",
                path.display()
            ));
        }
    };
    let license = match &json["license"] {
        Value::String(s) => (!s.contains(';')).then(|| s.clone()),
        Value::Array(all) => {
            let parts: Vec<&str> = all.iter().filter_map(|v| v.as_str()).collect();
            (!parts.is_empty()).then(|| parts.join(", "))
        }
        _ => None,
    };
    let mut jar = ClientJar {
        mod_id: mod_id.to_string(),
        name: json["name"]
            .as_str()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .unwrap_or(mod_id)
            .to_string(),
        version,
        license,
        side,
        entrypoints: json["entrypoints"]
            .as_object()
            .is_some_and(|e| !e.is_empty()),
        loader_wants: None,
        minecraft_wants: None,
        java_wants: None,
        mod_wants: Vec::new(),
    };
    if json.get("depends").is_some_and(|v| !v.is_object()) {
        return Err("fabric.mod.json depends must be an object.".into());
    }
    if let Some(depends) = json["depends"].as_object() {
        for (key, value) in depends {
            let range = depends_range(value)
                .filter(|r| !r.split(';').any(|s| s.trim().is_empty()))
                .ok_or_else(|| {
                    format!("fabric.mod.json depends.{key} must name nonempty ranges.")
                })?;
            match key.as_str() {
                "fabricloader" => jar.loader_wants = Some(range),
                "minecraft" => jar.minecraft_wants = Some(range),
                "java" => jar.java_wants = Some(range),
                other => jar.mod_wants.push((other.to_string(), range)),
            }
        }
    }
    Ok(jar)
}

/// What the install was asked for.
pub struct Goal<'a> {
    /// The local client JAR.
    pub jar: &'a Path,
    /// Install into this existing instance (its name or id) instead of making a new one.
    pub instance: Option<&'a str>,
    /// The new instance's name (default: the mod's display name).
    pub name: Option<&'a str>,
    /// The new instance's Minecraft version (default: the newest release the JAR allows).
    pub minecraft: Option<&'a str>,
    /// The Fabric Loader version (default: its newest stable one).
    pub loader_version: Option<&'a str>,
}

/// What an install did, for the report.
#[derive(Debug)]
pub struct Outcome {
    pub jar: ClientJar,
    /// The instance it landed in (name).
    pub instance: String,
    /// Whether the install made that instance.
    pub created: bool,
    pub minecraft: String,
    /// `fabric 0.16.9`, as the instance now records it.
    pub loader: String,
    /// Where the JAR went, relative to the game folder (`mods/dawn.jar`).
    pub file: String,
    /// Honest caveats: compatibility this could not verify, and what is not done yet.
    pub notes: Vec<String>,
}

/// Installs a checked local snapshot. Loader libraries and the game download at first play.
/// New instances are prepared off-list, then published with locked metadata.
pub fn install(
    game_home: &Path,
    data: &Path,
    versions: Option<&Versions>,
    goal: &Goal,
    fetch: Fetch,
) -> Result<Outcome, String> {
    install_reviewed(game_home, data, versions, goal, fetch, None)
}

/// A preview can bind the second-click install to the exact bounded bytes it inspected.
fn review_hash(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(bytes))
}

pub fn inspect_for_review(path: &Path) -> Result<(ClientJar, String), String> {
    if path.extension().and_then(|e| e.to_str()) != Some("jar") {
        return Err("Choose a .jar client file.".into());
    }
    let bytes = read_jar(path)?;
    let info = inspect_bytes(path, &bytes)?;
    Ok((info, review_hash(&bytes)))
}

pub fn install_reviewed(
    game_home: &Path,
    data: &Path,
    versions: Option<&Versions>,
    goal: &Goal,
    fetch: Fetch,
    expected: Option<&str>,
) -> Result<Outcome, String> {
    // Inspect and install the same bounded snapshot, never re-open the source after checks.
    if goal.jar.extension().and_then(|e| e.to_str()) != Some("jar") {
        return Err("Client installation takes a .jar file.".into());
    }
    let bytes = read_jar(goal.jar)?;
    if expected.is_some_and(|hash| hash != review_hash(&bytes)) {
        return Err("Client JAR changed since preview. Choose it again before installing.".into());
    }
    let jar = inspect_bytes(goal.jar, &bytes)?;
    let dest = dest_file(goal.jar)?;
    if jar.side == Side::Server {
        return Err(format!(
            "{} ({}) declares itself server-only: it is not a client mod.",
            jar.name, jar.mod_id
        ));
    }
    let mut notes = Vec::new();
    if !jar.entrypoints {
        notes.push(
            "It declares no entrypoints; mixins or other loader features may still run code."
                .to_string(),
        );
    }
    let all = instances::list_in(game_home, data);
    let mut install_mark = InstallMark(None);
    let (instance, created, loader) = match goal.instance {
        Some(query) => {
            let found = instances::find_in(&all, query)?;
            let Some(java) = found.java.clone() else {
                return Err(format!(
                    "{} is not a Minecraft: Java Edition instance, so a Fabric client JAR cannot go in it.",
                    found.name
                ));
            };
            let Some(loader) = java.loader.clone() else {
                return Err(format!(
                    "{} has no mod loader: give it one first (`horizon instance loader {} fabric`).",
                    found.name, found.id
                ));
            };
            if !matches!(loader.kind, Kind::Fabric | Kind::Quilt) {
                return Err(format!(
                    "{} runs {}, which cannot load a Fabric client JAR.",
                    found.name,
                    loader.kind.name()
                ));
            }
            if loader.kind != Kind::Fabric {
                return Err("Client JAR installation currently verifies Fabric Loader only, not Quilt compatibility.".into());
            }
            let mark = crate::config_sync::claim_running(&found.home)?.ok_or(
                "This instance is running or held. Quit the game before installing a client.",
            )?;
            install_mark.0 = Some(mark);
            ensure_no_java_process()?;
            check_loader(&jar, &loader.version)?;
            check_mod_dependencies(&jar, Some(&found.home))?;
            check_java(&jar, &java, versions, fetch)?;
            check_minecraft(&jar, &java.minecraft, &mut notes)?;
            let dest = dest_file(goal.jar)?;
            if found.home.join(&dest).exists() {
                return Err(format!("{} already has {dest}.", found.name));
            }
            (found, false, loader)
        }
        None => {
            let versions = versions.ok_or("Making an instance needs Mojang's version list.")?;
            let minecraft = match goal.minecraft {
                Some(wanted) => versions.find(wanted)?.id.clone(),
                None => pick_minecraft(&jar, versions)?,
            };
            check_minecraft(&jar, &minecraft, &mut notes)?;
            // The loader first, as `instance create` does: a Minecraft version it has no
            // build for makes no instance.
            let listed = loaders::versions(Kind::Fabric, &minecraft, fetch)?;
            let version = loaders::pick(Kind::Fabric, &listed, goal.loader_version)?;
            check_loader(&jar, &version)?;
            let loader = Loader {
                kind: Kind::Fabric,
                version,
            };
            check_mod_dependencies(&jar, None)?;
            check_java(
                &jar,
                &instances::JavaGame {
                    minecraft: minecraft.clone(),
                    ..Default::default()
                },
                Some(versions),
                fetch,
            )?;
            let name = goal.name.unwrap_or(&jar.name);
            let made = instances::create_client_in(
                game_home,
                data,
                name,
                instances::JavaGame {
                    minecraft,
                    loader: Some(loader.clone()),
                    ..Default::default()
                },
                |home| copy_bytes(home, &bytes, &dest),
            )?;
            (made, true, loader)
        }
    };
    let result = if created {
        Ok(())
    } else {
        instances::with_checked_client_target(game_home, data, &instance, || {
            copy_bytes(&instance.home, &bytes, &dest)
        })
    };
    result?;
    if let Some(mark) = install_mark.0.take() {
        mark.release()?;
    }
    notes.push(
        "Its own config file is not synced. options.txt sharing is available separately: join a group in this instance's sharing settings or with `instance sync`. New instances do not join automatically."
            .to_string(),
    );
    Ok(Outcome {
        minecraft: instance
            .java
            .as_ref()
            .map(|j| j.minecraft.clone())
            .unwrap_or_default(),
        loader: format!("{} {}", loader.kind.id(), loader.version),
        instance: instance.name,
        created,
        jar,
        file: dest,
        notes,
    })
}

/// Shared CLI/GUI eligibility seam. The mark's child lifecycle is owned by the caller.
pub fn claim_for_launch(
    java: bool,
    synced: bool,
    home: &Path,
) -> Result<Option<crate::config_sync::RunMark>, String> {
    if !java && !synced {
        return Ok(None);
    }
    crate::config_sync::claim_running(home)?
        .map(Some)
        .ok_or_else(|| {
            "Another run or client installation holds this instance. Quit it before launching."
                .into()
        })
}

/// Older/external Java games may not hold our mark. Conservatively refuse while any Java
/// process exists. Failure to enumerate processes is not evidence that games have stopped.
fn ensure_no_java_process() -> Result<(), String> {
    #[cfg(unix)]
    let output = crate::process::command("ps")
        .args(["-A", "-o", "comm="])
        .output();
    #[cfg(windows)]
    let output = crate::process::command("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-Process | Select-Object -ExpandProperty ProcessName",
        ])
        .output();
    #[cfg(not(any(unix, windows)))]
    return Err("Cannot verify stopped Java processes on this platform.".into());
    #[cfg(any(unix, windows))]
    {
        let output = output.map_err(|e| format!("Cannot verify stopped Java processes: {e}"))?;
        if !output.status.success() {
            return Err("Cannot enumerate Java processes safely.".into());
        }
        let names = std::str::from_utf8(&output.stdout).map_err(|e| e.to_string())?;
        if names.lines().any(|n| {
            let name = n
                .trim()
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            matches!(name.as_str(), "java" | "javaw" | "java.exe" | "javaw.exe")
        }) {
            return Err("A Java process is running. Quit Java games before adding a client to an existing instance.".into());
        }
        Ok(())
    }
}

// Installation never starts a child. Release only our own token, including error paths.
struct InstallMark(Option<crate::config_sync::RunMark>);
impl Drop for InstallMark {
    fn drop(&mut self) {
        if let Some(mark) = self.0.take() {
            if let Err(e) = mark.release() {
                eprintln!("Client install mark cleanup: {e}");
            }
        }
    }
}

fn check_java(
    jar: &ClientJar,
    game: &instances::JavaGame,
    versions: Option<&Versions>,
    fetch: Fetch,
) -> Result<(), String> {
    let Some(wants) = &jar.java_wants else {
        return Ok(());
    };
    let version = match &game.home {
        Some(home) => {
            let binary = home
                .join("bin")
                .join(if cfg!(windows) { "java.exe" } else { "java" });
            if !binary.is_file() {
                return Err("Selected Java home has no bin/java executable.".into());
            }
            crate::java::release_version(home)
                .ok_or("Cannot verify the selected Java runtime.")?
                .0
        }
        None => {
            let v = versions
                .ok_or("Java compatibility needs Mojang's version metadata.")?
                .find(&game.minecraft)?;
            let bytes = crate::sources::fetch_checked(fetch, &v.url, &v.sha1)?;
            let metadata: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            // Automatic selection fixes a major only; a later runtime may have any patch.
            if wants
                .split(|c: char| !(c.is_ascii_digit() || c == '.'))
                .any(|t| t.contains('.'))
            {
                return Err("Java patch constraints need an explicit Java home; automatic runtime patch is not known yet.".into());
            }
            crate::java::Need::of(&metadata).major.to_string()
        }
    };
    if range_allows(wants, &version) != Some(true) {
        return Err(format!(
            "Client needs Java {wants}; selected Java {version} is incompatible or unverified."
        ));
    }
    Ok(())
}

fn check_mod_dependencies(jar: &ClientJar, home: Option<&Path>) -> Result<(), String> {
    let mut available = Vec::new();
    if let Some(home) = home {
        let dir = home.join("mods");
        if fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink() || !m.is_dir()) {
            return Err("Cannot inspect dependencies: mods/ must be a real directory.".into());
        }
        if dir.exists() {
            for item in fs::read_dir(&dir).map_err(|e| e.to_string())? {
                let item = item.map_err(|e| e.to_string())?;
                if item.path().extension().and_then(|x| x.to_str()) == Some("jar") {
                    let info = inspect(&item.path())
                        .map_err(|e| format!("Cannot verify installed dependency: {e}"))?;
                    if info.mod_id == jar.mod_id {
                        return Err("A JAR with this mod id is already installed.".into());
                    }
                    if info.side == Side::Server
                        || info.minecraft_wants.is_some()
                        || info.loader_wants.is_some()
                        || info.java_wants.is_some()
                        || !info.mod_wants.is_empty()
                    {
                        return Err(format!("Dependency {} is server-only or has requirements needing full dependency resolution. Nothing was installed.", info.mod_id));
                    }
                    available.push(info);
                }
            }
        }
    }
    for (id, range) in &jar.mod_wants {
        if !available
            .iter()
            .any(|m| m.mod_id == *id && range_allows(range, &m.version) == Some(true))
        {
            return Err(format!("Client needs {id} {range}. Install its verified dependency into a Fabric instance first, then use --instance. Nothing was installed."));
        }
    }
    Ok(())
}

fn check_loader(jar: &ClientJar, version: &str) -> Result<(), String> {
    if let Some(wants) = &jar.loader_wants {
        if range_allows(wants, version) != Some(true) {
            return Err(format!("{} needs Fabric Loader {wants}; {version} is incompatible or unverified. Nothing was installed.", jar.name));
        }
    }
    Ok(())
}

/// Refuses incompatible and unsupported Minecraft ranges before installation.
fn check_minecraft(
    jar: &ClientJar,
    minecraft: &str,
    _notes: &mut Vec<String>,
) -> Result<(), String> {
    let Some(wants) = &jar.minecraft_wants else {
        return Ok(());
    };
    match range_allows(wants, minecraft) {
        Some(true) => Ok(()),
        Some(false) => Err(format!(
            "{} ({}) runs on Minecraft {wants}, not {minecraft}.",
            jar.name, jar.mod_id
        )),
        None => Err(format!(
            "Its Minecraft range {wants} is not supported here; compatibility with {minecraft} is unverified. Nothing was installed."
        )),
    }
}

/// The newest release the JAR allows, or a clear ask for --minecraft when that cannot be told.
pub fn pick_minecraft(jar: &ClientJar, versions: &Versions) -> Result<String, String> {
    let Some(wants) = &jar.minecraft_wants else {
        return Ok(versions.latest_release.clone());
    };
    for v in versions.of(false, false) {
        if range_allows(wants, &v.id) == Some(true) {
            return Ok(v.id.clone());
        }
    }
    Err(format!(
        "{} ({}) says Minecraft {wants}, and no release this sees matches: pick one with --minecraft.",
        jar.name, jar.mod_id
    ))
}

/// Where the JAR goes inside the instance, from its own file name. Only a plain file name
/// passes: no path parts, no `..`, `.jar` kept.
fn dest_file(jar: &Path) -> Result<String, String> {
    let name = jar
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("{} has no plain file name.", jar.display()))?;
    if name.len() > 200 || name.starts_with('.') || name.contains(['/', '\\']) {
        return Err(format!(
            "{} has a file name this will not write into mods/.",
            jar.display()
        ));
    }
    let dest = format!("mods/{name}");
    if !crate::browser::installed::in_folders(&dest) {
        return Err("Client JAR has a platform-unsafe file name.".into());
    }
    Ok(dest)
}

/// Test entry point for the bounded snapshot writer.
#[cfg(test)]
fn copy_in(home: &Path, jar: &Path, dest: &str) -> Result<(), String> {
    copy_bytes(home, &read_jar(jar)?, dest)
}

fn copy_bytes(home: &Path, bytes: &[u8], dest: &str) -> Result<(), String> {
    use std::io::Write;
    if !crate::browser::installed::in_folders(dest) || !dest.starts_with("mods/") {
        return Err("Unsafe client JAR destination.".into());
    }
    let dir = home.join("mods");
    if fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink() || !m.is_dir()) {
        return Err("mods/ must be a real directory, not a file or symlink.".into());
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let real_home = home.canonicalize().map_err(|e| e.to_string())?;
    if !dir
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(&real_home)
    {
        return Err("mods/ resolves outside the instance.".into());
    }
    let target = home.join(dest);
    if fs::symlink_metadata(&target).is_ok() {
        return Err(format!("{} is already there.", target.display()));
    }
    let temp = target.with_file_name(format!(
        "{}.horizon-part",
        target.file_name().unwrap().to_string_lossy()
    ));
    // Exclusive creation: never follow a planted staging link, truncate another install,
    // or delete a file we did not create.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| format!("Could not stage client JAR: {e}"))?;
    let result = (|| {
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        // A hard link publishes the complete file without replacing an existing target,
        // including one another installer created after our earlier check.
        fs::hard_link(&temp, &target).map_err(|e| e.to_string())
    })();
    let cleanup = fs::remove_file(&temp);
    match (result, cleanup) {
        (Err(e), _) => Err(e),
        (Ok(()), Err(e)) => Err(format!(
            "JAR was installed, but staging cleanup failed: {e}"
        )),
        (Ok(()), Ok(())) => Ok(()),
    }
}

#[cfg(test)]
#[path = "client_jar/tests.rs"]
mod tests;
