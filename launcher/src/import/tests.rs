use super::*;
use crate::export::{export, Format};
use std::io::Write;
use std::time::Duration;

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "horizon-launcher-import-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn zip_file(path: &Path, files: &[(&str, &[u8])]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, bytes) in files {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

fn put(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// Makes instances as folders of `root`, refusing a name already taken, as the launcher does.
fn maker(root: &Path) -> impl Fn(&str) -> Result<Instance, String> + '_ {
    move |name: &str| {
        let home = root.join(crate::modpacks::slug(name));
        if home.exists() {
            return Err(format!("There is already an instance called {name}."));
        }
        fs::create_dir_all(&home).unwrap();
        Ok(Instance {
            notes: None,
            id: crate::modpacks::slug(name),
            name: name.into(),
            kind: instances::Kind::Own,
            work: home.clone(),
            home,
            icon: None,
            group: None,
            version: None,
            played: Duration::ZERO,
            last_played: None,
            launch: Default::default(),
            java: None,
        })
    }
}

const NONE: &dyn Fn(&str, f32) = &|_, _| {};

/// Fixtures whose index lists no files: a download attempt means the fill went wrong.
const NO_DOWNLOADS: &dyn Fn(&FileRef, &Path, &dyn Fn(f32)) -> Result<(), String> =
    &|_, _, _| panic!("this fixture downloads nothing");

#[test]
fn client_overrides_win_in_either_zip_order_without_path_escape() {
    let root = scratch("overlay-order");
    for reverse in [false, true] {
        let archive = root.join(format!("order-{reverse}.mrpack"));
        let mut entries: Vec<(&str, &[u8])> = vec![
            ("overrides/config/test.txt", b"generic"),
            ("client-overrides/config/test.txt", b"client"),
            ("client-overrides/../escape.txt", b"bad"),
            ("client-overrides/config/../../escape.txt", b"bad"),
        ];
        if reverse {
            entries.reverse();
        }
        zip_file(&archive, &entries);
        let mut zip = open(&archive).unwrap();
        let home = root.join(format!("home-{reverse}"));
        let last_progress = std::cell::Cell::new(0.0);
        fill_with(
            &mut zip,
            Some(&serde_json::json!({"files":[]})),
            &home,
            &|_, value| last_progress.set(value),
            &java_target,
            NO_DOWNLOADS,
            MAX_IMPORT,
        )
        .unwrap();
        assert_eq!(last_progress.get(), 1.0);
        assert_eq!(fs::read(home.join("config/test.txt")).unwrap(), b"client");
        assert!(!root.join("escape.txt").exists());
        assert!(!home.join("escape.txt").exists());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unpack_budget_is_shared_across_override_layers() {
    let root = scratch("overlay-budget");
    let archive = root.join("budget.mrpack");
    zip_file(
        &archive,
        &[
            ("overrides/config/shared.txt", b"1234"),
            ("client-overrides/config/client.txt", b"5678"),
        ],
    );
    let mut zip = open(&archive).unwrap();
    let home = root.join("home");
    let error = fill_with(
        &mut zip,
        Some(&serde_json::json!({"files":[]})),
        &home,
        NONE,
        &java_target,
        NO_DOWNLOADS,
        6,
    )
    .unwrap_err();
    assert!(error.contains("6 bytes"), "{error}");
    assert_eq!(fs::read(home.join("config/shared.txt")).unwrap(), b"1234");
    assert_eq!(fs::read(home.join("config/client.txt")).unwrap(), b"56");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_index_download_never_overwrites_an_override() {
    let root = scratch("download-then-override");
    let index = serde_json::json!({"files":[{
        "path":"config/pack.cfg",
        "downloads":["https://cdn.modrinth.com/data/abcdef/pack.cfg"],
        "fileSize":9,
        "hashes":{"sha512":"00"}
    }]});
    // Rows: the override layers an archive carries, and the bytes that must survive the download.
    for (row, entries, winner) in [
        (
            "both-layers",
            vec![
                ("overrides/config/pack.cfg", &b"generic-override"[..]),
                ("client-overrides/config/pack.cfg", &b"client-override"[..]),
            ],
            &b"client-override"[..],
        ),
        (
            "generic-only",
            vec![("overrides/config/pack.cfg", &b"generic-override"[..])],
            &b"generic-override"[..],
        ),
    ] {
        let archive = root.join(format!("{row}.mrpack"));
        zip_file(&archive, &entries);
        let mut zip = open(&archive).unwrap();
        let home = root.join(format!("home-{row}"));
        // The network, stood in for: bytes that would overwrite the override if downloads ran
        // last. The assertion inside pins the order: no archive layer has landed yet.
        let cdn = &|file: &FileRef, folder: &Path, _: &dyn Fn(f32)| {
            assert!(
                !folder.join(&file.name).exists(),
                "download ran after an archive layer"
            );
            put(&folder.join(&file.name), b"cdn-bytes");
            Ok(())
        };
        fill_with(
            &mut zip,
            Some(&index),
            &home,
            NONE,
            &java_target,
            cdn,
            MAX_IMPORT,
        )
        .unwrap();
        assert_eq!(
            fs::read(home.join("config/pack.cfg")).unwrap(),
            winner,
            "{row}"
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_index_download_falls_back_through_its_mirrors() {
    let root = scratch("mirror-fallback");
    let archive = root.join("pack.mrpack");
    zip_file(&archive, &[]);
    let calls = std::cell::RefCell::new(Vec::new());
    // The network, stood in for: only URLs containing /live/ serve bytes.
    let mirror = &|file: &FileRef, folder: &Path, _: &dyn Fn(f32)| {
        calls.borrow_mut().push(file.url.clone());
        if file.url.contains("/live/") {
            put(&folder.join(&file.name), b"live");
            Ok(())
        } else {
            Err(format!("{} answered 404.", file.url))
        }
    };

    // Row 1: the first mirror is dead, the second serves the file.
    let index = serde_json::json!({"files":[{
        "path":"config/pack.cfg",
        "downloads":["https://cdn.modrinth.com/data/dead/pack.cfg","https://cdn.modrinth.com/data/live/pack.cfg"],
        "fileSize":4,
        "hashes":{"sha512":"00"}
    }]});
    let mut zip = open(&archive).unwrap();
    let home = root.join("home-fallback");
    fill_with(
        &mut zip,
        Some(&index),
        &home,
        NONE,
        &java_target,
        mirror,
        MAX_IMPORT,
    )
    .unwrap();
    assert_eq!(fs::read(home.join("config/pack.cfg")).unwrap(), b"live");
    assert_eq!(
        calls.borrow().as_slice(),
        [
            "https://cdn.modrinth.com/data/dead/pack.cfg",
            "https://cdn.modrinth.com/data/live/pack.cfg"
        ],
        "the dead mirror is tried, then the live one"
    );

    // Row 2: every mirror dead, the error names the file and each mirror's failure.
    let dead = serde_json::json!({"files":[{
        "path":"config/pack.cfg",
        "downloads":["https://cdn.modrinth.com/data/dead-a/pack.cfg","https://cdn.modrinth.com/data/dead-b/pack.cfg"],
        "fileSize":4,
        "hashes":{"sha512":"00"}
    }]});
    let mut zip = open(&archive).unwrap();
    let error = fill_with(
        &mut zip,
        Some(&dead),
        &root.join("home-all-dead"),
        NONE,
        &java_target,
        mirror,
        MAX_IMPORT,
    )
    .unwrap_err();
    assert!(error.contains("config/pack.cfg"), "{error}");
    assert!(error.contains("any of its mirrors"), "{error}");
    assert!(error.contains("dead-a"), "{error}");
    assert!(error.contains("dead-b"), "{error}");

    // Row 3: no URLs at all, the error says so and names the file.
    let empty = serde_json::json!({"files":[{
        "path":"config/pack.cfg",
        "downloads":[],
        "fileSize":4,
        "hashes":{"sha512":"00"}
    }]});
    let mut zip = open(&archive).unwrap();
    let error = fill_with(
        &mut zip,
        Some(&empty),
        &root.join("home-empty"),
        NONE,
        &java_target,
        mirror,
        MAX_IMPORT,
    )
    .unwrap_err();
    assert_eq!(error, "config/pack.cfg lists no download URLs.");

    // Row 4: the first mirror serves, the rest are never fetched.
    let first_wins = serde_json::json!({"files":[{
        "path":"config/first.cfg",
        "downloads":["https://cdn.modrinth.com/data/live/first.cfg","https://cdn.modrinth.com/data/dead-b/first.cfg","https://cdn.modrinth.com/data/dead-c/first.cfg"],
        "fileSize":4,
        "hashes":{"sha512":"00"}
    }]});
    calls.borrow_mut().clear();
    let mut zip = open(&archive).unwrap();
    let home = root.join("home-first-wins");
    fill_with(
        &mut zip,
        Some(&first_wins),
        &home,
        NONE,
        &java_target,
        mirror,
        MAX_IMPORT,
    )
    .unwrap();
    assert_eq!(fs::read(home.join("config/first.cfg")).unwrap(), b"live");
    assert_eq!(
        calls.borrow().as_slice(),
        ["https://cdn.modrinth.com/data/live/first.cfg"],
        "a serving mirror ends the walk: later mirrors stay unfetched"
    );

    // Row 5: blank and non-string entries are skipped, never fetched.
    let messy = serde_json::json!({"files":[{
        "path":"config/messy.cfg",
        "downloads":["", 123, "https://cdn.modrinth.com/data/live/messy.cfg"],
        "fileSize":4,
        "hashes":{"sha512":"00"}
    }]});
    calls.borrow_mut().clear();
    let mut zip = open(&archive).unwrap();
    let home = root.join("home-messy");
    fill_with(
        &mut zip,
        Some(&messy),
        &home,
        NONE,
        &java_target,
        mirror,
        MAX_IMPORT,
    )
    .unwrap();
    assert_eq!(fs::read(home.join("config/messy.cfg")).unwrap(), b"live");
    assert_eq!(
        calls.borrow().as_slice(),
        ["https://cdn.modrinth.com/data/live/messy.cfg"],
        "blank and non-string entries are skipped"
    );

    // Row 6: no downloads key at all behaves like an empty list.
    let absent = serde_json::json!({"files":[{
        "path":"config/pack.cfg",
        "fileSize":4,
        "hashes":{"sha512":"00"}
    }]});
    let mut zip = open(&archive).unwrap();
    let error = fill_with(
        &mut zip,
        Some(&absent),
        &root.join("home-absent"),
        NONE,
        &java_target,
        mirror,
        MAX_IMPORT,
    )
    .unwrap_err();
    assert_eq!(error, "config/pack.cfg lists no download URLs.");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn says_what_a_dropped_file_is() {
    let dir = scratch("what");
    let mrpack = |name: &str, deps: &str| {
        let path = dir.join(name);
        let index = format!(r#"{{"formatVersion":1,"name":"P","files":[],"dependencies":{deps}}}"#);
        zip_file(&path, &[("modrinth.index.json", index.as_bytes())]);
        path
    };
    let cases: Vec<(PathBuf, Result<Dropped, ()>)> = vec![
        (
            mrpack("shared.mrpack", r#"{"minecraft":"26.3"}"#),
            Ok(Dropped::Instance),
        ),
        (
            mrpack(
                "java.mrpack",
                r#"{"minecraft":"1.21.1","fabric-loader":"0.16"}"#,
            ),
            Ok(Dropped::JavaModpack),
        ),
        {
            let p = dir.join("cf.zip");
            zip_file(
                &p,
                &[("manifest.json", b"{}"), ("overrides/config/a.toml", b"")],
            );
            (p, Ok(Dropped::Modpack))
        },
        {
            let p = dir.join("rp.zip");
            zip_file(&p, &[("pack.mcmeta", b"{}"), ("assets/a.png", b"")]);
            (p, Ok(Dropped::Pack(packs::Kind::Resource)))
        },
        {
            let p = dir.join("dp.zip");
            zip_file(
                &p,
                &[
                    ("pack.mcmeta", b"{}"),
                    ("data/a/function/f.mcfunction", b""),
                ],
            );
            (p, Ok(Dropped::DataPack))
        },
        {
            let p = dir.join("sp.zip");
            zip_file(&p, &[("shaders/final.fsh", b"")]);
            (p, Ok(Dropped::Pack(packs::Kind::Shader)))
        },
        {
            let p = dir.join("glow.hshader");
            put(&p, b"PK");
            (p, Ok(Dropped::Pack(packs::Kind::Shader)))
        },
        {
            let p = dir.join("folder.zip");
            zip_file(
                &p,
                &[("options.txt", b"lang:fr_ch\n"), ("saves/W/level.dat", b"")],
            );
            (p, Ok(Dropped::Instance))
        },
        {
            let p = dir.join("random.zip");
            zip_file(&p, &[("readme.txt", b"")]);
            (p, Err(()))
        },
        {
            let p = dir.join("picture.png");
            put(&p, b"png");
            (p, Err(()))
        },
    ];
    for (file, want) in cases {
        assert_eq!(what(&file).map_err(|_| ()), want, "{}", file.display());
    }
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_export_comes_back_as_an_instance_and_nothing_escapes_its_folder() {
    let dir = scratch("roundtrip");
    let home = dir.join("from");
    put(&home.join("options.txt"), b"lang:fr_ch\n");
    put(&home.join("resourcepacks/Glass.zip"), b"glass");
    put(
        &home.join("horizon-mods/coords/horizon-mod.toml"),
        b"id = \"coords\"\n",
    );
    put(&home.join("saves/Base/region/r.0.0.mca"), b"region");
    put(&home.join("saves/Base/level.dat"), b"not nbt");
    put(&home.join("icon.png"), b"the picture");
    put(&home.join("banner.png"), b"the banner");
    let made = dir.join("instances");
    for format in [Format::Mrpack, Format::Zip] {
        let out = dir.join(format!("Shared.{}", format.extension()));
        export(&home, "Shared", format, &["Base".into()], &[], &out).unwrap();
        let i = import_into(&out, NONE, &maker(&made)).unwrap();
        for f in [
            "options.txt",
            "resourcepacks/Glass.zip",
            "horizon-mods/coords/horizon-mod.toml",
            "saves/Base/region/r.0.0.mca",
            "icon.png",
            "banner.png",
        ] {
            assert_eq!(
                fs::read(i.home.join(f)).unwrap(),
                fs::read(home.join(f)).unwrap(),
                "{f}"
            );
        }
    }
    // The .mrpack took the pack's name; the zip came second and got "(2)".
    assert!(made.join("shared").is_dir() && made.join("shared-2").is_dir());

    // A zip that reaches out of its folder, or carries what an instance doesn't keep.
    let evil = dir.join("evil.zip");
    zip_file(
        &evil,
        &[
            ("options.txt", b"ok"),
            ("../outside.txt", b"no"),
            ("saves/../../up.txt", b"no"),
            ("logs/latest.log", b"no"),
            ("session.json", b"no"),
        ],
    );
    let i = import_into(&evil, NONE, &maker(&made)).unwrap();
    assert!(i.home.join("options.txt").is_file());
    assert!(!dir.join("outside.txt").exists() && !made.join("up.txt").exists());
    assert!(!i.home.join("logs").exists() && !i.home.join("session.json").exists());

    // A file to download from anywhere but Modrinth is refused, and the half-made folder goes.
    let index = r#"{"formatVersion":1,"name":"Far","dependencies":{"minecraft":"26.3"},
        "files":[{"path":"resourcepacks/x.zip","hashes":{"sha1":"00","sha512":"00"},
        "downloads":["https://example.com/x.zip"],"fileSize":1}]}"#;
    let far = dir.join("far.mrpack");
    zip_file(&far, &[("modrinth.index.json", index.as_bytes())]);
    let refused = import_into(&far, NONE, &maker(&made)).unwrap_err();
    assert!(refused.contains("resourcepacks/x.zip"), "{refused}");
    assert!(!made.join("far").exists());

    let java = dir.join("java.mrpack");
    zip_file(
        &java,
        &[(
            "modrinth.index.json",
            br#"{"name":"J","files":[],"dependencies":{"neoforge":"21"}}"#,
        )],
    );
    assert!(import_into(&java, NONE, &maker(&made)).is_err());
    fs::remove_dir_all(&dir).unwrap();
}

fn java_maker(
    root: &Path,
) -> impl Fn(&str, &str, crate::loaders::Loader) -> Result<Instance, String> + '_ {
    move |name: &str, minecraft: &str, loader: crate::loaders::Loader| {
        let home = root.join(crate::modpacks::slug(name));
        if home.exists() {
            return Err(format!("There is already an instance called {name}."));
        }
        fs::create_dir_all(&home).unwrap();
        Ok(Instance {
            notes: None,
            id: crate::modpacks::slug(name),
            name: name.into(),
            kind: instances::Kind::Own,
            work: home.clone(),
            home,
            icon: None,
            group: None,
            version: None,
            played: Duration::ZERO,
            last_played: None,
            launch: Default::default(),
            java: Some(instances::JavaGame {
                minecraft: minecraft.into(),
                loader: Some(loader),
                ..Default::default()
            }),
        })
    }
}

fn loader_pack(root: &Path, deps: serde_json::Value, files: serde_json::Value) -> PathBuf {
    let pack = root.join("cool-pack.mrpack");
    let index = serde_json::json!({
        "formatVersion": 1,
        "game": "minecraft",
        "name": "Cool Pack",
        "versionId": "1.0",
        "dependencies": deps,
        "files": files,
    });
    zip_file(
        &pack,
        &[
            ("modrinth.index.json", index.to_string().as_bytes()),
            ("overrides/options.txt", b"gamma:1.0\n"),
            ("overrides/mods/cool.jar", b"PK"),
            ("overrides/../../escape.txt", b"nope"),
            ("overrides/randomfolder/evil.txt", b"not an instance file"),
        ],
    );
    pack
}

#[test]
fn a_loader_mrpack_becomes_a_java_instance_and_nothing_escapes() {
    let root = scratch("java-pack");
    let pack = loader_pack(
        &root,
        serde_json::json!({"minecraft": "26.3", "fabric-loader": "0.20.1"}),
        serde_json::json!([]),
    );
    let made = import_java_instance_with(
        &pack,
        NONE,
        &|mc| Ok(mc.to_string()),
        &|kind, mc, version| {
            assert_eq!((kind, mc, version), ("fabric", "26.3", "0.20.1"));
            Ok(crate::loaders::Loader {
                kind: crate::loaders::Kind::Fabric,
                version: version.into(),
            })
        },
        &java_maker(&root),
    )
    .unwrap();
    assert_eq!(made.name, "Cool Pack");
    let java = made.java.unwrap();
    assert_eq!(java.minecraft, "26.3");
    assert_eq!(java.loader.unwrap().version, "0.20.1");
    assert_eq!(
        fs::read_to_string(made.home.join("options.txt")).unwrap(),
        "gamma:1.0\n"
    );
    assert!(made.home.join("mods/cool.jar").is_file());
    assert!(!root.join("escape.txt").exists() && !root.join("randomfolder").exists());
    assert!(!made.home.join("modrinth.index.json").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_java_pack_file_cant_escape_its_folder() {
    let root = scratch("java-pack-evil");
    let pack = loader_pack(
        &root,
        serde_json::json!({"minecraft": "26.3", "fabric-loader": "0.20.1"}),
        serde_json::json!([{
            "path": "../evil.jar",
            "hashes": {"sha512": "ab".repeat(64), "sha1": "ab".repeat(20)},
            "downloads": ["https://cdn.modrinth.com/data/x/evil.jar"],
            "fileSize": 3
        }]),
    );
    let err = import_java_instance_with(
        &pack,
        NONE,
        &|mc| Ok(mc.to_string()),
        &|_, _, version| {
            Ok(crate::loaders::Loader {
                kind: crate::loaders::Kind::Fabric,
                version: version.into(),
            })
        },
        &java_maker(&root),
    )
    .unwrap_err();
    assert!(err.contains("../evil.jar"), "{err}");
    assert!(!root.join("evil.jar").exists());
    assert!(!root.join("cool-pack").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_forge_pack_says_so_instead_of_importing_broken() {
    let root = scratch("java-pack-forge");
    let pack = loader_pack(
        &root,
        serde_json::json!({"minecraft": "26.3", "neoforge": "21.1.0"}),
        serde_json::json!([]),
    );
    let err = import_java_instance_with(
        &pack,
        NONE,
        &|_| panic!("the version is never asked of Mojang"),
        &|_, _, _| panic!("the loader is never asked of its service"),
        &java_maker(&root),
    )
    .unwrap_err();
    assert!(err.contains("neoforge 21.1.0"), "{err}");
    assert!(err.contains(".mrpack"), "{err}");
    assert!(
        err.contains("importing Forge or NeoForge .mrpack files is not supported"),
        "{err}"
    );
    assert!(
        err.contains("A new Java instance can install neoforge itself"),
        "{err}"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn an_archive_unpacking_past_the_limit_is_refused_mid_fill() {
    let root = scratch("cap");
    let archive = root.join("over.zip");
    zip_file(
        &archive,
        &[("options.txt", b"12345"), ("config/a.txt", b"67890")],
    );
    let mut zip = open(&archive).unwrap();
    let home = root.join("inst");
    // A tiny limit stands in for 8 GiB: the budget is enforced on what actually unpacks,
    // entry by entry, so the second file crosses it.
    let err = fill_with(&mut zip, None, &home, &|_, _| {}, &target, NO_DOWNLOADS, 8).unwrap_err();
    assert!(err.contains("unpacks past"), "{err}");
    // Nothing lands past the budget: all five bytes of the first file, three of the second.
    assert_eq!(fs::metadata(home.join("options.txt")).unwrap().len(), 5);
    assert_eq!(fs::metadata(home.join("config/a.txt")).unwrap().len(), 3);
}

#[test]
fn a_single_huge_entry_is_cut_off_before_it_lands() {
    let root = scratch("cap-huge");
    let archive = root.join("huge.zip");
    let big = vec![b'x'; 100_000];
    zip_file(&archive, &[("options.txt", &big)]);
    let mut zip = open(&archive).unwrap();
    let home = root.join("inst");
    // One entry larger than the whole budget: the copy stops at the remaining budget and
    // one detection byte, never written, refuses the archive.
    let err = fill_with(
        &mut zip,
        None,
        &home,
        &|_, _| {},
        &target,
        NO_DOWNLOADS,
        1_000,
    )
    .unwrap_err();
    assert!(err.contains("unpacks past"), "{err}");
    assert_eq!(fs::metadata(home.join("options.txt")).unwrap().len(), 1_000);
}

#[test]
fn an_archive_within_the_limit_unpacks_all_of_it() {
    let root = scratch("cap-ok");
    let archive = root.join("fine.zip");
    zip_file(
        &archive,
        &[("options.txt", b"12345"), ("config/a.txt", b"67890")],
    );
    let mut zip = open(&archive).unwrap();
    let home = root.join("inst");
    fill_with(&mut zip, None, &home, &|_, _| {}, &target, NO_DOWNLOADS, 10).unwrap();
    assert_eq!(fs::read(home.join("options.txt")).unwrap(), b"12345");
    assert_eq!(fs::read(home.join("config/a.txt")).unwrap(), b"67890");
}

/// Every file under `dir` with its bytes, to compare a folder before and after.
fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
                out.push((rel, fs::read(&p).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out
}

/// What the player's other launcher left: the instance's own data next to the launcher's
/// credentials and sessions, which an import must not carry over.
fn other_launcher_files() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("options.txt", b"lang:fr_ch\n"),
        ("saves/World/level.dat", b"level"),
        ("resourcepacks/Glass.zip", b"glass"),
        ("accounts.json", b"{\"token\":\"secret\"}"),
        ("launcher_profiles.json", b"{\"authenticationDatabase\":{}}"),
        ("msa_token.json", b"msa"),
        ("auth/session.txt", b"session"),
        ("config/../accounts.bak", b"escape"),
        ("instance.cfg", b"[General]\nname=Prism"),
    ]
}

#[test]
fn an_import_leaves_the_original_archive_untouched() {
    let dir = scratch("originals");
    let made = dir.join("instances");
    let zip = dir.join("Other.zip");
    zip_file(&zip, &other_launcher_files());
    let (bytes, modified) = (
        fs::read(&zip).unwrap(),
        fs::metadata(&zip).unwrap().modified().unwrap(),
    );
    let i = import_into(&zip, NONE, &maker(&made)).unwrap();
    assert_eq!(
        fs::read(&zip).unwrap(),
        bytes,
        "the archive's bytes changed"
    );
    assert_eq!(fs::metadata(&zip).unwrap().modified().unwrap(), modified);
    // What was kept is the instance's own data; no credential or session file came along.
    assert!(i.home.join("options.txt").is_file());
    assert!(i.home.join("saves/World/level.dat").is_file());
    for left_out in [
        "accounts.json",
        "launcher_profiles.json",
        "msa_token.json",
        "auth/session.txt",
        "accounts.bak",
        "instance.cfg",
    ] {
        assert!(!i.home.join(left_out).exists(), "{left_out} was imported");
    }
    // A refused archive is just as untouched, and leaves no half-made instance.
    let far = dir.join("far.mrpack");
    let index = r#"{"formatVersion":1,"name":"Far","dependencies":{"minecraft":"26.3"},
        "files":[{"path":"resourcepacks/x.zip","hashes":{"sha1":"00","sha512":"00"},
        "downloads":["https://example.com/x.zip"],"fileSize":1}]}"#;
    zip_file(&far, &[("modrinth.index.json", index.as_bytes())]);
    let far_bytes = fs::read(&far).unwrap();
    assert!(import_into(&far, NONE, &maker(&made)).is_err());
    assert_eq!(fs::read(&far).unwrap(), far_bytes);
    assert!(!made.join("far").exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_export_and_its_import_leave_the_original_instance_folder_untouched() {
    let dir = scratch("export-originals");
    let home = dir.join("from");
    for (name, bytes) in other_launcher_files() {
        if !name.contains("..") {
            put(&home.join(name), bytes);
        }
    }
    let before = snapshot(&home);
    let out = dir.join("Shared.zip");
    export(&home, "Shared", Format::Zip, &["World".into()], &[], &out).unwrap();
    let i = import_into(&out, NONE, &maker(&dir.join("instances"))).unwrap();
    assert_eq!(snapshot(&home), before, "the original folder changed");
    assert!(!i.home.join("accounts.json").exists());
    assert!(!i.home.join("launcher_profiles.json").exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn failed_archive_cleanup_names_the_partial_destination() {
    let dir = scratch("cleanup-error");
    let home = dir.join("partial");
    // A non-directory destination makes real directory cleanup fail on every platform.
    fs::write(&home, b"leftover").unwrap();
    let error = cleanup_failed_import(&home, "archive extraction failed".into());
    assert!(error.contains("archive extraction failed"));
    assert!(error.contains("could not remove partial import"));
    assert!(error.contains(home.to_string_lossy().as_ref()));
    assert_eq!(fs::read(&home).unwrap(), b"leftover");
    fs::remove_file(&home).unwrap();
    fs::create_dir(&home).unwrap();
    fs::write(home.join("partial.txt"), b"part").unwrap();
    assert_eq!(
        cleanup_failed_import(&home, "archive extraction failed".into()),
        "archive extraction failed"
    );
    assert!(!home.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_archive_creation_keeps_the_last_cause_and_bounds_name_retries() {
    let mut tried = Vec::new();
    let error = create_named("Pack", |name| {
        tried.push(name.to_string());
        Err::<(), _>(format!("cannot write {name}: metadata denied"))
    })
    .unwrap_err();
    assert_eq!(tried.len(), 19);
    assert_eq!(tried[0], "Pack");
    assert_eq!(tried[1], "Pack (2)");
    assert_eq!(tried[18], "Pack (19)");
    assert!(error.contains("Could not make an instance called Pack"));
    assert!(error.contains("cannot write Pack (19): metadata denied"));
    let mut tried = Vec::new();
    let made = create_named("Pack", |name| {
        tried.push(name.to_string());
        if name == "Pack (2)" {
            Ok(name.to_string())
        } else {
            Err("name taken".into())
        }
    })
    .unwrap();
    assert_eq!(made, "Pack (2)");
    assert_eq!(tried, ["Pack", "Pack (2)"]);
}

#[test]
fn loader_metadata_failure_cleans_up_the_new_java_instance_or_reports_it() {
    for refuse_cleanup in [false, true] {
        let root = scratch(if refuse_cleanup {
            "loader-cleanup-denied"
        } else {
            "loader-cleanup"
        });
        let made = java_maker(&root)(
            "New pack",
            "1.21.1",
            crate::loaders::Loader {
                kind: crate::loaders::Kind::Fabric,
                version: "0.16.9".into(),
            },
        )
        .unwrap();
        let error = finish_created_instance(&made, Err("loader metadata denied".into()), &|id| {
            assert_eq!(id, made.id);
            if refuse_cleanup {
                Err("Trash denied".into())
            } else {
                fs::remove_dir_all(&made.home).map_err(|e| e.to_string())
            }
        })
        .unwrap_err();
        assert!(error.contains("loader metadata denied"));
        assert_eq!(made.home.exists(), refuse_cleanup);
        if refuse_cleanup {
            assert!(error.contains("Trash denied"));
            assert!(error.contains(&made.id));
        }
        fs::remove_dir_all(root).unwrap();
    }
    let root = scratch("loader-finish-ok");
    let made = java_maker(&root)(
        "Kept pack",
        "1.21.1",
        crate::loaders::Loader {
            kind: crate::loaders::Kind::Fabric,
            version: "0.16.9".into(),
        },
    )
    .unwrap();
    let finished =
        finish_created_instance(&made, Ok(made.clone()), &|_| panic!("success rolled back"))
            .unwrap();
    assert_eq!(finished, made);
    assert!(made.home.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn java_pack_without_minecraft_is_an_error_before_any_service_or_creation() {
    let root = scratch("missing-minecraft");
    let pack = root.join("missing.mrpack");
    zip_file(
        &pack,
        &[(
            "modrinth.index.json",
            br#"{"dependencies":{"fabric-loader":"0.16"},"files":[]}"#,
        )],
    );
    let error = import_java_instance_with(
        &pack,
        &|_, _| {},
        &|_| panic!("must not resolve a missing Minecraft version"),
        &|_, _, _| panic!("must not choose a loader for a malformed index"),
        &|_, _, _| panic!("must not create an instance for a malformed index"),
    )
    .unwrap_err();
    assert!(error.contains("names no Minecraft version"), "{error}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn conflicting_or_malformed_pack_loaders_are_refused_before_resolution() {
    let root = scratch("loader-conflict");
    for (deps, expected) in [
        (
            serde_json::json!({"minecraft":"1.21.1","fabric-loader":"0.16","neoforge":"21.1"}),
            "This .mrpack declares more than one mod loader; no loader was chosen.",
        ),
        (
            serde_json::json!({"minecraft":"1.21.1","fabric-loader":"0.16","quilt-loader":"0.27"}),
            "This .mrpack declares more than one mod loader; no loader was chosen.",
        ),
        (
            serde_json::json!({"minecraft":"1.21.1","fabric-loader":null}),
            "Its modrinth.index.json has no valid version for fabric-loader.",
        ),
        (
            serde_json::json!({"minecraft":"1.21.1","quilt-loader":"   "}),
            "Its modrinth.index.json has no valid version for quilt-loader.",
        ),
        (
            serde_json::json!({"minecraft":"1.21.1","forge":123}),
            "Its modrinth.index.json has no valid version for forge.",
        ),
    ] {
        let pack = loader_pack(&root, deps, serde_json::json!([]));
        let error = import_java_instance_with(
            &pack,
            NONE,
            &|_| panic!("invalid loader reached version resolver"),
            &|_, _, _| panic!("invalid loader reached loader service"),
            &java_maker(&root),
        )
        .unwrap_err();
        assert_eq!(error, expected);
        assert!(!root.join("cool-pack").exists());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn single_supported_loader_keeps_its_declared_version() {
    for (key, kind) in [("fabric-loader", "fabric"), ("quilt-loader", "quilt")] {
        let mut deps = serde_json::Map::new();
        deps.insert(key.into(), serde_json::json!("0.16"));
        assert_eq!(
            pack_loader(&deps).unwrap(),
            Some((kind.into(), "0.16".into()))
        );
    }
    assert_eq!(pack_loader(&serde_json::Map::new()).unwrap(), None);
}
