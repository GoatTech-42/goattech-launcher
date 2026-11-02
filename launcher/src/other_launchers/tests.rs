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
                notes: None,
                icon: None,
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
                notes: None,
                icon: None,
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
        notes: None,
        icon: None,
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

    let err = import(&found[1], "x", None).unwrap_err();
    assert_eq!(
        err,
        "the CurseForge app doesn't say which Minecraft version All the Mods plays here: name \
         it with --minecraft."
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
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
#[test]
fn ini_string_reads_what_qts_writer_emits() {
    // Every raw value below is exactly what Qt's INI writer (Prism/MultiMC's
    // INIFile::saveFile -> QSettings::IniFormat) emits for the expected string.
    assert_eq!(ini_string("plain"), "plain");
    assert_eq!(ini_string("one\\ntwo"), "one\ntwo"); // a multi-line note
    assert_eq!(ini_string("a\\tb"), "a\tb"); // a tab
    assert_eq!(ini_string("a\\rb"), "a\rb"); // a carriage return
    assert_eq!(ini_string("a\\ab"), "a\u{7}b"); // a control character
    assert_eq!(ini_string("C:\\\\new"), "C:\\new"); // a Windows path
    assert_eq!(ini_string("say \\\"hello\\\""), "say \"hello\""); // escaped quotes
    assert_eq!(ini_string("\"seed=42\""), "seed=42"); // quoted: it holds an '='
    assert_eq!(ini_string("\"a, b; c\""), "a, b; c"); // quoted: ',' and ';'
    assert_eq!(ini_string("\" padded \""), " padded "); // quoted: edge spaces kept
    assert_eq!(ini_string("trail  "), "trail"); // unquoted: trailing blanks chopped
    assert_eq!(ini_string("\\x41\\x42"), "AB"); // hex escapes
    assert_eq!(ini_string("\\101\\102"), "AB"); // octal escapes
    assert_eq!(ini_string("a\\qb"), "ab"); // an unknown escape drops both characters
    assert_eq!(ini_string("ends\\"), "ends"); // a trailing lone backslash drops
    assert_eq!(ini_string("line1\\\nline2"), "line1line2"); // a real line break continues
    // A bare comma is Qt's list separator; Prism's `value(key).toString()` of a
    // multi-item list is empty, and the writer always quotes a value holding one.
    assert_eq!(ini_string("a, b"), "");
    // Qt's variant pass after the lexical read: the writer escapes a value's own
    // leading '@' as a second one, so these are literals, never variants.
    assert_eq!(ini_string("@@home"), "@home");
    assert_eq!(ini_string("@@ByteArray(abc)"), "@ByteArray(abc)");
    // The wrappers the writer does emit: bytes, and a QString holding a NUL. The INI
    // escape writes the NUL as \0 and re-escapes a hex digit that follows it, so these
    // two rows are exactly what Qt's writer stores for "a\0b" and "a\01".
    assert_eq!(ini_string("@ByteArray(abc)"), "abc");
    assert_eq!(ini_string("@String(a\\0\\x62)"), "a\u{0}b");
    assert_eq!(ini_string("@String(a\\0\\x31)"), "a\u{0}1");
    // An absent value.
    assert_eq!(ini_string("@Invalid()"), "");
    // Hex/octal escapes accumulate in Qt's char16_t: digits beyond 16 bits wrap.
    assert_eq!(ini_string("\\x110000"), "\u{0}");
}

#[test]
fn quoted_and_escaped_cfg_values_read_through() {
    let root = scratch("prism-quoted");
    let pack = root.join("instances/Quoted");
    // Exactly what Qt's writer emits for name `seed=42, north` and notes
    // `say "hi"` + a tab + `there`.
    write(
        &pack.join("instance.cfg"),
        "[General]\nname=\"seed=42, north\"\nnotes=say \\\"hi\\\"\\tthere\n",
    );
    write(
        &pack.join("mmc-pack.json"),
        &json!({"components": [{"uid": "net.minecraft", "version": "1.21.1"}]}).to_string(),
    );
    write(&pack.join("minecraft/options.txt"), "opts");
    let found = prism_instances(&root, "Prism Launcher");
    assert_eq!(found[0].name, "seed=42, north");
    assert_eq!(found[0].notes.as_deref(), Some("say \"hi\"\tthere"));
    let _ = fs::remove_dir_all(&root);
}
#[test]
fn a_hand_written_continuation_is_not_joined_end_to_end() {
    // ini_value reads a value per physical line: Qt's writer never emits a raw line
    // break in a value (a newline is the \n escape), so a hand-written continuation is
    // out of scope end to end; the trailing lone backslash then drops, the way Qt reads
    // one. ini_string still resolves a line break inside a raw value it is handed.
    let cfg = "[General]\nnotes=line1\\\nline2\n";
    let raw = ini_value(cfg, "notes").unwrap();
    assert_eq!(ini_string(&raw), "line1");
}


#[test]
fn prism_notes_carry_to_the_imported_instance() {
    let root = scratch("prism-notes");
    let pack = root.join("instances/Noted");
    // Qt's INI writer escapes a newline as \n and a backslash as \\.
    write(
        &pack.join("instance.cfg"),
        "[General]\nname=Noted\niconKey=default\nnotes=first line\\nsecond C:\\\\path\\\\x\n",
    );
    write(
        &pack.join("mmc-pack.json"),
        &json!({"components": [{"uid": "net.minecraft", "version": "1.21.1"}]}).to_string(),
    );
    write(&pack.join("minecraft/options.txt"), "opts");

    let found = prism_instances(&root, "Prism Launcher");
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].notes.as_deref(),
        Some("first line\nsecond C:\\path\\x")
    );

    let made = import(&found[0], "noted-copy", Some("1.21.1")).unwrap();
    let listed = crate::instances::list();
    let again = listed.iter().find(|i| i.id == made.id).unwrap();
    assert_eq!(again.notes.as_deref(), Some("first line\nsecond C:\\path\\x"));
    let _ = crate::instances::delete(&made.id);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn import_without_notes_leaves_them_unset() {
    let root = scratch("no-notes");
    let game = root.join("game");
    write(&game.join("options.txt"), "opts");
    let base = Found {
        launcher: "Prism Launcher",
        name: "x".into(),
        minecraft: Some("1.21.1".into()),
        loader: None,
        notes: None,
        icon: None,
        game: game.clone(),
        shared: false,
    };
    let made = import(&base, "noteless-copy", Some("1.21.1")).unwrap();
    let listed = crate::instances::list();
    let again = listed.iter().find(|i| i.id == made.id).unwrap();
    assert_eq!(again.notes, None);
    let _ = crate::instances::delete(&made.id);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn prism_icons_carry() {
    let root = scratch("prism-icons");
    let pack = root.join("instances/Pack");
    write(&pack.join("instance.cfg"), "[General]\nname=Pack\niconKey=custom\n");
    write(
        &pack.join("mmc-pack.json"),
        &json!({"components": [{"uid": "net.minecraft", "version": "1.21.1"}]}).to_string(),
    );
    write(&pack.join("minecraft/saves/x/level.dat"), "dat");
    write(&root.join("icons/custom.png"), "icon");
    // Its own icon.png wins over the iconKey.
    let own = root.join("instances/Own");
    write(&own.join("instance.cfg"), "[General]\nname=Own\niconKey=custom\n");
    write(
        &own.join("mmc-pack.json"),
        &json!({"components": [{"uid": "net.minecraft", "version": "1.21.1"}]}).to_string(),
    );
    write(&own.join("minecraft/options.txt"), "opts");
    write(&own.join("icon.png"), "own");

    let found = prism_instances(&root, "Prism Launcher");
    assert_eq!(found[1].icon, Some(root.join("icons/custom.png")));
    assert_eq!(found[0].icon, Some(own.join("icon.png")));

    let to = root.join("imported");
    copy_game(&found[1], &to).unwrap();
    assert_eq!(fs::read(to.join("icon.png")).unwrap(), b"icon");
    assert!(to.join("saves/x/level.dat").is_file());
    // The instance's own icon.png is not only selected over the iconKey, it is the
    // one that lands in the new home.
    let own_to = root.join("imported-own");
    copy_game(&found[0], &own_to).unwrap();
    assert_eq!(fs::read(own_to.join("icon.png")).unwrap(), b"own");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_path_like_icon_key_carries_nothing() {
    let root = scratch("prism-icon-key-escape");
    let pack = root.join("instances/Pack");
    write(
        &pack.join("instance.cfg"),
        "[General]\nname=Pack\niconKey=../../outside\n",
    );
    write(
        &pack.join("mmc-pack.json"),
        &json!({"components": [{"uid": "net.minecraft", "version": "1.21.1"}]}).to_string(),
    );
    write(&pack.join("minecraft/options.txt"), "opts");
    write(&root.join("outside.png"), "secret");
    let found = prism_instances(&root, "Prism Launcher");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].icon, None);
    let to = root.join("imported");
    copy_game(&found[0], &to).unwrap();
    assert!(!to.join("icon.png").exists());
}

// Links are a unix-only fixture here.
#[cfg(unix)]
#[test]
fn a_linked_icon_is_left_behind() {
    let root = scratch("prism-icon-link");
    let pack = root.join("instances/Pack");
    write(
        &pack.join("mmc-pack.json"),
        &json!({"components": [{"uid": "net.minecraft", "version": "1.21.1"}]}).to_string(),
    );
    write(&pack.join("minecraft/options.txt"), "opts");
    write(&root.join("real.png"), "icon");
    std::os::unix::fs::symlink(root.join("real.png"), pack.join("icon.png")).unwrap();
    let found = prism_instances(&root, "Prism Launcher");
    // Detected through the link, refused at the scan: no icon carries, the card falls
    // back to the instance's mark, and the import still happened.
    assert_eq!(found[0].icon, None);
    let to = root.join("imported");
    copy_game(&found[0], &to).unwrap();
    assert!(!to.join("icon.png").exists());
    assert!(to.join("options.txt").is_file());
}

#[test]
fn a_missing_icon_never_fails_the_import() {
    let root = scratch("prism-icon-gone");
    let game = root.join("game");
    write(&game.join("options.txt"), "opts");
    // One gone between scan and import: skipped, not an import failure.
    let found = Found {
        launcher: "Prism Launcher",
        name: "Gone".into(),
        minecraft: Some("1.21.1".into()),
        loader: None,
        icon: Some(root.join("vanished.png")),
        notes: None,
        game: game.clone(),
        shared: false,
    };
    copy_game(&found, &root.join("imported")).unwrap();
    assert!(root.join("imported/options.txt").is_file());
}
#[test]
fn the_game_folders_own_icon_is_never_clobbered() {
    let root = scratch("icon-no-clobber");
    let game = root.join("game");
    write(&game.join("options.txt"), "opts");
    write(&game.join("icon.png"), "game-own");
    write(&root.join("icons/custom.png"), "shared");
    let found = Found {
        launcher: "Prism Launcher",
        name: "x".into(),
        minecraft: Some("1.21.1".into()),
        loader: None,
        notes: None,
        icon: Some(root.join("icons/custom.png")),
        game: game.clone(),
        shared: false,
    };
    let to = root.join("imported");
    copy_game(&found, &to).unwrap();
    assert_eq!(fs::read(to.join("icon.png")).unwrap(), b"game-own");
    let _ = fs::remove_dir_all(&root);
}
#[test]
fn an_existing_temp_name_is_never_truncated() {
    let root = scratch("icon-tmp-clash");
    let game = root.join("game");
    write(&game.join("options.txt"), "opts");
    // A file the game folder already holds under our first temporary's name.
    write(&game.join("icon.png.horizon-import-tmp"), "user-file");
    write(&root.join("icons/custom.png"), "shared");
    let found = Found {
        launcher: "Prism Launcher",
        name: "x".into(),
        minecraft: Some("1.21.1".into()),
        loader: None,
        notes: None,
        icon: Some(root.join("icons/custom.png")),
        game: game.clone(),
        shared: false,
    };
    let to = root.join("imported");
    copy_game(&found, &to).unwrap();
    // The icon still lands, the preexisting file keeps its bytes, and no temporary of
    // ours is left behind.
    assert_eq!(fs::read(to.join("icon.png")).unwrap(), b"shared");
    assert_eq!(
        fs::read(to.join("icon.png.horizon-import-tmp")).unwrap(),
        b"user-file"
    );
    assert!(!to.join("icon.png.horizon-import-tmp.1").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn an_oversized_icon_is_skipped_not_failed() {
    let root = scratch("prism-icon-huge");
    let game = root.join("game");
    write(&game.join("options.txt"), "opts");
    let big = root.join("big.png");
    fs::write(&big, vec![0u8; (crate::instances::MAX_PICTURE_BYTES + 1) as usize]).unwrap();
    let found = Found {
        launcher: "Prism Launcher",
        name: "Huge".into(),
        minecraft: Some("1.21.1".into()),
        loader: None,
        notes: None,
        icon: Some(big),
        game: game.clone(),
        shared: false,
    };
    copy_game(&found, &root.join("imported")).unwrap();
    assert!(!root.join("imported/icon.png").exists());
    assert!(root.join("imported/options.txt").is_file());
    let _ = fs::remove_dir_all(&root);
}

// Links are a unix-only fixture here.
#[cfg(unix)]
#[test]
fn a_linked_icons_dir_carries_nothing() {
    let root = scratch("prism-icons-dir-link");
    let pack = root.join("instances/Pack");
    write(
        &pack.join("mmc-pack.json"),
        &json!({"components": [{"uid": "net.minecraft", "version": "1.21.1"}]}).to_string(),
    );
    write(&pack.join("minecraft/options.txt"), "opts");
    write(&pack.join("instance.cfg"), "[General]\niconKey=custom\n");
    write(&root.join("outside/custom.png"), "outside-icon");
    std::os::unix::fs::symlink(root.join("outside"), root.join("icons")).unwrap();
    let found = prism_instances(&root, "Prism Launcher");
    assert_eq!(found.len(), 1);
    // The key would resolve through the linked `icons/` folder: refused at scan, so
    // the import never even sees it.
    assert_eq!(found[0].icon, None);
    let to = root.join("imported");
    copy_game(&found[0], &to).unwrap();
    assert!(!to.join("icon.png").exists());
    assert!(to.join("options.txt").is_file());
    let _ = fs::remove_dir_all(&root);
}

// Links are a unix-only fixture here.
#[cfg(unix)]
#[test]
fn a_linked_instance_folder_keeps_no_own_icon() {
    let root = scratch("prism-instance-link");
    let real = root.join("real/Real");
    write(
        &real.join("mmc-pack.json"),
        &json!({"components": [{"uid": "net.minecraft", "version": "1.21.1"}]}).to_string(),
    );
    write(&real.join("minecraft/options.txt"), "opts");
    write(&real.join("icon.png"), "own");
    fs::create_dir_all(root.join("instances")).unwrap();
    std::os::unix::fs::symlink(&real, root.join("instances/Linked")).unwrap();
    let found = prism_instances(&root, "Prism Launcher");
    // The instance itself still imports (that is the scan's preexisting behavior);
    // its icon is not read through the link.
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].icon, None);
    let _ = fs::remove_dir_all(&root);
}
