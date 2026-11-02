use super::*;

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "horizon-launcher-prepared-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn only_a_version_with_its_resources_manifest_counts_as_prepared() {
    let root = scratch("any");
    let versions = root.join("versions");

    // Missing and empty folders, and a stray file: nothing prepared.
    assert!(!any_prepared_in(&versions));
    fs::create_dir_all(&versions).unwrap();
    assert!(!any_prepared_in(&versions));
    fs::write(versions.join("stray"), "x").unwrap();
    assert!(!any_prepared_in(&versions));

    // An interrupted install's partial folder (no manifest yet): still nothing.
    fs::create_dir_all(versions.join(".v1.partial/download")).unwrap();
    assert!(!any_prepared_in(&versions));

    // A version folder without its resources: not prepared.
    let v1 = versions.join("v1");
    fs::create_dir_all(&v1).unwrap();
    assert!(!any_prepared_in(&versions));

    // The manifest lands: prepared.
    let res = v1.join(".local/horizon-client/26.3");
    fs::create_dir_all(&res).unwrap();
    fs::write(v1.join("research-lock.json"), r#"{"minecraft":{"version":"26.3"}}"#).unwrap();
    fs::write(res.join(RESOURCES_MANIFEST), "{}").unwrap();
    assert!(any_prepared_in(&versions));
}
