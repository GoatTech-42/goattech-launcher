use super::*;
use serde_json::json;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("horizon-other-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn prism_and_multimc_instances() {
    let root = scratch("prism");
    let fabric = root.join("instances/Fabric 1.21.1");
    write(
        &fabric.join("instance.cfg"),
        "[General]\nInstanceType=OneSix\nname=My Fabric\niconKey=default\n",
    );
    let pack = json!({"formatVersion": 1, "components": [
        {"uid": "org.lwjgl3", "version": "3.3.3", "dependencyOnly": true},
        {"uid": "net.minecraft", "version": "1.21.1", "important": true},
        {"uid": "net.fabricmc.intermediary", "version": "1.21.1"},
        {"uid": "net.fabricmc.fabric-loader", "version": "0.16.9"}
    ]});
    write(&fabric.join("mmc-pack.json"), &pack.to_string());
    write(&fabric.join("minecraft/mods/sodium.jar"), "jar");
    // MultiMC's game folder, and no instance.cfg: the folder's name.
    let old = root.join("instances/old");
    write(
        &old.join("mmc-pack.json"),
        &json!({"components": [{"uid": "net.minecraft", "version": "1.12.2"},
            {"uid": "net.minecraftforge", "version": "14.23.5.2860"}]})
        .to_string(),
    );
    fs::create_dir_all(old.join(".minecraft")).unwrap();
    // Not an instance.
    fs::create_dir_all(root.join("instances/_MMC_TEMP")).unwrap();

    let found = prism_instances(&root, "Prism Launcher");
    assert_eq!(
        found,
        [
            Found {
                launcher: "Prism Launcher",
                name: "My Fabric".into(),
                minecraft: Some("1.21.1".into()),
                loader: Some(Loader {
                    name: "fabric",
                    version: "0.16.9".into()
                }),
                game: fabric.join("minecraft"),
                shared: false,
            },
            Found {
                launcher: "Prism Launcher",
                name: "old".into(),
                minecraft: Some("1.12.2".into()),
                loader: Some(Loader {
                    name: "forge",
                    version: "14.23.5.2860".into()
                }),
                game: old.join(".minecraft"),
                shared: false,
            },
        ]
    );
    assert_eq!(in_folder(&fabric, None)[0].name, "My Fabric");
    // Fabric comes along at the same version; Forge isn't installed by Horizon yet.
    assert_eq!(
        loader(&found[0]),
        Some(crate::loaders::Loader {
            kind: crate::loaders::Kind::Fabric,
            version: "0.16.9".into()
        })
    );
    assert_eq!(loader(&found[1]), None);
    assert_eq!(in_folder(&root.join("instances"), Some("MultiMC")).len(), 2);
    let named = root.join("MultiMC/instances/old");
    fs::create_dir_all(named.parent().unwrap()).unwrap();
    fs::rename(&old, &named).unwrap();
    assert_eq!(in_folder(&named, None)[0].launcher, "MultiMC");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn official_launcher_profiles() {
    let root = scratch("official");
    let custom = root.join("custom");
    let profiles = json!({"profiles": {
        "a": {"type": "latest-release", "lastVersionId": "latest-release", "name": ""},
        "b": {"type": "custom", "name": "Fabric", "lastVersionId": "fabric-loader-0.16.9-1.21.1",
              "gameDir": custom.to_string_lossy()},
        "c": {"type": "custom", "name": "Old Forge", "lastVersionId": "1.20.1-forge-47.3.0"},
        "d": {"type": "custom", "name": "", "lastVersionId": "1.21.4"},
        "e": {"type": "custom", "name": "Broken"}
    }});
    write(&root.join("launcher_profiles.json"), &profiles.to_string());
    let profile = |id: &str, parent: &str| {
        let file = root.join(format!("versions/{id}/{id}.json"));
        write(
            &file,
            &json!({"id": id, "inheritsFrom": parent}).to_string(),
        );
    };
    profile("fabric-loader-0.16.9-1.21.1", "1.21.1");
    profile("1.20.1-forge-47.3.0", "1.20.1");

    let found = in_folder(&root, None);
    let short: Vec<_> = found
        .iter()
        .map(|f| {
            (
                f.name.as_str(),
                f.minecraft.as_deref().unwrap(),
                f.loader.as_ref().map(|l| (l.name, l.version.as_str())),
                f.shared,
            )
        })
        .collect();
    assert_eq!(
        short,
        [
            ("1.21.4", "1.21.4", None, true),
            ("Fabric", "1.21.1", Some(("fabric", "0.16.9")), false),
            ("Latest release", "latest", None, true),
            ("Old Forge", "1.20.1", Some(("forge", "47.3.0")), true),
        ]
    );
    assert_eq!(found[1].game, custom);
    assert_eq!(found[0].game, root);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn copies_an_instance_but_only_what_a_shared_folder_holds_for_one() {
    let root = scratch("copy");
    let game = root.join("game");
    for f in [
        "saves/World/level.dat",
        "mods/a.jar",
        "kubejs/startup.js",
        "options.txt",
        "logs/latest.log",
        "versions/1.21.1/1.21.1.jar",
        "launcher_profiles.json",
    ] {
        write(&game.join(f), f);
    }
    let mut found = Found {
        launcher: "Prism Launcher",
        name: "x".into(),
        minecraft: Some("1.21.1".into()),
        loader: None,
        game: game.clone(),
        shared: false,
    };
    let listed = |dir: &Path| {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    let own = root.join("own");
    copy_game(&found, &own).unwrap();
    assert_eq!(
        listed(&own),
        [
            "kubejs",
            "launcher_profiles.json",
            "mods",
            "options.txt",
            "saves",
            "versions"
        ]
    );
    assert_eq!(
        fs::read_to_string(own.join("saves/World/level.dat")).unwrap(),
        "saves/World/level.dat"
    );

    found.shared = true;
    let shared = root.join("shared");
    copy_game(&found, &shared).unwrap();
    assert_eq!(listed(&shared), ["mods", "options.txt", "saves"]);
    // The other launcher's folder is left as it was.
    assert!(game.join("logs/latest.log").is_file());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn curseforge_and_modrinth_app_instances_are_their_game_folders() {
    let root = scratch("apps");
    let curseforge = root.join("Documents/curseforge/minecraft");
    write(&curseforge.join("Instances/All the Mods/mods/a.jar"), "jar");
    write(
        &curseforge.join("Instances/All the Mods/minecraftinstance.json"),
        "{}",
    );
    write(&curseforge.join("Instances/All the Mods/.curseclient"), "");
    fs::create_dir_all(curseforge.join("Instances/empty")).unwrap();
    let modrinth = root.join("data/ModrinthApp");
    write(
        &modrinth.join("profiles/Fabulously Optimized/options.txt"),
        "",
    );

    let folders = Folders {
        home: root.clone(),
        data: root.join("data"),
        config: root.join("config"),
    };
    let found = scan(Os::MacOs, &folders);
    let short: Vec<_> = found
        .iter()
        .map(|f| (f.launcher, f.name.as_str(), f.minecraft.is_none(), f.shared))
        .collect();
    assert_eq!(
        short,
        [
            (MODRINTH_APP, "Fabulously Optimized", true, false),
            (CURSEFORGE, "All the Mods", true, false),
        ]
    );
    let pack = curseforge.join("Instances/All the Mods");
    assert_eq!(in_folder(&curseforge, None), [found[1].clone()]);
    assert_eq!(in_folder(&pack, None), [found[1].clone()]);
    assert_eq!(in_folder(&modrinth, None), [found[0].clone()]);

    // The app's own files stay behind.
    let to = root.join("copy");
    copy_game(&found[1], &to).unwrap();
    assert!(to.join("mods/a.jar").is_file());
    assert!(!to.join("minecraftinstance.json").exists() && !to.join(".curseclient").exists());

    let err = import(&found[1], "x", None, true).unwrap_err();
    assert_eq!(
        err,
        "the CurseForge app doesn't say which Minecraft version All the Mods plays here: name \
         it with --minecraft."
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
#[cfg(unix)]
fn a_linked_file_or_folder_is_not_copied() {
    let root = scratch("links");
    let game = root.join("game");
    write(&game.join("mods/real.jar"), "jar");
    write(&game.join("saves/world/level.dat"), "level");
    write(&root.join("outside/secret.txt"), "secret");
    std::os::unix::fs::symlink(root.join("outside"), game.join("linked-dir")).unwrap();
    std::os::unix::fs::symlink(
        root.join("outside/secret.txt"),
        game.join("mods/linked.jar"),
    )
    .unwrap();
    let found = Found {
        launcher: "Prism Launcher",
        name: "Linked".into(),
        minecraft: None,
        loader: None,
        game,
        shared: false,
    };
    let to = root.join("copy");
    copy_game(&found, &to).unwrap();
    assert!(to.join("mods/real.jar").is_file());
    assert!(to.join("saves/world/level.dat").is_file());
    assert!(!to.join("mods/linked.jar").exists());
    assert!(!to.join("linked-dir").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_curseforge_record_says_its_version_and_loader() {
    let root = scratch("cf-json");
    let pack = root.join("Documents/curseforge/minecraft/Instances/ForgePack");
    write(&pack.join("mods/forge-mod.jar"), "jar");
    write(&pack.join("options.txt"), "");
    write(
        &pack.join("minecraftinstance.json"),
        r#"{"gameVersion":"1.20.1","baseModLoader":{"name":"forge-47.2.0"}}"#,
    );
    let folders = Folders {
        home: root.clone(),
        data: root.join("data"),
        config: root.join("config"),
    };
    let found = scan(Os::Linux, &folders);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].minecraft.as_deref(), Some("1.20.1"));
    let loader = found[0].loader.as_ref().expect("a loader");
    assert_eq!(loader.name, "forge");
    assert_eq!(loader.version, "47.2.0");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn mods_that_could_never_load_are_refused_or_left_behind_never_silent() {
    let root = scratch("blocked");
    let game = root.join("game");
    write(&game.join("mods/a.jar"), "jar");
    write(&game.join("options.txt"), "lang:en_us");
    let base = Found {
        launcher: "Prism Launcher",
        name: "x".into(),
        minecraft: Some("1.21.1".into()),
        loader: None,
        game: game.clone(),
        shared: false,
    };

    // No loader recorded, mods present: blocked, and the reason says why and how to proceed.
    let why = mods_blocked(&base).expect("unknown loader + mods is blocked");
    assert!(why.contains("records no mod loader"), "{why}");
    assert!(why.contains("1 mod(s)"), "{why}");

    // Fabric/Quilt loaders are installed: not blocked.
    let mut fabric = base.clone();
    fabric.loader = Some(Loader {
        name: "fabric",
        version: "0.16".into(),
    });
    assert_eq!(mods_blocked(&fabric), None);

    // Forge/NeoForge: blocked, naming the loader and version.
    let mut forge = base.clone();
    forge.loader = Some(Loader {
        name: "forge",
        version: "47.2.0".into(),
    });
    let why = mods_blocked(&forge).expect("forge is blocked");
    assert!(why.contains("forge 47.2.0"), "{why}");

    // No mods: never blocked.
    let mut empty = base.clone();
    empty.game = root.join("empty");
    write(&empty.game.join("options.txt"), "");
    assert_eq!(mods_blocked(&empty), None);

    // A link in mods/ is not a jar the copy would carry.
    #[cfg(unix)]
    {
        write(&game.join("mods/real.jar"), "jar");
        std::os::unix::fs::symlink("/etc/hostname", game.join("mods/link.jar")).unwrap();
        let mut linked = base.clone();
        linked.loader = Some(Loader {
            name: "fabric",
            version: "0.16".into(),
        });
        assert_eq!(mods_blocked(&linked), None);
        let _ = fs::remove_file(game.join("mods/link.jar"));
        let _ = fs::remove_file(game.join("mods/real.jar"));
    }

    // The import refuses with the reason and the escape, and makes nothing.
    let err = import(&base, "blocked-copy", Some("1.21.1"), true).unwrap_err();
    assert!(err.contains("records no mod loader"), "{err}");
    assert!(err.contains("--no-mods"), "{err}");

    // With mods left behind, the copy keeps the rest.
    let made = import(&base, "light-copy", Some("1.21.1"), false).unwrap();
    assert!(made.home.join("options.txt").is_file());
    assert!(!made.home.join("mods").exists());
    let _ = crate::instances::delete(&made.id);
    let _ = fs::remove_dir_all(&root);
}

fn atlauncher_instances_are_read_from_their_instance_json() {
    // Synthetic folders holding only the fields four real instance.json files agree on.
    let root = scratch("atl");
    let forge = json!({"id": "1.20.1", "launcher": {"name": "My Pack", "pack": "Minecraft",
        "loaderVersion": {"version": "47.3.12", "rawVersion": "1.20.1-47.3.12",
            "recommended": false, "type": "Forge"}}});
    let neo = json!({"id": "1.21.1", "launcher": {"name": "Neo",
        "loaderVersion": {"version": "21.1.219", "type": "NeoForge"}}});
    let plain = json!({"id": "1.21.1", "launcher": {"name": ""}});
    let unknown = json!({"id": "1.20.1", "launcher": {"name": "Odd",
        "loaderVersion": {"version": "1", "type": "SomethingElse"}}});
    for (dir, doc) in [
        ("Pack1", &forge),
        ("Neo", &neo),
        ("Plain", &plain),
        ("Odd", &unknown),
    ] {
        write(
            &root.join("instances").join(dir).join("instance.json"),
            &doc.to_string(),
        );
    }
    write(&root.join("instances/Pack1/mods/a.jar"), "x");
    write(&root.join("instances/NotOne/readme.txt"), "x");
    write(
        &root.join("instances/NoLauncher/instance.json"),
        r#"{"id": "1.20.1"}"#,
    );
    write(
        &root.join("instances/NoId/instance.json"),
        r#"{"launcher": {"name": "x"}}"#,
    );
    let found = atlauncher_instances(&root);
    let names: Vec<_> = found.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["My Pack", "Neo", "Odd", "Plain"]);
    let pack = &found[0];
    assert_eq!(pack.launcher, "ATLauncher");
    assert_eq!(pack.minecraft.as_deref(), Some("1.20.1"));
    assert_eq!(pack.loader.as_ref().unwrap().name, "forge");
    assert_eq!(pack.loader.as_ref().unwrap().version, "47.3.12");
    assert_eq!(pack.game, root.join("instances/Pack1"));
    assert_eq!(found[1].loader.as_ref().unwrap().name, "neoforge");
    assert!(found[2].loader.is_none());
    assert!(found[3].loader.is_none());
    assert_eq!(in_folder(&root, None).len(), 4);
    assert_eq!(in_folder(&root.join("instances"), None).len(), 4);
    assert_eq!(in_folder(&root.join("instances/Pack1"), None).len(), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn atlauncher_loader_types_match_whatever_the_case() {
    for (kind, want) in [
        ("fabric", "fabric"),
        ("Quilt", "quilt"),
        ("NEOFORGE", "neoforge"),
    ] {
        let launcher = json!({"loaderVersion": {"type": kind, "version": "1.0"}});
        assert_eq!(atlauncher_loader(&launcher).unwrap().name, want);
    }
    assert!(atlauncher_loader(&json!({"loaderVersion": {"type": "Forge"}})).is_none());
    assert!(atlauncher_loader(&json!({})).is_none());
}