//! Version files shaped like Mojang's (a 1.21 and a 1.8.9), with the files they name made up in
//! a scratch folder.
use super::*;
use serde_json::json;
use std::io::Write;

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("horizon-java-launch-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// `rel` (`a/b/c`) under `base`, one part at a time as the code joins them (Windows paths).
fn under(base: &Path, rel: &str) -> PathBuf {
    rel.split('/')
        .fold(base.to_path_buf(), |path, part| path.join(part))
}

fn player() -> Player {
    Player {
        name: "Notch".into(),
        uuid: "069a79f444e94726a5befca90e38aaf5".into(),
        token: "mc-token".into(),
        xuid: "2535400000000000".into(),
    }
}

fn args(cmd: &Command) -> Vec<String> {
    cmd.get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

fn modern() -> Value {
    json!({
        "id": "1.21.4", "type": "release", "mainClass": "net.minecraft.client.main.Main",
        "assetIndex": {"id": "19"},
        "arguments": {
            "game": ["--username", "${auth_player_name}", "--version", "${version_name}",
                     "--gameDir", "${game_directory}", "--assetsDir", "${assets_root}",
                     "--assetIndex", "${assets_index_name}", "--uuid", "${auth_uuid}",
                     "--accessToken", "${auth_access_token}", "--xuid", "${auth_xuid}",
                     "--userType", "${user_type}",
                     {"rules": [{"action": "allow", "features": {"is_demo_user": true}}], "value": "--demo"},
                     {"rules": [{"action": "allow", "features": {"has_custom_resolution": true}}],
                      "value": ["--width", "${resolution_width}", "--height", "${resolution_height}"]},
                     {"rules": [{"action": "allow", "features": {"is_quick_play_multiplayer": true}}],
                      "value": ["--quickPlayMultiplayer", "${quickPlayMultiplayer}"]}],
            "jvm": [{"rules": [{"action": "allow", "os": {"name": "osx"}}], "value": ["-XstartOnFirstThread"]},
                    {"rules": [{"action": "allow", "os": {"name": "windows", "arch": "x86"}}], "value": "-Xss1M"},
                    "-Djava.library.path=${natives_directory}", "-Dminecraft.launcher.brand=${launcher_name}",
                    "-cp", "${classpath}"]
        },
        "logging": {"client": {"argument": "-Dlog4j.configurationFile=${path}",
                               "file": {"id": "client-1.21.2.xml"}}},
        "libraries": [
            {"name": "a:common:1", "downloads": {"artifact": {"path": "a/common/1/common-1.jar"}}},
            {"name": "a:mac:1", "downloads": {"artifact": {"path": "a/mac/1/mac-1.jar"}},
             "rules": [{"action": "allow", "os": {"name": "osx"}}]},
            {"name": "a:not-mac:1", "downloads": {"artifact": {"path": "a/not-mac/1/not-mac-1.jar"}},
             "rules": [{"action": "allow"}, {"action": "disallow", "os": {"name": "osx"}}]},
            {"name": "a:common:1-again", "downloads": {"artifact": {"path": "a/common/1/common-1.jar"}}}
        ]
    })
}

#[test]
fn a_modern_version_gets_its_rules_placeholders_and_classpath() {
    let dir = scratch("modern");
    let (mc, game, natives) = (dir.join("mc"), dir.join("game"), dir.join("natives"));
    let java = dir.join("java");
    let p = player();
    let extra = vec!["-XX:+UseZGC".to_string()];
    let launch = |os| Launch {
        minecraft: &mc,
        game_dir: &game,
        natives: &natives,
        java: &java,
        memory_mb: 6144,
        jvm_args: &extra,
        player: &p,
        os,
        window: None,
        server: None,
    };
    let cmd = command(&modern(), &launch("osx")).unwrap();
    let a = args(&cmd);
    let at = |s: &str| {
        a.iter()
            .position(|x| x == s)
            .unwrap_or_else(|| panic!("no {s} in {a:?}"))
    };
    assert_eq!(a[0], "-Xmx6144M");
    assert!(a.contains(&"-Dlog4j2.formatMsgNoLookups=true".into()));
    let config = under(&mc, "assets/log_configs/client-1.21.2.xml");
    assert!(a.contains(&format!("-Dlog4j.configurationFile={}", config.display())));
    assert!(at("-XstartOnFirstThread") < at("net.minecraft.client.main.Main"));
    assert!(at("-XX:+UseZGC") < at("net.minecraft.client.main.Main"));
    assert!(a.contains(&"-Dminecraft.launcher.brand=horizon-launcher".into()));
    assert_eq!(a[at("--username") + 1], "Notch");
    assert_eq!(a[at("--accessToken") + 1], "mc-token");
    assert_eq!(a[at("--assetIndex") + 1], "19");
    assert_eq!(a[at("--gameDir") + 1], game.to_string_lossy());
    assert_eq!(a[at("--userType") + 1], "msa");
    // Features are off: no demo, no custom size.
    assert!(!a.iter().any(|x| x == "--demo" || x == "--width"));
    let sep = if cfg!(windows) { ";" } else { ":" };
    let cp: Vec<String> = a[at("-cp") + 1].split(sep).map(String::from).collect();
    // Library paths are Mojang's, `/` and all (`mojang::relative`): Windows takes them.
    let lib = |p: &str| mc.join("libraries").join(p).to_string_lossy().into_owned();
    assert_eq!(
        cp,
        [
            lib("a/common/1/common-1.jar"),
            lib("a/mac/1/mac-1.jar"),
            under(&mc, "versions/1.21.4/1.21.4.jar")
                .to_string_lossy()
                .into_owned()
        ]
    );
    assert_eq!(cmd.get_current_dir(), Some(game.as_path()));

    let linux = args(&command(&modern(), &launch("linux")).unwrap());
    assert!(!linux.contains(&"-XstartOnFirstThread".into()));
    assert!(linux.iter().any(|x| x.contains("not-mac-1.jar")));
    // The x86-only rule holds on 32-bit Windows alone.
    let windows = args(&command(&modern(), &launch("windows")).unwrap());
    assert_eq!(
        windows.contains(&"-Xss1M".into()),
        cfg!(target_arch = "x86")
    );
    let _ = fs::remove_dir_all(&dir);
}

/// A 1.8.9-shaped version: `minecraftArguments`, natives unpacked without META-INF, and a
/// virtual asset index laid out by name.
#[test]
fn an_old_version_gets_its_natives_and_assets_by_name() {
    let dir = scratch("old");
    let (mc, game, natives) = (dir.join("mc"), dir.join("game"), dir.join("natives"));
    let jar = mc.join("libraries/org/lwjgl/lwjgl-platform-2.9.4-natives-osx.jar");
    fs::create_dir_all(jar.parent().unwrap()).unwrap();
    let mut zip = zip::ZipWriter::new(fs::File::create(&jar).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    for (name, body) in [
        ("liblwjgl.dylib", b"lib".as_slice()),
        ("META-INF/MANIFEST.MF", b"m"),
    ] {
        zip.start_file(name, opts).unwrap();
        zip.write_all(body).unwrap();
    }
    zip.finish().unwrap();
    let hash = "0123456789abcdef0123456789abcdef01234567";
    let object = mc.join("assets/objects/01").join(hash);
    fs::create_dir_all(object.parent().unwrap()).unwrap();
    fs::write(&object, b"ogg").unwrap();
    fs::create_dir_all(mc.join("assets/indexes")).unwrap();
    fs::write(
        mc.join("assets/indexes/1.8.json"),
        json!({"virtual": true, "objects": {"sounds/step/grass1.ogg": {"hash": hash, "size": 3}}})
            .to_string(),
    )
    .unwrap();
    let version = json!({
        "id": "1.8.9", "type": "release", "mainClass": "net.minecraft.client.main.Main",
        "assetIndex": {"id": "1.8"},
        "minecraftArguments": "--username ${auth_player_name} --session ${auth_session} --assetsDir ${game_assets} --userProperties ${user_properties} --unknown ${nothing}",
        "libraries": [{
            "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4",
            "natives": {"osx": "natives-osx"},
            "extract": {"exclude": ["META-INF/"]},
            "downloads": {"classifiers": {"natives-osx": {"path": "org/lwjgl/lwjgl-platform-2.9.4-natives-osx.jar"}}}
        }]
    });
    let (java, p) = (dir.join("java"), player());
    let launch = Launch {
        minecraft: &mc,
        game_dir: &game,
        natives: &natives,
        java: &java,
        memory_mb: 2048,
        jvm_args: &[],
        player: &p,
        os: "osx",
        window: None,
        server: None,
    };
    let a = args(&command(&version, &launch).unwrap());
    assert_eq!(fs::read(natives.join("liblwjgl.dylib")).unwrap(), b"lib");
    assert!(!natives.join("META-INF").exists());
    let virtual_dir = under(&mc, "assets/virtual/1.8");
    assert_eq!(
        fs::read(virtual_dir.join("sounds/step/grass1.ogg")).unwrap(),
        b"ogg"
    );
    let at = |s: &str| a.iter().position(|x| x == s).unwrap();
    assert_eq!(a[at("--assetsDir") + 1], virtual_dir.to_string_lossy());
    assert_eq!(
        a[at("--session") + 1],
        "token:mc-token:069a79f444e94726a5befca90e38aaf5"
    );
    assert_eq!(a[at("--userProperties") + 1], "{}");
    assert_eq!(a[at("--unknown") + 1], "${nothing}");
    assert_eq!(a[0], "-Xmx2048M");
    assert!(a.contains(&format!("-Djava.library.path={}", natives.display())));
    // Natives are not on the classpath; the client jar is.
    let cp = &a[at("-cp") + 1];
    assert!(cp.ends_with("1.8.9.jar") && !cp.contains("natives"), "{cp}");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn placeholders_fill_in_place() {
    let vars = HashMap::from([("a", "1".to_string()), ("b", "two".to_string())]);
    assert_eq!(fill("x${a}y${b}z", &vars), "x1ytwoz");
    assert_eq!(fill("${c} ${a", &vars), "${c} ${a");
}

/// A loader's profile on its game version: its main class, its libraries first (one replacing
/// the game's), its JVM argument after the game's, and the game's jar.
#[test]
fn a_loader_profile_plays_on_its_game_version() {
    let dir = scratch("loader");
    let mc = dir.join("mc");
    let write = |id: &str, version: &Value| {
        let file = mc.join(format!("versions/{id}/{id}.json"));
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, version.to_string()).unwrap();
    };
    write("1.21.4", &modern());
    let profile = json!({
        "id": "fabric-loader-0.16.9-1.21.4", "inheritsFrom": "1.21.4",
        "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
        "arguments": {"game": [], "jvm": ["-DFabricMcEmu= net.minecraft.client.main.Main "]},
        "libraries": [
            {"name": "net.fabricmc:fabric-loader:0.16.9",
             "downloads": {"artifact": {"path": "net/fabricmc/fabric-loader-0.16.9.jar"}}},
            {"name": "a:common:2", "downloads": {"artifact": {"path": "a/common/2/common-2.jar"}}}
        ]
    });
    let version = resolve(profile, &mc).unwrap();
    assert_eq!(version["id"], "fabric-loader-0.16.9-1.21.4");
    assert_eq!(version["assetIndex"]["id"], "19", "the game's fields stay");
    let cp = classpath(&version, &mc, "linux").unwrap();
    let lib = |p: &str| mc.join("libraries").join(p);
    assert_eq!(
        cp,
        [
            lib("net/fabricmc/fabric-loader-0.16.9.jar"),
            lib("a/common/2/common-2.jar"),
            lib("a/not-mac/1/not-mac-1.jar"),
            under(&mc, "versions/1.21.4/1.21.4.jar"),
        ]
    );
    let (game, natives, java, p) = (
        dir.join("game"),
        dir.join("natives"),
        dir.join("java"),
        player(),
    );
    let launch = Launch {
        minecraft: &mc,
        game_dir: &game,
        natives: &natives,
        java: &java,
        memory_mb: 4096,
        jvm_args: &[],
        player: &p,
        os: "linux",
        window: None,
        server: None,
    };
    let a = args(&command(&version, &launch).unwrap());
    let at = |s: &str| a.iter().position(|x| x == s).unwrap();
    assert!(at("-cp") < at("-DFabricMcEmu= net.minecraft.client.main.Main "));
    assert!(
        at("-DFabricMcEmu= net.minecraft.client.main.Main ")
            < at("net.fabricmc.loader.impl.launch.knot.KnotClient")
    );
    assert_eq!(a[at("--version") + 1], "fabric-loader-0.16.9-1.21.4");

    // On a version from before 1.13 the profile's JVM argument joins the old ones.
    write(
        "1.8.9",
        &json!({"id": "1.8.9", "mainClass": "net.minecraft.client.main.Main",
                "minecraftArguments": "--username ${auth_player_name}", "libraries": []}),
    );
    let old = resolve(
        json!({"id": "fabric-loader-0.16.9-1.8.9", "inheritsFrom": "1.8.9",
               "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
               "arguments": {"jvm": ["-Dfabric=1"]}, "libraries": []}),
        &mc,
    )
    .unwrap();
    let a = args(&command(&old, &launch).unwrap());
    assert!(a.contains(&"-cp".into()) && a.contains(&"-Dfabric=1".into()));
    assert_eq!(
        a[a.iter().position(|x| x == "--username").unwrap() + 1],
        "Notch"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_version_that_inherits_from_itself_is_refused() {
    let dir = scratch("loops");
    let mc = dir.join("mc");
    let write = |id: &str, parent: &str| {
        let file = mc.join(format!("versions/{id}/{id}.json"));
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, json!({"id": id, "inheritsFrom": parent}).to_string()).unwrap();
    };
    write("self", "self");
    write("a", "b");
    write("b", "a");
    let read = |id: &str| read(&mc.join(format!("versions/{id}/{id}.json"))).unwrap();
    assert_eq!(
        resolve(read("self"), &mc).unwrap_err(),
        "self: inheritsFrom loops"
    );
    assert_eq!(
        resolve(read("a"), &mc).unwrap_err(),
        "b: inheritsFrom loops"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_window_size_and_a_server_to_join() {
    let dir = scratch("join");
    let (mc, game, natives, java) = (
        dir.join("mc"),
        dir.join("game"),
        dir.join("natives"),
        dir.join("java"),
    );
    let p = player();
    let launch = |window, server| Launch {
        minecraft: &mc,
        game_dir: &game,
        natives: &natives,
        java: &java,
        memory_mb: 4096,
        jvm_args: &[],
        player: &p,
        os: "linux",
        window,
        server,
    };
    let after = |a: &[String], flag: &str| {
        a.iter()
            .position(|x| x == flag)
            .map(|i| a[i + 1..].iter().take(3).cloned().collect::<Vec<_>>())
    };
    // A modern version asks for both through its rules.
    let a = args(
        &command(
            &modern(),
            &launch(Some((1280, 720)), Some("play.example.net:25570")),
        )
        .unwrap(),
    );
    assert_eq!(
        after(&a, "--width").unwrap()[..3],
        ["1280", "--height", "720"]
    );
    assert_eq!(
        after(&a, "--quickPlayMultiplayer").unwrap()[0],
        "play.example.net:25570"
    );
    assert!(!a.contains(&"--server".into()) && !a.contains(&"--demo".into()));
    // Off unless asked.
    let a = args(&command(&modern(), &launch(None, None)).unwrap());
    assert!(!a.contains(&"--width".into()) && !a.contains(&"--quickPlayMultiplayer".into()));

    // Before 1.13: no rules, the arguments themselves; before 1.20 a server is --server/--port.
    let old = json!({
        "id": "1.12.2", "mainClass": "net.minecraft.client.main.Main", "assetIndex": {"id": "1.12"},
        "minecraftArguments": "--username ${auth_player_name}", "libraries": []
    });
    let a = args(&command(&old, &launch(Some((1280, 720)), Some("mc.example.org"))).unwrap());
    let tail: Vec<&str> = a[a.len() - 8..].iter().map(String::as_str).collect();
    assert_eq!(
        tail,
        [
            "--width",
            "1280",
            "--height",
            "720",
            "--server",
            "mc.example.org",
            "--port",
            "25565"
        ]
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn server_addresses_split_into_host_and_port() {
    assert_eq!(host_and_port("mc.example.org"), ("mc.example.org", 25565));
    assert_eq!(
        host_and_port("mc.example.org:25570"),
        ("mc.example.org", 25570)
    );
    assert_eq!(host_and_port("[2001:db8::1]:25570"), ("2001:db8::1", 25570));
    assert_eq!(host_and_port("2001:db8::1"), ("2001:db8::1", 25565));
    assert_eq!(
        host_and_port("mc.example.org:x"),
        ("mc.example.org:x", 25565)
    );
}

#[test]
fn a_missing_or_damaged_version_file_is_an_error_naming_the_file() {
    let dir = scratch("read-errors");
    fs::create_dir_all(&dir).unwrap();
    let missing = dir.join("none.json");
    let damaged = dir.join("damaged.json");
    fs::write(&damaged, b"{\"id\": \"1.21.4\", \"libraries\": [").unwrap();
    let text = dir.join("bytes.json");
    fs::write(&text, [0xff, 0xfe, 0x00]).unwrap();
    for file in [&missing, &damaged, &text] {
        let e = read(file).unwrap_err();
        assert!(e.starts_with(&file.display().to_string()), "{e}");
    }
    // A file that reads is returned as it is.
    let good = dir.join("good.json");
    fs::write(&good, modern().to_string()).unwrap();
    assert_eq!(read(&good).unwrap(), modern());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_profile_whose_parent_is_missing_or_unsafe_is_refused_before_anything_is_merged() {
    let dir = scratch("parent-errors");
    let mc = dir.join("mc");
    fs::create_dir_all(mc.join("versions")).unwrap();
    let child = |parent: &str| json!({"id": "loader", "inheritsFrom": parent, "libraries": []});
    // The parent's file is not there: the error names the file looked for.
    let e = resolve(child("1.21.4"), &mc).unwrap_err();
    let wanted = under(&mc, "versions/1.21.4/1.21.4.json");
    assert!(e.starts_with(&wanted.display().to_string()), "{e}");
    // A parent id that would leave the versions folder is never read.
    for parent in ["../outside", "a/../../outside", "/etc/passwd", "a\\b"] {
        let e = resolve(child(parent), &mc).unwrap_err();
        assert!(e.starts_with("Unsafe path"), "{parent}: {e}");
    }
    // A parent file that is damaged is an error too, not an empty parent.
    let broken = under(&mc, "versions/broken/broken.json");
    fs::create_dir_all(broken.parent().unwrap()).unwrap();
    fs::write(&broken, b"not json").unwrap();
    let e = resolve(child("broken"), &mc).unwrap_err();
    assert!(e.starts_with(&broken.display().to_string()), "{e}");
    // A version with no parent is returned untouched.
    assert_eq!(resolve(modern(), &mc).unwrap(), modern());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_classpath_refuses_unsafe_paths_and_always_ends_in_the_client_jar() {
    let mc = Path::new("mc");
    // No id: nothing to name the client jar by.
    assert!(classpath(&json!({"libraries": []}), mc, "linux").is_err());
    // No libraries at all: the client jar alone. A library with no artifact (natives only) adds
    // nothing.
    let bare = json!({"id": "1.8.9", "libraries": [{"name": "a:natives:1"}]});
    let only = classpath(&bare, mc, "linux").unwrap();
    assert_eq!(only, [under(mc, "versions/1.8.9/1.8.9.jar")]);
    // A loader's profile plays on `jar`, and the jar is last.
    let loader = json!({
        "id": "loader", "jar": "1.21.4",
        "libraries": [{"downloads": {"artifact": {"path": "a/b/1/b-1.jar"}}}]
    });
    let paths = classpath(&loader, mc, "linux").unwrap();
    assert_eq!(
        paths,
        [
            under(mc, "libraries/a/b/1/b-1.jar"),
            under(mc, "versions/1.21.4/1.21.4.jar")
        ]
    );
    // A library path or a jar name that leaves its folder is an error, not a path.
    for bad in ["../../evil.jar", "/abs/evil.jar", "a\\evil.jar"] {
        let library = json!({
            "id": "1.21.4",
            "libraries": [{"downloads": {"artifact": {"path": bad}}}]
        });
        let e = classpath(&library, mc, "linux").unwrap_err();
        assert!(e.starts_with("Unsafe path"), "{bad}: {e}");
        let jar = json!({"id": "1.21.4", "jar": bad, "libraries": []});
        let e = classpath(&jar, mc, "linux").unwrap_err();
        assert!(e.starts_with("Unsafe path"), "jar {bad}: {e}");
    }
}

/// A version with one library whose osx natives are `path` under libraries/.
fn natives_version(path: &str) -> Value {
    json!({
        "id": "1.8.9",
        "libraries": [{
            "name": "a:b:1", "natives": {"osx": "natives-osx"},
            "extract": {"exclude": ["META-INF/"]},
            "downloads": {"classifiers": {"natives-osx": {"path": path}}}
        }]
    })
}

#[test]
fn natives_that_cannot_be_unpacked_are_an_error_and_nothing_leaves_the_folder() {
    let dir = scratch("natives-errors");
    let (mc, natives) = (dir.join("mc"), dir.join("natives"));
    // The jar as the code joins it: the classifier's path is one piece, so on Windows the string
    // keeps its slash and only a path joined the same way prints the same.
    let jar = mc
        .join("libraries")
        .join(mojang::relative("a/b-natives.jar").unwrap());
    fs::create_dir_all(jar.parent().unwrap()).unwrap();
    let version = natives_version("a/b-natives.jar");
    // The jar is not there: the error names it.
    let e = unpack_natives(&version, &mc, "osx", &natives).unwrap_err();
    assert!(e.starts_with(&jar.display().to_string()), "{e}");
    // The jar is not a zip: the error names it too.
    fs::write(&jar, b"not a zip").unwrap();
    let e = unpack_natives(&version, &mc, "osx", &natives).unwrap_err();
    assert!(e.starts_with(&jar.display().to_string()), "{e}");
    // A classifier path that leaves libraries/ is refused before any file is opened.
    let e = unpack_natives(&natives_version("../../x.jar"), &mc, "osx", &natives).unwrap_err();
    assert!(e.starts_with("Unsafe path"), "{e}");
    // A library the rules or the os do not use needs no jar at all.
    unpack_natives(&version, &mc, "linux", &natives).unwrap();
    // An entry named to climb out of the folder is skipped; the others are unpacked.
    let mut zip = zip::ZipWriter::new(fs::File::create(&jar).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    for (name, body) in [
        ("../evil.dll", b"evil".as_slice()),
        ("sub/../../evil2.dll", b"evil"),
        ("good.dylib", b"good"),
        ("META-INF/MANIFEST.MF", b"m"),
    ] {
        zip.start_file(name, opts).unwrap();
        zip.write_all(body).unwrap();
    }
    zip.finish().unwrap();
    unpack_natives(&version, &mc, "osx", &natives).unwrap();
    assert_eq!(fs::read(natives.join("good.dylib")).unwrap(), b"good");
    assert!(!dir.join("evil.dll").exists() && !dir.join("evil2.dll").exists());
    assert!(!natives.join("META-INF").exists());
    let mut names: Vec<String> = fs::read_dir(&natives)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["good.dylib"]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_asset_layout_of_an_old_version_fails_loudly_and_a_modern_one_copies_nothing() {
    let dir = scratch("assets-errors");
    let (mc, game) = (dir.join("mc"), dir.join("game"));
    let indexes = under(&mc, "assets/indexes");
    fs::create_dir_all(&indexes).unwrap();
    let version = |id: &str| json!({"id": "x", "assetIndex": {"id": id}});
    let hash = "0123456789abcdef0123456789abcdef01234567";
    // No index file: the asset store itself, nothing created.
    assert_eq!(
        game_assets(&version("none"), &mc, &game).unwrap(),
        mc.join("assets")
    );
    assert!(!mc.join("assets/virtual").exists() && !game.exists());
    // A damaged index is an error naming it, not a silent fall back to the store.
    let damaged = indexes.join("damaged.json");
    fs::write(&damaged, b"{\"virtual\": tr").unwrap();
    let e = game_assets(&version("damaged"), &mc, &game).unwrap_err();
    assert!(e.starts_with(&damaged.display().to_string()), "{e}");
    // An index that is neither virtual nor map_to_resources copies nothing.
    fs::write(
        indexes.join("modern.json"),
        json!({"objects": {"a.ogg": {"hash": hash, "size": 1}}}).to_string(),
    )
    .unwrap();
    assert_eq!(
        game_assets(&version("modern"), &mc, &game).unwrap(),
        mc.join("assets")
    );
    assert!(!mc.join("assets/virtual").exists() && !game.exists());
    // A virtual index whose object is not in the store is an error naming the source file.
    fs::write(
        indexes.join("lost.json"),
        json!({"virtual": true, "objects": {"a.ogg": {"hash": hash, "size": 1}}}).to_string(),
    )
    .unwrap();
    let source = under(&mc, "assets/objects/01").join(hash);
    let e = game_assets(&version("lost"), &mc, &game).unwrap_err();
    assert!(e.starts_with(&source.display().to_string()), "{e}");
    // An object name that climbs out of the target is refused, and nothing is written outside.
    fs::write(
        indexes.join("climb.json"),
        json!({"virtual": true, "objects": {"../../evil.ogg": {"hash": hash, "size": 1}}})
            .to_string(),
    )
    .unwrap();
    let e = game_assets(&version("climb"), &mc, &game).unwrap_err();
    assert!(e.starts_with("Unsafe path"), "{e}");
    assert!(!mc.join("assets/evil.ogg").exists() && !mc.join("evil.ogg").exists());
    // An object with a hash that is not 40 characters is skipped, not copied from a made-up path.
    fs::write(
        indexes.join("short.json"),
        json!({"virtual": true, "objects": {"a.ogg": {"hash": "abc", "size": 1}}}).to_string(),
    )
    .unwrap();
    let to = game_assets(&version("short"), &mc, &game).unwrap();
    assert_eq!(to, under(&mc, "assets/virtual/short"));
    assert!(!to.join("a.ogg").exists());
    let _ = fs::remove_dir_all(&dir);
}
