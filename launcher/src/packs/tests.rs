use super::*;
use std::io::Write;

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "horizon-launcher-packs-{name}-{}",
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

const META: &[u8] = "{\"pack\":{\"pack_format\":69,\"description\":[\"\",{\"text\":\"Smooth \u{a7}aglass\"},\"\\nand more\"]}}"
    .as_bytes();

/// A game folder with three resource packs (one on), and shader packs of both kinds.
fn home(name: &str) -> PathBuf {
    let home = scratch(name);
    let rp = home.join("resourcepacks");
    zip_file(
        &rp.join("Glass.zip"),
        &[("pack.mcmeta", META), ("pack.png", b"png")],
    );
    zip_file(&rp.join("Broken.zip"), &[("readme.txt", b"no meta")]);
    fs::create_dir_all(rp.join("Folder Pack")).unwrap();
    fs::write(
        rp.join("Folder Pack/pack.mcmeta"),
        br#"{"pack":{"description":"Mine"}}"#,
    )
    .unwrap();
    zip_file(&rp.join("Zed.zip"), &[("pack.mcmeta", META)]);
    let sp = home.join("shaderpacks");
    fs::create_dir_all(sp.join("warm")).unwrap();
    fs::write(
        sp.join("warm/horizon-shaders.toml"),
        "schema = 1\nid = \"warm\"\nname = \"Warm Light\"\n",
    )
    .unwrap();
    zip_file(&sp.join("Classic.zip"), &[("shaders/final.fsh", b"")]);
    fs::write(sp.join("notes.txt"), b"").unwrap();
    fs::write(
        home.join("options.txt"),
        "version:4786\nresourcePacks:[\"vanilla\",\"file/Zed.zip\"]\nincompatibleResourcePacks:[\"file/Zed.zip\"]\nlang:fr_ch\n",
    )
    .unwrap();
    fs::write(
        home.join("horizon-graphics.txt"),
        "preset:custom\npickedShaderPack:\nfxaa:true\n",
    )
    .unwrap();
    home
}

#[test]
fn lists_what_the_game_reads_and_which_are_on() {
    let home = home("list");
    let rp = list(&home, Kind::Resource);
    let names: Vec<(&str, bool)> = rp.iter().map(|p| (p.file.as_str(), p.enabled)).collect();
    assert_eq!(
        names,
        [
            ("Folder Pack", false),
            ("Glass.zip", false),
            ("Zed.zip", true)
        ],
        "by name; the pack without pack.mcmeta is left out, as the game does"
    );
    let glass = &rp[1];
    assert_eq!(glass.description, "Smooth glass and more");
    assert_eq!(glass.icon.as_deref(), Some(&b"png"[..]));
    let sp = list(&home, Kind::Shader);
    let names: Vec<&str> = sp.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(names, ["Classic.zip", "Warm Light"]);
    assert!(list(&home.join("none"), Kind::Shader).is_empty());
    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn turning_packs_on_and_off_writes_the_games_own_lines() {
    let home = home("toggle");
    let glass = find(&home, Kind::Resource, "Glass.zip").unwrap();
    set_enabled(&home, &glass, true).unwrap();
    let folder = find(&home, Kind::Resource, "folder pack").unwrap();
    set_enabled(&home, &folder, true).unwrap();
    assert!(
        fs::read_to_string(home.join("options.txt")).unwrap().contains(
            "resourcePacks:[\"vanilla\",\"file/Zed.zip\",\"file/Glass.zip\",\"file/Folder Pack\"]\n"
        ),
        "the newest on top"
    );
    let zed = find(&home, Kind::Resource, "Zed.zip").unwrap();
    set_enabled(&home, &zed, false).unwrap();
    assert_eq!(
        fs::read_to_string(home.join("options.txt")).unwrap(),
        "version:4786\nresourcePacks:[\"vanilla\",\"file/Glass.zip\",\"file/Folder Pack\"]\nincompatibleResourcePacks:[]\nlang:fr_ch\n"
    );

    let warm = find(&home, Kind::Shader, "Warm Light").unwrap();
    set_enabled(&home, &warm, true).unwrap();
    let classic = find(&home, Kind::Shader, "Classic.zip").unwrap();
    set_enabled(&home, &classic, true).unwrap();
    assert_eq!(
        fs::read_to_string(home.join("horizon-graphics.txt")).unwrap(),
        "preset:custom\npickedShaderPack:Classic.zip\nfxaa:true\n",
        "one shader pack at a time"
    );
    set_enabled(
        &home,
        &find(&home, Kind::Shader, "Classic.zip").unwrap(),
        false,
    )
    .unwrap();
    assert!(list(&home, Kind::Shader).iter().all(|p| !p.enabled));

    // A fresh instance has neither file yet.
    let fresh = scratch("fresh");
    fs::create_dir_all(fresh.join("resourcepacks")).unwrap();
    fs::copy(
        home.join("resourcepacks/Zed.zip"),
        fresh.join("resourcepacks/Zed.zip"),
    )
    .unwrap();
    set_enabled(
        &fresh,
        &find(&fresh, Kind::Resource, "Zed.zip").unwrap(),
        true,
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(fresh.join("options.txt")).unwrap(),
        "resourcePacks:[\"vanilla\",\"file/Zed.zip\"]\n"
    );
    fs::remove_dir_all(&home).unwrap();
    fs::remove_dir_all(&fresh).unwrap();
}

#[test]
fn a_dropped_file_goes_to_its_folder_under_a_free_name() {
    let home = home("add");
    let drops = scratch("drops");
    zip_file(&drops.join("Zed.zip"), &[("pack.mcmeta", META)]);
    zip_file(&drops.join("Shiny.zip"), &[("shaders/composite.fsh", b"")]);
    fs::write(drops.join("glow.hshader"), b"PK").unwrap();
    zip_file(&drops.join("random.zip"), &[("a.txt", b"")]);
    fs::write(drops.join("picture.png"), b"").unwrap();

    let added = add(&home, &drops.join("Zed.zip")).unwrap();
    assert_eq!(
        (added.kind, added.file.as_str(), added.enabled),
        (Kind::Resource, "Zed (2).zip", false)
    );
    assert_eq!(
        add(&home, &drops.join("Shiny.zip")).unwrap().kind,
        Kind::Shader
    );
    assert!(home.join("shaderpacks/Shiny.zip").is_file());
    assert_eq!(kind_of(&drops.join("glow.hshader")).unwrap(), Kind::Shader);
    assert!(add(&home, &drops.join("random.zip")).is_err());
    assert!(add(&home, &drops.join("picture.png")).is_err());
    fs::remove_dir_all(&home).unwrap();
    fs::remove_dir_all(&drops).unwrap();
}

#[test]
fn a_pack_toggle_never_rewrites_an_options_file_it_cannot_read() {
    let home = home("toggle-unreadable");
    let glass = find(&home, Kind::Resource, "Glass.zip").unwrap();
    let options = home.join(OPTIONS);
    // A resourcePacks line that is not a list of names, and a file that is not UTF-8: each is an
    // error and the bytes stay as they were.
    let cases: [&[u8]; 4] = [
        b"fov:90\nresourcePacks:[\"vanilla\",\"file/other.zip\",]\nlang:en_us\n",
        b"fov:90\nresourcePacks:\"vanilla\"\n",
        b"fov:90\nresourcePacks:[1,2]\n",
        b"fov:90\n\xFF\xFE\nresourcePacks:[\"vanilla\"]\n",
    ];
    for bytes in cases {
        fs::write(&options, bytes).unwrap();
        assert!(set_enabled(&home, &glass, true).is_err(), "{bytes:?}");
        assert!(set_enabled(&home, &glass, false).is_err(), "{bytes:?}");
        assert_eq!(fs::read(&options).unwrap(), bytes);
    }
    // The same file is safe for a shader pack's own file, and a missing options file is empty.
    fs::remove_file(&options).unwrap();
    set_enabled(&home, &glass, true).unwrap();
    assert_eq!(
        fs::read_to_string(&options).unwrap(),
        "resourcePacks:[\"vanilla\",\"file/Glass.zip\"]\n"
    );
    // Non-UTF-8 in the shader settings file is refused the same way.
    let graphics = home.join(GRAPHICS);
    fs::write(&graphics, b"a:1\n\xFF\n").unwrap();
    let shader = find(&home, Kind::Shader, "Warm Light").unwrap();
    assert!(set_enabled(&home, &shader, true).is_err());
    assert_eq!(fs::read(&graphics).unwrap(), b"a:1\n\xFF\n");
    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn toggling_packs_replaces_the_settings_files_whole_and_leaves_no_partial_file() {
    let home = home("toggle-whole");
    let names = |dir: &Path| -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    };
    let before = names(&home);
    let glass = find(&home, Kind::Resource, "Glass.zip").unwrap();
    set_enabled(&home, &glass, true).unwrap();
    set_enabled(&home, &glass, false).unwrap();
    let warm = find(&home, Kind::Shader, "Warm Light").unwrap();
    set_enabled(&home, &warm, true).unwrap();
    set_enabled(&home, &warm, false).unwrap();
    assert_eq!(names(&home), before, "a partial or extra file was left");
    // The other lines of both files are still there.
    let options = fs::read_to_string(home.join(OPTIONS)).unwrap();
    assert!(options.contains("lang:fr_ch\n") && options.contains("version:4786\n"));
    let graphics = fs::read_to_string(home.join(GRAPHICS)).unwrap();
    assert!(graphics.contains("preset:custom\n") && graphics.contains("fxaa:true\n"));
    fs::remove_dir_all(&home).unwrap();
}
