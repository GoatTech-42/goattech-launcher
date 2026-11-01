//! Mod loaders for Java instances: Fabric and Quilt serve launcher-ready profiles from their
//! metadata services; Forge and NeoForge ship an installer jar that builds the profile instead
//! (`crate::forge_install`). For each Minecraft version, a loader's
//! metadata service lists its versions and serves a launcher profile: a version JSON that
//! `inheritsFrom` the game's own, with the loader's main class and its libraries by Maven name and
//! repository. The profile is kept in `versions/` beside the game's, as the official launcher keeps
//! a loader's, and its libraries in `libraries/`, each checked against its sha1 (the profile's,
//! else the one the repository publishes beside the file). `java_launch::resolve` merges the
//! profile into the game's version.

use crate::sources::{fetch_checked, sha1_file, write_file, Fetch};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

/// A mod loader.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

impl Kind {
    pub fn parse(name: &str) -> Result<Kind, String> {
        match name.trim().to_ascii_lowercase().as_str() {
            "fabric" => Ok(Kind::Fabric),
            "quilt" => Ok(Kind::Quilt),
            "forge" => Ok(Kind::Forge),
            "neoforge" => Ok(Kind::NeoForge),
            _ => Err(format!("No mod loader {name:?}: fabric, quilt, forge or neoforge.")),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Fabric => "Fabric",
            Kind::Quilt => "Quilt",
            Kind::Forge => "Forge",
            Kind::NeoForge => "NeoForge",
        }
    }

    /// Whether the launcher installs it today: Fabric and Quilt. Forge and NeoForge's
    /// installer driver (`crate::forge_install`) is verified before this flips
    /// (goattech/FORGE-NEOFORGE.md); until then imports keep their explicit block.
    pub fn installable(self) -> bool {
        matches!(self, Kind::Fabric | Kind::Quilt)
    }

    /// As Modrinth and the instance's settings name it: `fabric`, `quilt`.
    pub fn id(self) -> &'static str {
        match self {
            Kind::Fabric => "fabric",
            Kind::Quilt => "quilt",
            Kind::Forge => "forge",
            Kind::NeoForge => "neoforge",
        }
    }

    /// Its metadata service (Fabric, Quilt: launcher-ready profiles); none for a loader whose
    /// profile its installer builds.
    fn meta(self) -> Option<&'static str> {
        match self {
            Kind::Fabric => Some("https://meta.fabricmc.net/v2"),
            Kind::Quilt => Some("https://meta.quiltmc.org/v3"),
            _ => None,
        }
    }

    /// Its Maven repository and `group/artifact` path (Forge, NeoForge: installer jars).
    pub(crate) fn maven(self) -> Option<(&'static str, &'static str)> {
        match self {
            Kind::Forge => Some(("https://maven.minecraftforge.net", "net/minecraftforge/forge")),
            Kind::NeoForge => Some(("https://maven.neoforged.net/releases", "net/neoforged/neoforge")),
            _ => None,
        }
    }

    /// The id its profile has: `fabric-loader-0.16.9-1.21.4`, or for a Forge-family loader the
    /// id its installer gives the version JSON (`1.21.1-forge-52.1.20`, `neoforge-21.1.265`;
    /// checked against the real installer jars, 2026-10-31).
    pub fn profile_id(self, version: &str, minecraft: &str) -> String {
        match self {
            Kind::Forge => version.replacen('-', "-forge-", 1),
            Kind::NeoForge => format!("neoforge-{version}"),
            _ => format!(
                "{}-loader-{version}-{minecraft}",
                self.name().to_lowercase()
            ),
        }
    }
}

/// The loader a Java instance plays with, at one version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loader {
    pub kind: Kind,
    pub version: String,
}

/// One of a loader's versions for a Minecraft version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Listed {
    pub version: String,
    pub stable: bool,
}

/// How versions sort: by their numbers, a release above its betas (`0.20.0` > `0.20.0-beta.9`),
/// betas by their numbers too (`beta.10` > `beta.9`).
fn order(version: &str) -> (Vec<u64>, bool, Vec<(u64, String)>) {
    let (core, pre) = match version.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (version.split('+').next().unwrap_or(version), None),
    };
    let numbers = core.split('.').map(|n| n.parse().unwrap_or(0)).collect();
    let pre: Vec<(u64, String)> = pre
        .into_iter()
        .flat_map(|p| p.split(['.', '-', '+']))
        .map(|part| (part.parse().unwrap_or(0), part.to_string()))
        .collect();
    (numbers, pre.is_empty(), pre)
}

/// A service's list, `[{"loader": {"version", "stable"}}]`, the newest first (Quilt's comes in no
/// order). Quilt's has no `stable`: its betas carry a `-` (`0.20.0-beta.9`).
pub fn parse_versions(text: &str) -> Result<Vec<Listed>, String> {
    let list: Value =
        serde_json::from_str(text).map_err(|e| format!("The loader's version list: {e}"))?;
    let list = list
        .as_array()
        .ok_or("The loader's version list is not a list")?;
    let mut listed: Vec<Listed> = list
        .iter()
        .filter_map(|entry| {
            let loader = &entry["loader"];
            let version = loader["version"].as_str()?.to_string();
            let stable = loader["stable"].as_bool().unwrap_or(!version.contains('-'));
            Some(Listed { version, stable })
        })
        .collect();
    listed.sort_by_cached_key(|l| std::cmp::Reverse(order(&l.version)));
    Ok(listed)
}

/// `kind`'s versions for `minecraft`.
pub fn versions(kind: Kind, minecraft: &str, fetch: Fetch) -> Result<Vec<Listed>, String> {
    if kind.maven().is_some() {
        let listed = maven_versions(kind, minecraft, &fetch)?;
        if listed.is_empty() {
            return Err(format!(
                "{} has no loader for Minecraft {minecraft}.",
                kind.name()
            ));
        }
        return Ok(listed);
    }
    let meta = kind
        .meta()
        .ok_or_else(|| format!("{} has no version service", kind.name()))?;
    let body = fetch(&format!("{meta}/versions/loader/{minecraft}"))?;
    let listed = parse_versions(&String::from_utf8_lossy(&body))?;
    if listed.is_empty() {
        return Err(format!(
            "{} has no loader for Minecraft {minecraft}.",
            kind.name()
        ));
    }
    Ok(listed)
}

/// The loader `kind` names at `version` (none or `latest`: the newest stable one) for
/// `minecraft`, checked against its service's list.
pub fn choose(kind: &str, minecraft: &str, version: Option<&str>) -> Result<Loader, String> {
    let kind = Kind::parse(kind)?;
    let listed = versions(kind, minecraft, &crate::backend::get_bytes)?;
    Ok(Loader {
        kind,
        version: pick(kind, &listed, version)?,
    })
}

/// `wanted` among them: a version, or none or `latest` for the newest stable one.
pub fn pick(kind: Kind, listed: &[Listed], wanted: Option<&str>) -> Result<String, String> {
    let found = match wanted.map(str::trim) {
        None | Some("latest") => listed.iter().find(|l| l.stable).or(listed.first()),
        Some(version) => listed.iter().find(|l| l.version == version),
    };
    found.map(|l| l.version.clone()).ok_or_else(|| {
        format!(
            "{} has no loader {} for this Minecraft version.",
            kind.name(),
            wanted.unwrap_or("latest")
        )
    })
}

/// A library's file in a Maven repository: `group:artifact:version[:classifier][@extension]`.
pub fn maven_path(name: &str) -> Result<String, String> {
    let (coordinates, extension) = name.split_once('@').unwrap_or((name, "jar"));
    let parts: Vec<&str> = coordinates.split(':').collect();
    let (group, artifact, version, classifier) = match parts[..] {
        [g, a, v] => (g, a, v, None),
        [g, a, v, c] => (g, a, v, Some(c)),
        _ => return Err(format!("Bad library name {name:?}")),
    };
    let file = match classifier {
        Some(c) => format!("{artifact}-{version}-{c}.{extension}"),
        None => format!("{artifact}-{version}.{extension}"),
    };
    Ok(format!(
        "{}/{artifact}/{version}/{file}",
        group.replace('.', "/")
    ))
}

/// Downloads `loader`'s profile for `minecraft` and its libraries into the game folder `dir`, and
/// keeps the profile in `versions/` with each library's file named the way Mojang's are
/// (`downloads.artifact`). Returns the profile's id.
pub fn install(
    loader: &Loader,
    minecraft: &str,
    dir: &Path,
    fetch: Fetch,
    log: &dyn Fn(String),
) -> Result<String, String> {
    if loader.kind.maven().is_some() {
        return crate::forge_install::install(loader, minecraft, dir, fetch, log);
    }
    let name = loader.kind.name();
    let url = format!(
        "{}/versions/loader/{minecraft}/{}/profile/json",
        loader.kind.meta().ok_or("A profile-serving loader")?,
        loader.version
    );
    let mut profile: Value =
        serde_json::from_slice(&fetch(&url)?).map_err(|e| format!("{name}'s profile: {e}"))?;
    if profile["inheritsFrom"] != minecraft {
        return Err(format!(
            "{name}'s profile is for {}, not Minecraft {minecraft}",
            profile["inheritsFrom"]
        ));
    }
    let id = profile["id"]
        .as_str()
        .ok_or_else(|| format!("{name}'s profile has no id"))?
        .to_string();
    let libraries = profile["libraries"]
        .as_array_mut()
        .ok_or_else(|| format!("{name}'s profile has no libraries"))?;
    log(format!(
        "Downloading {name} {} ({} libraries)",
        loader.version,
        libraries.len()
    ));
    for library in libraries.iter_mut() {
        let path = maven_path(library["name"].as_str().ok_or("A library has no name")?)?;
        let repository = library["url"]
            .as_str()
            .unwrap_or("https://repo1.maven.org/maven2/");
        let url = format!("{}/{path}", repository.trim_end_matches('/'));
        let sha1 = match library["sha1"].as_str() {
            Some(sha1) => sha1.to_string(),
            None => String::from_utf8_lossy(&fetch(&format!("{url}.sha1"))?)
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string(),
        };
        let file = dir.join("libraries").join(crate::mojang::relative(&path)?);
        if sha1_file(&file).as_ref() != Some(&sha1) {
            let body = fetch_checked(fetch, &url, &sha1).map_err(|e| format!("{url} {e}"))?;
            write_file(&file, &body)?;
        }
        library["downloads"] = json!({"artifact": {"path": path, "url": url, "sha1": sha1}});
    }
    let out = dir
        .join("versions")
        .join(crate::mojang::relative(&id)?)
        .join(format!("{id}.json"));
    write_file(
        &out,
        &serde_json::to_vec_pretty(&profile).map_err(|e| e.to_string())?,
    )?;
    Ok(id)
}

/// The prefix NeoForge versions carry for a Minecraft version: `1.21.1` plays NeoForge
/// `21.1.*` (the Minecraft version without its leading `1.` and patch); a year-named Minecraft
/// (`26.3`) plays `26.3.*`.
fn neoforge_prefix(minecraft: &str) -> Result<String, String> {
    let stripped = minecraft.strip_prefix("1.").unwrap_or(minecraft);
    let mut parts = stripped.split('.');
    match (parts.next(), parts.next()) {
        (Some(a), Some(b)) if a.parse::<u64>().is_ok() && b.parse::<u64>().is_ok() => {
            Ok(format!("{a}.{b}."))
        }
        _ => Err(format!(
            "Which NeoForge versions Minecraft {minecraft} has is unknown"
        )),
    }
}

/// A Forge-family loader's versions for `minecraft`, from its Maven metadata: the `<version>`
/// entries under this Minecraft version's prefix, newest first. Forge names them
/// `<minecraft>-<build>` (`1.21.1-52.1.20`, a tagged build like `-prune` is not stable);
/// NeoForge `<prefix><build>` with betas carrying a `-` (`21.1.94-beta`).
pub fn parse_maven_versions(kind: Kind, text: &str, minecraft: &str) -> Result<Vec<Listed>, String> {
    let prefix = match kind {
        Kind::Forge => format!("{minecraft}-"),
        Kind::NeoForge => neoforge_prefix(minecraft)?,
        _ => return Err(format!("{} versions come from its service", kind.name())),
    };
    let mut listed: Vec<Listed> = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("<version>") {
        rest = &rest[start + "<version>".len()..];
        let Some(end) = rest.find("</version>") else {
            break;
        };
        let version = rest[..end].trim();
        rest = &rest[end + "</version>".len()..];
        let Some(build) = version.strip_prefix(&prefix) else {
            continue;
        };
        let stable = match kind {
            Kind::Forge => build.split('.').all(|p| p.parse::<u64>().is_ok()),
            _ => !build.contains('-'),
        };
        listed.push(Listed {
            version: version.to_string(),
            stable,
        });
    }
    listed.sort_by_cached_key(|l| std::cmp::Reverse(order(&l.version)));
    Ok(listed)
}

/// `kind`'s Forge-family versions for `minecraft`, fetched from its Maven metadata.
fn maven_versions(kind: Kind, minecraft: &str, fetch: &Fetch) -> Result<Vec<Listed>, String> {
    let (base, path) = kind
        .maven()
        .ok_or_else(|| format!("{} has no Maven repository", kind.name()))?;
    let body = fetch(&format!("{base}/{path}/maven-metadata.xml"))?;
    parse_maven_versions(kind, &String::from_utf8_lossy(&body), minecraft)
}

#[cfg(test)]
mod maven_tests {
    use super::*;

    const FORGE_META: &str = r#"<?xml version="1.0"?>
<metadata><groupId>net.minecraftforge</groupId><artifactId>forge</artifactId>
<versioning><latest>26.3-66.1.2</latest><release>26.3-66.1.2</release>
<versions>
<version>1.21.1-52.1.18</version>
<version>1.21.1-52.1.20</version>
<version>1.21.1-52.1.19</version>
<version>1.21.1-52.0.0-prune</version>
<version>1.21.4-54.0.6</version>
</versions></versioning></metadata>"#;

    const NEO_META: &str = r#"<?xml version="1.0"?>
<metadata><groupId>net.neoforged</groupId><artifactId>neoforge</artifactId>
<versioning><latest>26.3.0.100</latest>
<versions>
<version>21.1.263</version>
<version>21.1.265</version>
<version>21.1.264</version>
<version>21.1.300-beta</version>
<version>21.4.1</version>
</versions></versioning></metadata>"#;

    #[test]
    fn forge_versions_for_one_minecraft() {
        let listed = parse_maven_versions(Kind::Forge, FORGE_META, "1.21.1").unwrap();
        let versions: Vec<&str> = listed.iter().map(|l| l.version.as_str()).collect();
        assert_eq!(
            versions,
            vec!["1.21.1-52.1.20", "1.21.1-52.1.19", "1.21.1-52.1.18", "1.21.1-52.0.0-prune"]
        );
        assert!(listed[0].stable && listed[1].stable && listed[2].stable);
        assert!(!listed[3].stable, "a tagged build is not stable");
    }

    #[test]
    fn neoforge_versions_drop_the_leading_one() {
        let listed = parse_maven_versions(Kind::NeoForge, NEO_META, "1.21.1").unwrap();
        let versions: Vec<&str> = listed.iter().map(|l| l.version.as_str()).collect();
        assert_eq!(versions, vec!["21.1.300-beta", "21.1.265", "21.1.264", "21.1.263"]);
        assert!(listed[1].stable && !listed[0].stable);
        // The newest stable is what `latest` picks.
        assert_eq!(pick(Kind::NeoForge, &listed, None).unwrap(), "21.1.265");
    }

    #[test]
    fn neoforge_prefix_for_year_named_minecraft() {
        assert_eq!(neoforge_prefix("26.3").unwrap(), "26.3.");
        assert!(neoforge_prefix("wurst").is_err());
    }
}

#[cfg(test)]
#[path = "loaders/tests.rs"]
mod tests;
