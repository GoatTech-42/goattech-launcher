//! Forge and NeoForge installs. Unlike Fabric and Quilt, their launcher profile is not served
//! ready-made: the loader's installer jar builds it. The jar's `install_profile.json` names a
//! version JSON, libraries (piston-meta shape) and JVM processors that download and patch the
//! game, so the launcher drives the official installer headless (`--install-client`) in a
//! scratch folder and keeps the profile and libraries it produced (goattech/FORGE-NEOFORGE.md).
//! One install per (loader, version): `backend::java_game` marks it `.downloaded`, and the next
//! start is fully offline, as a Fabric or Quilt install is.

use crate::loaders::{maven_path, Loader};
use crate::sources::{fetch_checked, write_file, Fetch};
use serde_json::{json, Value};
use std::fs;
use std::io::Read;
use std::path::Path;

/// `loader`'s profile for `minecraft`, built by its official installer into the game folder
/// `dir`. Returns the profile's id (the id `launch` starts).
pub fn install(
    loader: &Loader,
    minecraft: &str,
    dir: &Path,
    fetch: Fetch,
    log: &dyn Fn(String),
) -> Result<String, String> {
    let name = loader.kind.name();
    let (base, coordinates) = loader
        .kind
        .maven()
        .ok_or_else(|| format!("{name} has no installer"))?;
    // The installer's `install_profile.json` layout exists from Minecraft 1.13 on; older Forge
    // installers predate it and would only fail deep in the zip read (goattech/FORGE-NEOFORGE.md).
    // Name the cutoff up front instead. Unknown version shapes (snapshots) pass through to the
    // installer's own Minecraft check below.
    if matches!(minecraft_minor(minecraft), Some(minor) if minor < 13) {
        return Err(format!(
            "{name} needs Minecraft 1.13 or newer: older installers predate the layout this install drives"
        ));
    }
    let artifact = coordinates.rsplit('/').next().unwrap_or(coordinates);
    let file_name = format!("{artifact}-{}-installer.jar", loader.version);
    let url = format!("{base}/{coordinates}/{}/{file_name}", loader.version);
    let jar = dir
        .join("installers")
        .join(crate::mojang::relative(&loader.version)?)
        .join(&file_name);
    if !jar.is_file() {
        log(format!("Downloading {name} {}", loader.version));
        let sha1 = String::from_utf8_lossy(&fetch(&format!("{url}.sha1"))?)
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string();
        let body = fetch_checked(fetch, &url, &sha1).map_err(|e| format!("{url} {e}"))?;
        write_file(&jar, &body)?;
    }
    let (id, profile) = install_profile(&jar)?;
    match profile["minecraft"].as_str() {
        Some(for_mc) if for_mc == minecraft => {}
        Some(for_mc) => {
            return Err(format!(
                "{name} {} is for Minecraft {for_mc}, not {minecraft}",
                loader.version
            ))
        }
        None => return Err(format!("{name}'s installer names no Minecraft version")),
    }

    // The installer wants a launcher-like folder: launcher_profiles.json, and the vanilla jar
    // and version file where a launcher keeps them (it downloads what it cannot find).
    let scratch = dir
        .join("installer-work")
        .join(crate::mojang::relative(&id)?);
    if scratch.exists() {
        fs::remove_dir_all(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
    }
    fs::create_dir_all(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
    let ok = run_installer(&jar, loader.kind, &scratch, &id, name, minecraft, dir, log);
    if let Err(e) = ok {
        let _ = fs::remove_dir_all(&scratch);
        return Err(e);
    }

    // Keep what it made: the profile (with its libraries given their on-disk paths, as Mojang's
    // own version files carry them) and every library file into the shared `libraries/`.
    let produced = scratch
        .join("versions")
        .join(crate::mojang::relative(&id)?)
        .join(format!("{id}.json"));
    let mut version: Value = serde_json::from_slice(
        &fs::read(&produced).map_err(|e| format!("{}: {e}", produced.display()))?,
    )
    .map_err(|e| format!("{name}'s installed profile: {e}"))?;
    let mut kept = 0usize;
    keep_libraries(&scratch.join("libraries"), &dir.join("libraries"), &mut kept)?;
    if let Some(libraries) = version["libraries"].as_array_mut() {
        for library in libraries.iter_mut() {
            if library["downloads"]["artifact"]["path"].is_string() {
                continue;
            }
            if let Some(lib_name) = library["name"].as_str() {
                if let Ok(path) = maven_path(lib_name) {
                    library["downloads"] = json!({"artifact": {"path": path}});
                }
            }
        }
    }
    let out = dir
        .join("versions")
        .join(crate::mojang::relative(&id)?)
        .join(format!("{id}.json"));
    write_file(
        &out,
        &serde_json::to_vec_pretty(&version).map_err(|e| e.to_string())?,
    )?;
    let _ = fs::remove_dir_all(&scratch);
    log(format!("Installed {name} {} ({kept} libraries)", loader.version));
    Ok(id)
}

/// The minor in `1.<minor>...`, when `minecraft` is a release-style version id.
fn minecraft_minor(minecraft: &str) -> Option<u64> {
    let mut parts = minecraft.split('.');
    if parts.next()? != "1" {
        return None;
    }
    parts.next()?.parse().ok()
}

/// The installer jar's `install_profile.json`: its profile's id and the whole profile.
fn install_profile(jar: &Path) -> Result<(String, Value), String> {
    let file = fs::File::open(jar).map_err(|e| format!("{}: {e}", jar.display()))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", jar.display()))?;
    let mut text = String::new();
    zip.by_name("install_profile.json")
        .map_err(|e| format!("{}: install_profile.json: {e}", jar.display()))?
        .read_to_string(&mut text)
        .map_err(|e| format!("{}: install_profile.json: {e}", jar.display()))?;
    let profile: Value = serde_json::from_str(&text)
        .map_err(|e| format!("{}: install_profile.json: {e}", jar.display()))?;
    let id = profile["version"]
        .as_str()
        .ok_or("the installer's profile names no version")?
        .to_string();
    Ok((id, profile))
}

/// Runs the installer headless in `scratch`: `java -jar installer.jar --install-client .`
fn run_installer(
    jar: &Path,
    kind: crate::loaders::Kind,
    scratch: &Path,
    id: &str,
    name: &str,
    minecraft: &str,
    dir: &Path,
    log: &dyn Fn(String),
) -> Result<(), String> {
    write_file(
        &scratch.join("launcher_profiles.json"),
        b"{\"profiles\":{}}",
    )?;
    let vanilla_dir = scratch
        .join("versions")
        .join(crate::mojang::relative(minecraft)?);
    let have = dir
        .join("versions")
        .join(crate::mojang::relative(minecraft)?);
    for suffix in ["json", "jar"] {
        let from = have.join(format!("{minecraft}.{suffix}"));
        if from.is_file() {
            fs::create_dir_all(&vanilla_dir)
                .map_err(|e| format!("{}: {e}", vanilla_dir.display()))?;
            fs::copy(&from, vanilla_dir.join(format!("{minecraft}.{suffix}")))
                .map_err(|e| format!("{}: {e}", from.display()))?;
        }
    }
    let version: Value = serde_json::from_slice(
        &fs::read(dir.join("versions").join(crate::mojang::relative(minecraft)?).join(format!(
            "{minecraft}.json"
        )))
        .map_err(|e| format!("Minecraft {minecraft} is not downloaded yet ({e})"))?,
    )
    .map_err(|e| e.to_string())?;
    let need = crate::java::Need::of(&version);
    let found = crate::java::pick(
        &need,
        None,
        &crate::java::mojang_runtimes(&crate::java::runtimes_dir()),
        &crate::java::installed(),
    )?;
    let runtime = match found {
        Some(runtime) => runtime,
        None => {
            log(format!("Downloading Java {} from Mojang", need.major));
            crate::java::install(&need, log)?
        }
    };
    log(format!("Running {name}'s installer (this downloads and patches the game)"));
    // Each installer's own spelling: NeoForge's jopt-simple takes the dashed form, Forge's
    // SimpleInstaller only the camelCase one (live-verified against both installers).
    let mode = match kind {
        crate::loaders::Kind::Forge => "--installClient",
        _ => "--install-client",
    };
    let out = std::process::Command::new(runtime.java())
        .arg("-jar")
        .arg(jar)
        .arg(mode)
        .arg(".")
        .current_dir(scratch)
        .output()
        .map_err(|e| format!("{}: {e}", runtime.java().display()))?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines().chain(stderr.lines()).take(12) {
        log(line.to_string());
    }
    if !out.status.success() {
        let tail: Vec<&str> = stderr.lines().collect();
        return Err(format!(
            "{name}'s installer failed ({}): {}",
            out.status,
            tail.last().unwrap_or(&"no error output")
        ));
    }
    let produced = scratch
        .join("versions")
        .join(crate::mojang::relative(id)?)
        .join(format!("{id}.json"));
    if !produced.is_file() {
        return Err(format!(
            "{name}'s installer finished without the profile {}",
            produced.display()
        ));
    }
    Ok(())
}

/// Every file under `from` kept under `to` (already-there files stay as they are).
fn keep_libraries(from: &Path, to: &Path, kept: &mut usize) -> Result<(), String> {
    if !from.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let (src, dest) = (entry.path(), to.join(entry.file_name()));
        if src.is_dir() {
            keep_libraries(&src, &dest, kept)?;
        } else if !dest.exists() {
            fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
            fs::copy(&src, &dest).map_err(|e| format!("{}: {e}", src.display()))?;
            *kept += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn profile_ids_match_the_installers() {
        // From the real installer jars (2026-10-31): install_profile.json's `version`.
        use crate::loaders::Kind;
        assert_eq!(
            Kind::Forge.profile_id("1.21.1-52.1.20", "1.21.1"),
            "1.21.1-forge-52.1.20"
        );
        assert_eq!(
            Kind::NeoForge.profile_id("21.1.265", "1.21.1"),
            "neoforge-21.1.265"
        );
    }

    #[test]
    fn release_versions_give_their_minor() {
        use super::minecraft_minor;
        assert_eq!(minecraft_minor("1.12.2"), Some(12));
        assert_eq!(minecraft_minor("1.13"), Some(13));
        assert_eq!(minecraft_minor("1.20.1"), Some(20));
        assert_eq!(minecraft_minor("24w14a"), None);
    }

    #[test]
    fn pre_1_13_is_named_before_any_download() {
        use super::{install, minecraft_minor};
        use crate::loaders::{Kind, Loader};
        use std::path::Path;
        let loader = Loader {
            kind: Kind::Forge,
            version: "1.12.2-14.23.5.2859".to_string(),
        };
        let fetch: &(dyn Fn(&str) -> Result<Vec<u8>, String> + Sync) =
            &|_| panic!("the cutoff answers before any network");
        let log = |_line: String| {};
        let error = install(&loader, "1.12.2", Path::new("/nonexistent"), fetch, &log).unwrap_err();
        assert!(error.contains("1.13"), "{error}");
        assert_eq!(minecraft_minor("1.12.2"), Some(12));
    }
}
