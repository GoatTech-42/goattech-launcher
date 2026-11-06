//! Synthetic JARs only, built in the test: no real client bytes, no network (the fetch is a
//! map that fails every unknown URL).
use super::*;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn jar(dir: &Path, name: &str, files: &[(&str, &str)]) -> PathBuf {
    let path = dir.join(name);
    let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    for (entry, body) in files {
        zip.start_file(entry, options).unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
    path
}

const DAWN_LIKE: &str = r#"{
    "schemaVersion": 1,
    "id": "dawn",
    "version": "1.2.3",
    "name": "Dawn",
    "license": ["ARR"],
    "environment": "client",
    "entrypoints": {"main": ["com.example.dawn.Dawn"]},
    "depends": {
        "fabricloader": ">=0.16.0",
        "minecraft": "~1.21",
        "java": ">=21",
        "fabric-api": "*"
    }
}"#;

#[test]
fn a_full_manifest_reads() {
    let dir = std::env::temp_dir().join(format!("cj-manifest-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = jar(
        &dir,
        "dawn.jar",
        &[("fabric.mod.json", DAWN_LIKE), ("icon.png", "png")],
    );
    let jar = inspect(&path).unwrap();
    assert_eq!(jar.mod_id, "dawn");
    assert_eq!(jar.name, "Dawn");
    assert_eq!(jar.version, "1.2.3");
    assert_eq!(jar.license.as_deref(), Some("ARR"));
    assert_eq!(jar.side, Side::Client);
    assert!(jar.entrypoints);
    assert_eq!(jar.loader_wants.as_deref(), Some(">=0.16.0"));
    assert_eq!(jar.minecraft_wants.as_deref(), Some("~1.21"));
    assert_eq!(jar.java_wants.as_deref(), Some(">=21"));
    assert_eq!(
        jar.mod_wants,
        vec![("fabric-api".to_string(), "*".to_string())]
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn contract_violations_refuse() {
    let dir = std::env::temp_dir().join(format!("cj-bad-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let cases: &[(&str, &str, &str)] = &[
        (
            "no-schema.jar",
            r#"{"id":"dawn","version":"1"}"#,
            "schemaVersion",
        ),
        (
            "schema-two.jar",
            r#"{"schemaVersion":2,"id":"dawn","version":"1"}"#,
            "schemaVersion 2",
        ),
        ("no-id.jar", r#"{"schemaVersion":1,"version":"1"}"#, "no id"),
        (
            "bad-id.jar",
            r#"{"schemaVersion":1,"id":"Dawn!","version":"1"}"#,
            "not a Fabric mod id",
        ),
        (
            "no-version.jar",
            r#"{"schemaVersion":1,"id":"dawn"}"#,
            "no version",
        ),
        (
            "bad-env.jar",
            r#"{"schemaVersion":1,"id":"dawn","version":"1","environment":"everywhere"}"#,
            "environment",
        ),
        ("not-json.jar", "hello", "not JSON"),
    ];
    for (name, body, want) in cases {
        let path = jar(&dir, name, &[("fabric.mod.json", body)]);
        let err = inspect(&path).unwrap_err();
        assert!(err.contains(want), "{name}: {err} lacks {want:?}");
    }
    // A Quilt-only JAR and a plain library are named for what they are.
    let quilt = jar(&dir, "quilt.jar", &[("quilt.mod.json", "{}")]);
    assert!(inspect(&quilt).unwrap_err().contains("Quilt mod"));
    let plain = jar(&dir, "lib.jar", &[("a.class", "ca")]);
    assert!(inspect(&plain).unwrap_err().contains("no mod metadata"));
    // Not a zip at all, and not a jar by name.
    let junk = dir.join("junk.jar");
    fs::write(&junk, b"not a zip").unwrap();
    assert!(inspect(&junk).unwrap_err().contains("not a readable JAR"));
    let txt = dir.join("dawn.txt");
    fs::write(&txt, b"").unwrap();
    assert!(inspect(&txt).unwrap_err().contains("not a .jar"));
    // A manifest past the cap, and an entry bomb, refuse before reading much.
    let big = format!(
        r#"{{"schemaVersion":1,"id":"dawn","version":"1","description":"{}"}}"#,
        "x".repeat((MAX_META_BYTES + 8) as usize)
    );
    let path = jar(&dir, "big.jar", &[("fabric.mod.json", &big)]);
    assert!(inspect(&path).unwrap_err().contains("1 MiB"));
    let bomb = dir.join("bomb.jar");
    let mut zip = zip::ZipWriter::new(fs::File::create(&bomb).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    for n in 0..MAX_ENTRIES + 1 {
        zip.start_file(format!("f{n}"), options).unwrap();
    }
    zip.finish().unwrap();
    assert!(inspect(&bomb).unwrap_err().contains("entries"));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn ranges_match_their_forms() {
    let yes = Some(true);
    let no = Some(false);
    assert_eq!(range_allows("*", "1.20.1"), yes);
    assert_eq!(range_allows("1.20.1", "1.20.1"), yes);
    assert_eq!(range_allows("1.20.1", "1.20.2"), no);
    assert_eq!(range_allows("1.20.x", "1.20.4"), yes);
    assert_eq!(range_allows("1.20.x", "1.21"), no);
    assert_eq!(range_allows("~1.20", "1.20"), yes);
    assert_eq!(range_allows("~1.20", "1.20.6"), yes);
    assert_eq!(range_allows("~1.20", "1.21"), no);
    assert_eq!(range_allows("~1.20", "1.19.4"), no);
    assert_eq!(range_allows(">=1.17", "1.20.1"), yes);
    assert_eq!(range_allows(">=1.17", "1.17"), yes);
    assert_eq!(range_allows(">=1.17", "1.16.5"), no);
    assert_eq!(range_allows(">1.17", "1.17"), no);
    assert_eq!(range_allows("<1.21", "1.20.4"), yes);
    assert_eq!(range_allows("<1.21", "1.21"), no);
    assert_eq!(range_allows(">=1.17 <1.21", "1.18.2"), yes);
    assert_eq!(range_allows(">=1.17 <1.21", "1.21"), no);
    // Alternatives (a JSON array in depends) OR.
    assert_eq!(range_allows("1.20.1;1.20.4", "1.20.4"), yes);
    assert_eq!(range_allows("1.20.1;1.20.4", "1.20.2"), no);
    // Snapshots and odd names claim nothing, either side.
    assert_eq!(range_allows(">=1.17", "24w14a"), None);
    assert_eq!(range_allows("24w14a", "1.20"), None);
}

fn versions_fixture() -> Versions {
    Versions::parse(
        r#"{
        "latest": {"release": "1.21.4", "snapshot": "25w01a"},
        "versions": [
            {"id": "25w01a", "type": "snapshot", "url": "u", "sha1": "s", "releaseTime": "t"},
            {"id": "1.21.4", "type": "release", "url": "u", "sha1": "s", "releaseTime": "t"},
            {"id": "1.21.1", "type": "release", "url": "u", "sha1": "s", "releaseTime": "t"},
            {"id": "1.20.6", "type": "release", "url": "u", "sha1": "s", "releaseTime": "t"},
            {"id": "1.20.1", "type": "release", "url": "u", "sha1": "s", "releaseTime": "t"}
        ]
    }"#,
    )
    .unwrap()
}

#[test]
fn the_newest_allowed_release_is_picked() {
    let versions = versions_fixture();
    let mut jar = inspect(&jar(
        &std::env::temp_dir(),
        &format!("dawn-pick-{}.jar", std::process::id()),
        &[("fabric.mod.json", DAWN_LIKE)],
    ))
    .unwrap();
    assert_eq!(pick_minecraft(&jar, &versions).unwrap(), "1.21.4");
    jar.minecraft_wants = Some("~1.20".to_string());
    assert_eq!(pick_minecraft(&jar, &versions).unwrap(), "1.20.6");
    jar.minecraft_wants = None;
    assert_eq!(pick_minecraft(&jar, &versions).unwrap(), "1.21.4");
    jar.minecraft_wants = Some("~1.19".to_string());
    assert!(pick_minecraft(&jar, &versions)
        .unwrap_err()
        .contains("--minecraft"));
}

/// The loader list Fabric's meta serves, without the network.
fn fake_fetch(url: &str) -> Result<Vec<u8>, String> {
    if url == "https://meta.fabricmc.net/v2/versions/loader/1.21.4"
        || url == "https://meta.fabricmc.net/v2/versions/loader/1.20.6"
    {
        Ok(
            br#"[{"loader": {"version": "0.17.0-beta.1", "stable": false}},
                 {"loader": {"version": "0.16.9", "stable": true}}]"#
                .to_vec(),
        )
    } else {
        Err(format!("404 {url}"))
    }
}

fn scratch(tag: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("cj-{tag}-{}", std::process::id()));
    let home = root.join("game");
    let data = root.join("data");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&data).unwrap();
    (home, data)
}

fn dawn_jar(dir: &Path) -> PathBuf {
    jar(
        dir,
        "dawn.jar",
        &[
            (
                "fabric.mod.json",
                r#"{"schemaVersion":1,"id":"dawn","name":"Dawn","version":"1.2.3","depends":{"fabricloader":">=0.16.0","minecraft":"~1.21"}}"#,
            ),
            ("x.class", "bytes"),
        ],
    )
}

#[test]
fn a_new_instance_takes_the_jar_whole() {
    let (home, data) = scratch("new");
    let jar_path = dawn_jar(&home);
    let versions = versions_fixture();
    let out = install(
        &home,
        &data,
        Some(&versions),
        &Goal {
            jar: &jar_path,
            instance: None,
            name: None,
            minecraft: None,
            loader_version: None,
        },
        &fake_fetch,
    )
    .unwrap();
    assert!(out.created);
    assert_eq!(out.instance, "Dawn");
    assert_eq!(out.minecraft, "1.21.4");
    assert_eq!(out.loader, "fabric 0.16.9");
    assert_eq!(out.file, "mods/dawn.jar");
    assert!(out
        .notes
        .iter()
        .any(|n| n.contains("config file is not synced")));
    let all = instances::list_in(&home, &data);
    let made = all.iter().find(|i| i.name == "Dawn").unwrap();
    let java = made.java.as_ref().unwrap();
    assert_eq!(java.minecraft, "1.21.4");
    assert_eq!(java.loader.as_ref().unwrap().version, "0.16.9");
    assert_eq!(
        fs::read(made.home.join("mods/dawn.jar")).unwrap(),
        fs::read(&jar_path).unwrap()
    );
    // The same JAR again into the same new name says so, and changes nothing.
    let err = install(
        &home,
        &data,
        Some(&versions),
        &Goal {
            jar: &jar_path,
            instance: None,
            name: None,
            minecraft: None,
            loader_version: None,
        },
        &fake_fetch,
    )
    .unwrap_err();
    assert!(err.contains("already an instance called Dawn"), "{err}");
    fs::remove_dir_all(home.parent().unwrap()).ok();
}

#[test]
fn an_existing_instance_is_opt_in_and_checked() {
    let (home, data) = scratch("existing");
    let jar_path = dawn_jar(&home);
    let versions = versions_fixture();
    // A Horizon instance is refused; a Java one without a loader is told how; a Forge one
    // is refused; a Fabric one takes it.
    let plain = instances::create_in(&home, &data, "Plain", None, None).unwrap();
    fn goal<'a>(jar: &'a Path, instance: Option<&'a str>) -> Goal<'a> {
        Goal {
            jar,
            instance,
            name: None,
            minecraft: None,
            loader_version: None,
        }
    }
    let err = install(
        &home,
        &data,
        None,
        &goal(&jar_path, Some("Plain")),
        &fake_fetch,
    )
    .unwrap_err();
    assert!(
        err.contains("not a Minecraft: Java Edition instance"),
        "{err}"
    );
    let vanilla = instances::create_in(&home, &data, "Vanilla", None, Some("1.21.4")).unwrap();
    let err = install(
        &home,
        &data,
        None,
        &goal(&jar_path, Some("Vanilla")),
        &fake_fetch,
    )
    .unwrap_err();
    assert!(err.contains("no mod loader"), "{err}");
    instances::edit_in(&home, &data, &vanilla.id, |m| {
        m.java.get_or_insert_with(Default::default).loader = Some(Loader {
            kind: Kind::Forge,
            version: "1".to_string(),
        });
    })
    .unwrap();
    let err = install(
        &home,
        &data,
        None,
        &goal(&jar_path, Some("Vanilla")),
        &fake_fetch,
    )
    .unwrap_err();
    assert!(err.contains("cannot load a Fabric client JAR"), "{err}");
    instances::edit_in(&home, &data, &vanilla.id, |m| {
        m.java.get_or_insert_with(Default::default).loader = Some(Loader {
            kind: Kind::Fabric,
            version: "0.16.9".to_string(),
        });
    })
    .unwrap();
    let out = install(
        &home,
        &data,
        None,
        &goal(&jar_path, Some("Vanilla")),
        &fake_fetch,
    )
    .unwrap();
    assert!(!out.created);
    assert_eq!(out.instance, "Vanilla");
    assert!(vanilla.home.join("mods/dawn.jar").is_file());
    // Again: the file is already there, nothing is overwritten.
    let err = install(
        &home,
        &data,
        None,
        &goal(&jar_path, Some("Vanilla")),
        &fake_fetch,
    )
    .unwrap_err();
    assert!(err.contains("already installed"), "{err}");
    assert_eq!(
        fs::read(vanilla.home.join("mods/dawn.jar")).unwrap(),
        fs::read(&jar_path).unwrap()
    );
    // A Minecraft version the JAR rules out is a refusal, naming both sides.
    let old = instances::create_in(&home, &data, "Old", None, Some("1.20.6")).unwrap();
    instances::edit_in(&home, &data, &old.id, |m| {
        m.java.get_or_insert_with(Default::default).loader = Some(Loader {
            kind: Kind::Fabric,
            version: "0.16.9".to_string(),
        });
    })
    .unwrap();
    let pinned = jar(
        &home,
        "pinned.jar",
        &[(
            "fabric.mod.json",
            r#"{"schemaVersion":1,"id":"dawn","version":"1","depends":{"minecraft":"~1.20 <1.20.6"}}"#,
        )],
    );
    let err = install(
        &home,
        &data,
        Some(&versions),
        &Goal {
            jar: &pinned,
            instance: Some("Old"),
            name: None,
            minecraft: None,
            loader_version: None,
        },
        &fake_fetch,
    )
    .unwrap_err();
    assert!(err.contains("not 1.20.6"), "{err}");
    let _ = plain;
    fs::remove_dir_all(home.parent().unwrap()).ok();
}

#[test]
fn a_failed_copy_keeps_the_existing_instance() {
    let (home, data) = scratch("kept");
    let jar_path = dawn_jar(&home);
    let vanilla = instances::create_in(&home, &data, "Vanilla", None, Some("1.21.4")).unwrap();
    instances::edit_in(&home, &data, &vanilla.id, |m| {
        m.java.get_or_insert_with(Default::default).loader = Some(Loader {
            kind: Kind::Fabric,
            version: "0.16.9".to_string(),
        });
    })
    .unwrap();
    // A file where mods/ must be made: the copy cannot land.
    fs::write(vanilla.home.join("mods"), b"not a dir").unwrap();
    let err = install(
        &home,
        &data,
        None,
        &Goal {
            jar: &jar_path,
            instance: Some("Vanilla"),
            name: None,
            minecraft: None,
            loader_version: None,
        },
        &fake_fetch,
    )
    .unwrap_err();
    assert!(err.contains("mods"), "{err}");
    // The instance is untouched: still listed, no JAR, no temp left behind.
    assert!(instances::list_in(&home, &data)
        .iter()
        .any(|i| i.name == "Vanilla"));
    assert!(!vanilla.home.join("mods").is_dir());
    assert!(fs::read_dir(&vanilla.home).unwrap().all(|e| !e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains("horizon-part")));
    fs::remove_dir_all(home.parent().unwrap()).ok();
}

#[test]
fn unmaking_a_created_instance_cleans_dir_and_registry() {
    let (home, data) = scratch("unmake");
    let made = instances::create_in(&home, &data, "Made", None, Some("1.21.4")).unwrap();
    assert!(made.home.exists());
    instances::delete_in(&home, &data, &made.id, &|p| {
        fs::remove_dir_all(p).map_err(|e| e.to_string())
    })
    .unwrap();
    assert!(!made.home.exists());
    assert!(instances::list_in(&home, &data)
        .iter()
        .all(|i| i.id != made.id));
    fs::remove_dir_all(home.parent().unwrap()).ok();
}

#[test]
fn the_copy_lands_whole_or_not_at_all() {
    let dir = std::env::temp_dir().join(format!("cj-copy-{}", std::process::id()));
    let home = dir.join("home");
    fs::create_dir_all(&home).unwrap();
    let jar_path = dawn_jar(&dir);
    copy_in(&home, &jar_path, "mods/dawn.jar").unwrap();
    assert_eq!(
        fs::read(home.join("mods/dawn.jar")).unwrap(),
        fs::read(&jar_path).unwrap()
    );
    // A second copy refuses rather than overwrite.
    assert!(copy_in(&home, &jar_path, "mods/dawn.jar")
        .unwrap_err()
        .contains("already there"));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn malformed_or_unsupported_ranges_never_match() {
    for wanted in ["", "1..2", "1.2;", ">=1.bad", "^1.2", ">=1.0-rc1"] {
        assert_eq!(range_allows(wanted, "2.0"), None, "{wanted:?}");
    }
    assert!(!mod_id_ok("a"));
    assert!(mod_id_ok("ab"));
    assert!(mod_id_ok(&"a".repeat(64)));
    assert!(!mod_id_ok(&"a".repeat(65)));
}

#[test]
fn manifest_dependency_shape_is_not_silently_dropped() {
    let dir = std::env::temp_dir().join(format!("cj-shapes-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    for (n, field) in [
        ("env", r#""environment":42"#),
        ("deps", r#""depends":[]"#),
        ("value", r#""depends":{"minecraft":42}"#),
        ("empty", r#""depends":{"minecraft":[]}"#),
    ] {
        let body = format!(r#"{{"schemaVersion":1,"id":"fixture","version":"1",{field}}}"#);
        let path = jar(&dir, &format!("{n}.jar"), &[("fabric.mod.json", &body)]);
        assert!(inspect(&path).is_err(), "accepted malformed {n}");
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn unknown_minecraft_compatibility_refuses_before_writing() {
    let dir = std::env::temp_dir().join(format!("cj-unknown-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = jar(
        &dir,
        "fixture.jar",
        &[(
            "fabric.mod.json",
            r#"{"schemaVersion":1,"id":"fixture","version":"1","depends":{"minecraft":"^1.21"}}"#,
        )],
    );
    let info = inspect(&path).unwrap();
    assert!(check_minecraft(&info, "1.21.4", &mut Vec::new()).is_err());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
#[cfg(unix)]
fn a_symlink_mods_folder_cannot_redirect_the_copy() {
    let (home, data) = scratch("symlink-copy");
    let outside = data.join("outside");
    fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, home.join("mods")).unwrap();
    let source = dawn_jar(&data);
    assert!(copy_in(&home, &source, "mods/dawn.jar").is_err());
    assert!(!outside.join("dawn.jar").exists());
    fs::remove_dir_all(home.parent().unwrap()).unwrap();
}

#[test]
fn an_incompatible_loader_is_not_installed_with_a_warning() {
    let (home, data) = scratch("bad-loader");
    let source = jar(
        &home,
        "fixture.jar",
        &[(
            "fabric.mod.json",
            r#"{"schemaVersion":1,"id":"fixture","version":"1","depends":{"fabricloader":">=99","minecraft":"~1.21"}}"#,
        )],
    );
    let result = install(
        &home,
        &data,
        Some(&versions_fixture()),
        &Goal {
            jar: &source,
            instance: None,
            name: None,
            minecraft: None,
            loader_version: None,
        },
        &fake_fetch,
    );
    assert!(result.is_err(), "incompatible loader was installed");
    assert!(!home.join("instances/fixture").exists());
    fs::remove_dir_all(home.parent().unwrap()).unwrap();
}

#[test]
#[cfg(unix)]
fn a_planted_staging_symlink_does_not_overwrite_another_file() {
    let (home, data) = scratch("stage-link");
    fs::create_dir_all(home.join("mods")).unwrap();
    let victim = data.join("victim");
    fs::write(&victim, b"keep").unwrap();
    std::os::unix::fs::symlink(&victim, home.join("mods/dawn.jar.horizon-part")).unwrap();
    let source = dawn_jar(&data);
    let _ = copy_in(&home, &source, "mods/dawn.jar");
    assert_eq!(fs::read(victim).unwrap(), b"keep");
    fs::remove_dir_all(home.parent().unwrap()).unwrap();
}

#[test]
fn required_mod_dependencies_are_not_silently_ignored() {
    let (home, data) = scratch("missing-dep");
    let source = jar(&home, "dependency.jar", &[("fabric.mod.json", DAWN_LIKE)]);
    let result = install(
        &home,
        &data,
        Some(&versions_fixture()),
        &Goal {
            jar: &source,
            instance: None,
            name: None,
            minecraft: None,
            loader_version: None,
        },
        &fake_fetch,
    );
    assert!(
        result.is_err(),
        "installed without declared fabric-api dependency"
    );
    assert!(!home.join("instances/dawn").exists());
    fs::remove_dir_all(home.parent().unwrap()).unwrap();
}

#[test]
fn duplicate_mod_id_refuses_even_without_dependencies() {
    let (home, data) = scratch("duplicate-id");
    let made = instances::create_in(&home, &data, "Existing", None, Some("1.21.4")).unwrap();
    fs::create_dir_all(made.home.join("mods")).unwrap();
    dawn_jar(&made.home.join("mods"));
    let info = inspect(&dawn_jar(&data)).unwrap();
    assert!(check_mod_dependencies(&info, Some(&made.home)).is_err());
    fs::remove_dir_all(home.parent().unwrap()).unwrap();
}

#[test]
fn java_requirement_checks_selected_runtime_and_verified_metadata() {
    let (home, data) = scratch("java-check");
    let info = inspect(&jar(&home, "java.jar", &[("fabric.mod.json", DAWN_LIKE)])).unwrap();
    fs::write(data.join("release"), "JAVA_VERSION=\"17.0.1\"\n").unwrap();
    let mut game = instances::JavaGame {
        home: Some(data.clone()),
        ..Default::default()
    };
    assert!(check_java(&info, &game, None, &fake_fetch).is_err());
    fs::write(data.join("release"), "JAVA_VERSION=\"21.0.1\"\n").unwrap();
    check_java(&info, &game, None, &fake_fetch).unwrap();
    let metadata = br#"{"javaVersion":{"majorVersion":21,"component":"java-runtime-delta"}}"#;
    let mut versions = versions_fixture();
    versions
        .all
        .iter_mut()
        .find(|v| v.id == "1.21.4")
        .unwrap()
        .sha1 = crate::sources::sha1_hex(metadata);
    game.home = None;
    game.minecraft = "1.21.4".into();
    check_java(&info, &game, Some(&versions), &|url| {
        assert_eq!(url, "u");
        Ok(metadata.to_vec())
    })
    .unwrap();
    assert!(check_java(&info, &game, Some(&versions), &|_| Ok(b"corrupt".to_vec())).is_err());
    fs::remove_dir_all(home.parent().unwrap()).unwrap();
}

#[test]
fn active_game_mark_prevents_client_install() {
    let (home, data) = scratch("active-game");
    let made = instances::create_in(&home, &data, "Busy", None, Some("1.21.4")).unwrap();
    instances::edit_in(&home, &data, &made.id, |m| {
        m.java.as_mut().unwrap().loader = Some(Loader {
            kind: Kind::Fabric,
            version: "0.16.9".into(),
        })
    })
    .unwrap();
    let mark = crate::config_sync::claim_running(&made.home)
        .unwrap()
        .unwrap();
    let source = dawn_jar(&data);
    let result = install(
        &home,
        &data,
        None,
        &Goal {
            jar: &source,
            instance: Some("Busy"),
            name: None,
            minecraft: None,
            loader_version: None,
        },
        &fake_fetch,
    );
    assert!(result.is_err(), "installed into a running game's folder");
    assert!(!made.home.join("mods/dawn.jar").exists());
    mark.release().unwrap();
    fs::remove_dir_all(home.parent().unwrap()).unwrap();
}

#[test]
fn malformed_store_leaves_no_new_client_folder() {
    let (home, data) = scratch("bad-store");
    fs::write(data.join("instances.json"), b"broken").unwrap();
    let source = dawn_jar(&home);
    let result = install(
        &home,
        &data,
        Some(&versions_fixture()),
        &Goal {
            jar: &source,
            instance: None,
            name: None,
            minecraft: None,
            loader_version: None,
        },
        &fake_fetch,
    );
    assert!(result.is_err());
    assert!(!home.join("instances/dawn").exists());
    assert_eq!(fs::read(data.join("instances.json")).unwrap(), b"broken");
    fs::remove_dir_all(home.parent().unwrap()).unwrap();
}

#[test]
fn unsupported_resolution_features_are_not_claimed_compatible() {
    let (home, data) = scratch("resolution");
    for field in [
        r#""jars":[{"file":"nested.jar"}]"#,
        r#""provides":["alias"]"#,
        r#""breaks":{"other":"*"}"#,
    ] {
        let body = format!(r#"{{"schemaVersion":1,"id":"fixture","version":"1",{field}}}"#);
        let source = jar(&home, "fixture.jar", &[("fabric.mod.json", &body)]);
        assert!(
            inspect(&source).is_err(),
            "accepted unverified resolution: {field}"
        );
    }
    fs::remove_dir_all(home.parent().unwrap()).unwrap();
    let _ = data;
}

#[test]
fn a_failed_preparation_is_not_a_listed_instance() {
    let (home, data) = scratch("prepare-fail");
    let result = instances::create_client_in(
        &home,
        &data,
        "Failed",
        instances::JavaGame::default(),
        |stage| {
            fs::write(stage.join("partial"), b"partial").unwrap();
            Err("copy failed".into())
        },
    );
    assert!(result.is_err());
    assert!(!home.join("instances/failed").exists());
    assert!(!data.join("instances.json").exists());
    assert!(!fs::read_dir(&home).unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".client-stage-")));
    fs::remove_dir_all(home.parent().unwrap()).unwrap();
}

#[test]
fn failed_store_commit_cleans_prepared_client_instance() {
    let (home, data) = scratch("save-fail");
    // Store::save uses a same-directory temporary file. Make that name a directory so the
    // commit fails, after preparation, without needing timing or privileged permissions.
    fs::create_dir(data.join(format!("instances.json.tmp-{}", std::process::id()))).unwrap();
    let result = instances::create_client_in(
        &home,
        &data,
        "Failed",
        instances::JavaGame::default(),
        |stage| {
            fs::create_dir(stage.join("mods")).unwrap();
            fs::write(stage.join("mods/test.jar"), b"complete").unwrap();
            Ok(())
        },
    );
    assert!(result.is_err());
    assert!(!home.join("instances/failed").exists());
    assert!(!data.join("instances.json").exists());
    fs::remove_dir_all(home.parent().unwrap()).unwrap();
}

#[test]
fn installed_dependency_can_satisfy_a_required_mod() {
    let (home, data) = scratch("dependency-ok");
    let mods = home.join("mods");
    fs::create_dir_all(&mods).unwrap();
    jar(
        &mods,
        "dependency.jar",
        &[(
            "fabric.mod.json",
            r#"{"schemaVersion":1,"id":"fabric-api","version":"0.100.0+mc1.21"}"#,
        )],
    );
    let info = inspect(&jar(&data, "client.jar", &[("fabric.mod.json", DAWN_LIKE)])).unwrap();
    check_mod_dependencies(&info, Some(&home)).unwrap();
    fs::remove_dir_all(home.parent().unwrap()).unwrap();
}

#[test]
fn failed_compatibility_check_releases_only_installers_mark() {
    let (home, data) = scratch("release-error");
    let made = instances::create_in(&home, &data, "Checked", None, Some("1.21.4")).unwrap();
    instances::edit_in(&home, &data, &made.id, |m| {
        m.java.as_mut().unwrap().loader = Some(Loader {
            kind: Kind::Fabric,
            version: "0.1.0".into(),
        })
    })
    .unwrap();
    let source = dawn_jar(&data);
    let result = install(
        &home,
        &data,
        None,
        &Goal {
            jar: &source,
            instance: Some("Checked"),
            name: None,
            minecraft: None,
            loader_version: None,
        },
        &fake_fetch,
    );
    assert!(result.is_err());
    let mark = crate::config_sync::claim_running(&made.home)
        .unwrap()
        .expect("installer released its claim after error");
    mark.release().unwrap();
    fs::remove_dir_all(home.parent().unwrap()).unwrap();
}
