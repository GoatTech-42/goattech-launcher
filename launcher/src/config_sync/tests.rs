use super::*;

fn scratch(name: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "horizon-launcher-sync-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let data = root.join("data");
    let a = root.join("a");
    let b = root.join("b");
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    (root, data, a, b)
}

fn put(home: &Path, text: &str) {
    fs::write(home.join(OPTIONS_FILE), text).unwrap();
}

fn get(home: &Path) -> String {
    fs::read_to_string(home.join(OPTIONS_FILE)).unwrap()
}

fn backups(dir: &Path) -> Vec<PathBuf> {
    // Sorted: the zero-padded names order by creation time, oldest first.
    let mut list: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .contains(BACKUP_INFIX)
        })
        .collect();
    list.sort();
    list
}

#[test]
fn a_new_group_seeds_from_the_joining_instance_without_touching_it() {
    let (root, data, a, _b) = scratch("seed");
    put(&a, "version:3465\nfov:90\nkey_key.jump:32\n");
    let before = get(&a);
    let (m, report) = join(&data, &a, "main", None).unwrap();
    assert!(report.seeded);
    assert_eq!(get(&a), before, "the seeding instance's file is untouched");
    let shared = read_shared(&data, "main").unwrap();
    assert_eq!(shared.get("fov").unwrap(), b"90");
    assert_eq!(
        shared.get("version").unwrap(),
        b"3465",
        "the marker rides in the shared file, so format and values are one atomic write"
    );
    assert_eq!(group_format(&data, "main").unwrap().unwrap(), b"3465");
    assert_eq!(
        m.shared.len(),
        2,
        "the marker never enters baselines or syncs"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn joining_with_differences_needs_a_chosen_side_and_writes_nothing_without_one() {
    let (root, data, a, b) = scratch("joinpick");
    put(&a, "version:3465\nfov:90\n");
    let (_m_a, _) = join(&data, &a, "main", None).unwrap();
    put(&b, "version:3465\nfov:70\nrenderDistance:12\n");
    let before = get(&b);
    let err = join(&data, &b, "main", None).unwrap_err();
    assert!(err.contains("fov"), "names the differing settings: {err}");
    assert!(err.contains("--take"), "says how to choose: {err}");
    assert_eq!(get(&b), before, "nothing written without a choice");
    assert!(backups(&b).is_empty());

    // Choosing the group's side: backed up, only the differing setting moves, and
    // settings the file doesn't have are never added.
    let (_m_b, report) = join(&data, &b, "main", Some(Side::Theirs)).unwrap();
    assert_eq!(report.pulled, vec!["fov".to_string()]);
    let text = get(&b);
    assert!(text.contains("fov:90"));
    assert!(
        text.contains("renderDistance:12"),
        "untouched keys keep their bytes"
    );
    assert!(!text.contains("key_key"), "group-only keys are never added");
    assert_eq!(backups(&b).len(), 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn choosing_my_side_moves_my_values_into_the_group_with_a_backup() {
    let (root, data, a, b) = scratch("takemine");
    put(&a, "version:3465\nfov:90\n");
    let (_m_a, _) = join(&data, &a, "main", None).unwrap();
    put(&b, "version:3465\nfov:70\n");
    let (_m_b, report) = join(&data, &b, "main", Some(Side::Mine)).unwrap();
    assert_eq!(report.pushed, vec!["fov".to_string()]);
    assert_eq!(
        read_shared(&data, "main").unwrap().get("fov").unwrap(),
        b"70"
    );
    let group_backups = backups(&data.join("sync").join("main"));
    assert_eq!(group_backups.len(), 1, "the group's file was backed up too");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn different_options_formats_never_exchange_values() {
    let (root, data, a, b) = scratch("formats");
    put(&a, "version:1343\nkey_key.jump:32\n");
    let (_m_a, _) = join(&data, &a, "main", None).unwrap();
    put(&b, "version:3465\nkey_key.jump:key.keyboard.space\n");
    let before = get(&b);
    let err = join(&data, &b, "main", Some(Side::Theirs)).unwrap_err();
    assert!(err.contains("formats differ"), "{err}");
    assert_eq!(
        get(&b),
        before,
        "a format mismatch writes nothing, even with --take"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn edits_push_after_a_clean_exit_and_pull_per_setting() {
    let (root, data, a, b) = scratch("twoway");
    put(&a, "version:3465\nfov:90\n");
    let (mut m_a, _) = join(&data, &a, "main", None).unwrap();
    put(&b, "version:3465\nfov:90\n");
    let (mut m_b, _) = join(&data, &b, "main", None).unwrap();

    put(&a, "version:3465\nfov:100\n");
    let report = sync(&data, &a, "a", &mut m_a, Pass::CleanExit).unwrap();
    assert_eq!(report.pushed, vec!["fov".to_string()]);
    let report = sync(&data, &b, "b", &mut m_b, Pass::PreLaunch).unwrap();
    assert_eq!(report.pulled, vec!["fov".to_string()]);
    assert_eq!(get(&b), "version:3465\nfov:100\n");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_prelaunch_pass_never_pushes_a_torn_crashed_file() {
    let (root, data, a, _b) = scratch("torn");
    put(&a, "version:3465\nfov:90\nkey_key.jump:32\n");
    let (mut m_a, _) = join(&data, &a, "main", None).unwrap();
    // A crash leaves a valid-looking partial write: one real setting truncated.
    put(&a, "version:3465\nfov:9\n");
    let report = sync(&data, &a, "a", &mut m_a, Pass::PreLaunch).unwrap();
    assert!(
        report.pushed.is_empty(),
        "pre-launch never pushes: {report:?}"
    );
    let shared = read_shared(&data, "main").unwrap();
    assert_eq!(
        shared.get("fov").unwrap(),
        b"90",
        "the torn value stayed local"
    );
    assert_eq!(
        shared.get("key_key.jump").unwrap(),
        b"32",
        "a vanished key deletes nothing"
    );
    // And an empty crashed file is just as safe.
    put(&a, "");
    let report = sync(&data, &a, "a", &mut m_a, Pass::PreLaunch).unwrap();
    assert!(report.pushed.is_empty());
    assert_eq!(
        read_shared(&data, "main").unwrap().get("fov").unwrap(),
        b"90"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_fresh_default_never_overwrites_the_groups_tuned_value() {
    let (root, data, a, b) = scratch("freshdefault");
    put(&a, "version:3465\nfov:90\n");
    let (mut m_a, _) = join(&data, &a, "main", None).unwrap();
    put(&b, "version:3465\nfov:90\n");
    let (mut m_b, _) = join(&data, &b, "main", None).unwrap();
    // A tunes a mod setting; it pushes. B installs the mod later: its file gains the key
    // with a default. The group's tuned value lands instead of the default pushing up.
    put(&a, "version:3465\nfov:90\nmodsetting:tuned\n");
    sync(&data, &a, "a", &mut m_a, Pass::CleanExit).unwrap();
    put(&b, "version:3465\nfov:90\nmodsetting:default\n");
    let report = sync(&data, &b, "b", &mut m_b, Pass::CleanExit).unwrap();
    assert_eq!(report.pulled, vec!["modsetting".to_string()], "{report:?}");
    assert!(report.pushed.is_empty());
    assert_eq!(
        read_shared(&data, "main")
            .unwrap()
            .get("modsetting")
            .unwrap(),
        b"tuned"
    );
    assert!(get(&b).contains("modsetting:tuned"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn conflicts_are_held_per_instance_and_only_the_owner_resolves() {
    let (root, data, a, b) = scratch("ownconflict");
    put(&a, "version:3465\nfov:90\n");
    let (mut m_a, _) = join(&data, &a, "main", None).unwrap();
    put(&b, "version:3465\nfov:90\n");
    let (mut m_b, _) = join(&data, &b, "main", None).unwrap();

    put(&a, "version:3465\nfov:100\n");
    sync(&data, &a, "a", &mut m_a, Pass::CleanExit).unwrap();
    put(&b, "version:3465\nfov:70\n");
    let report = sync(&data, &b, "b", &mut m_b, Pass::CleanExit).unwrap();
    assert_eq!(report.conflicts.len(), 1);

    // A has no conflict of its own and cannot clear B's.
    assert!(conflicts(&data, "main", "a").unwrap().is_empty());
    assert!(resolve(&data, &a, "a", &mut m_a, "fov", false).is_err());
    assert_eq!(
        conflicts(&data, "main", "b").unwrap().len(),
        1,
        "B's conflict intact"
    );

    let done = resolve(&data, &b, "b", &mut m_b, "fov", false).unwrap();
    assert_eq!(done, vec!["fov".to_string()]);
    assert!(get(&b).contains("fov:100"));
    assert!(conflicts(&data, "main", "b").unwrap().is_empty());
    let report = sync(&data, &b, "b", &mut m_b, Pass::CleanExit).unwrap();
    assert!(report.conflicts.is_empty() && report.pulled.is_empty() && report.pushed.is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn untouched_lines_keep_their_exact_bytes() {
    let (root, data, a, b) = scratch("bytesexact");
    put(&a, "version:3465\nfov:90\n");
    let (mut m_a, _) = join(&data, &a, "main", None).unwrap();
    // The group moves fov to 100 through A's clean exit.
    put(&a, "version:3465\nfov:100\n");
    sync(&data, &a, "a", &mut m_a, Pass::CleanExit).unwrap();
    // B's file has CRLFs, a blank line, a colon-less line and no final newline.
    fs::write(
        b.join(OPTIONS_FILE),
        b"version:3465\r\nfov:70\r\n\r\nweird line without colon\r\nlast:no-newline",
    )
    .unwrap();
    let (_m_b, report) = join(&data, &b, "main", Some(Side::Theirs)).unwrap();
    assert_eq!(report.pulled, vec!["fov".to_string()]);
    let bytes = fs::read(b.join(OPTIONS_FILE)).unwrap();
    let expected: &[u8] =
        b"version:3465\r\nfov:100\r\n\r\nweird line without colon\r\nlast:no-newline";
    assert_eq!(bytes, expected, "only the changed setting's value moved");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn two_backups_in_one_second_never_overwrite() {
    let (root, data, a, _b) = scratch("backuprace");
    put(&a, "version:3465\nfov:90\n");
    let path = a.join(OPTIONS_FILE);
    backup(&path).unwrap();
    put(&a, "version:3465\nfov:91\n");
    backup(&path).unwrap();
    let list = backups(&a);
    assert_eq!(list.len(), 2, "unique names: {list:?}");
    let first = fs::read(&list[0]).unwrap();
    assert_eq!(first, b"version:3465\nfov:90\n");
    let _ = data;
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_foreign_file_is_never_rewritten() {
    let (root, data, a, _b) = scratch("foreign");
    put(&a, "version:3465\nfov:90\n");
    let (mut m_a, _) = join(&data, &a, "main", None).unwrap();
    fs::write(a.join(OPTIONS_FILE), b"\xff\xfe not options at all").unwrap();
    let before = fs::read(a.join(OPTIONS_FILE)).unwrap();
    assert!(sync(&data, &a, "a", &mut m_a, Pass::Manual).is_err());
    assert_eq!(fs::read(a.join(OPTIONS_FILE)).unwrap(), before, "untouched");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn non_utf8_values_round_trip_byte_exact() {
    let (root, data, a, b) = scratch("nonutf8");
    fs::write(
        a.join(OPTIONS_FILE),
        b"version:3465\nfov:90\nodd:\xc3\x28\n",
    )
    .unwrap();
    let (m_a, _) = join(&data, &a, "main", None).unwrap();
    assert!(matches!(m_a.shared.get("odd"), Some(Bytes::Hex { .. })));
    fs::write(b.join(OPTIONS_FILE), b"version:3465\nodd:plain\n").unwrap();
    let (_m_b, _) = join(&data, &b, "main", Some(Side::Theirs)).unwrap();
    let bytes = fs::read(b.join(OPTIONS_FILE)).unwrap();
    assert!(
        bytes.windows(6).any(|w| w == b"odd:\xc3\x28"),
        "raw bytes landed: {bytes:?}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn corrupt_conflict_metadata_is_an_error_not_a_silent_reset() {
    let (root, data, a, _b) = scratch("corruptmeta");
    put(&a, "version:3465\nfov:90\n");
    let (_m_a, _) = join(&data, &a, "main", None).unwrap();
    let dir = data.join("sync").join("main");
    fs::write(dir.join("conflicts.json"), "{ not json").unwrap();
    assert!(conflicts(&data, "main", "a").is_err());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn group_names_stay_inside_the_sync_folder() {
    let (root, data, a, _b) = scratch("names");
    for bad in ["../escape", "a/b", "", ".hidden"] {
        assert!(join(&data, &a, bad, None).is_err(), "{bad:?} refused");
    }
    assert!(join(&data, &a, "my group-1", None).is_ok());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_setting_new_to_the_file_moves_through_the_real_pass_gate() {
    let (root, data, a, _b) = scratch("newkey");
    put(&a, "version:3465\nfov:90\n");
    let (mut m, _) = join(&data, &a, "main", None).unwrap();
    assert!(!pending(&data, &a, &m), "quiet right after joining");
    // A mod install writes its own line into the file.
    put(&a, "version:3465\nfov:90\nmodsetting:tuned\n");
    assert!(
        pending(&data, &a, &m),
        "a brand-new local setting is noticed"
    );
    let report = sync(&data, &a, "a", &mut m, Pass::Manual).unwrap();
    assert_eq!(report.pushed, vec!["modsetting".to_string()]);
    assert_eq!(
        read_shared(&data, "main")
            .unwrap()
            .get("modsetting")
            .unwrap(),
        b"tuned"
    );
    assert!(!pending(&data, &a, &m), "settled after the pass");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_prelaunch_pass_defers_a_local_edit_until_a_trusted_pass() {
    let (root, data, a, _b) = scratch("defer");
    put(&a, "version:3465\nfov:90\n");
    let (mut m, _) = join(&data, &a, "main", None).unwrap();
    // The player tunes fov; the game crashes before it can exit cleanly.
    put(&a, "version:3465\nfov:100\n");
    let r = sync(&data, &a, "a", &mut m, Pass::PreLaunch).unwrap();
    assert!(r.pushed.is_empty() && r.pulled.is_empty());
    assert_eq!(
        get(&a),
        "version:3465\nfov:100\n",
        "a pre-launch pass never overwrites a local edit"
    );
    assert_eq!(
        read_shared(&data, "main").unwrap().get("fov").unwrap(),
        b"90",
        "and never pushes it"
    );
    assert!(pending(&data, &a, &m), "the edit stays pending");
    let r = sync(&data, &a, "a", &mut m, Pass::CleanExit).unwrap();
    assert_eq!(r.pushed, vec!["fov".to_string()]);
    assert_eq!(
        read_shared(&data, "main").unwrap().get("fov").unwrap(),
        b"100"
    );
    assert!(!pending(&data, &a, &m), "settled after the trusted pass");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_torn_file_defers_then_the_games_rewrite_syncs() {
    let (root, data, a, _b) = scratch("torndefer");
    put(&a, "version:3465\nfov:90\nkey_key.jump:32\n");
    let (mut m, _) = join(&data, &a, "main", None).unwrap();
    // A crash leaves a torn file; the next launch's pre-launch pass must not trust it.
    put(&a, "version:3465\nfov:9");
    let r = sync(&data, &a, "a", &mut m, Pass::PreLaunch).unwrap();
    assert!(r.pushed.is_empty() && r.conflicts.is_empty());
    assert_eq!(
        read_shared(&data, "main").unwrap().get("fov").unwrap(),
        b"90"
    );
    assert!(
        pending(&data, &a, &m),
        "torn bytes stay pending, not adopted"
    );
    // The game starts and rewrites the file in full with the real values.
    put(&a, "version:3465\nfov:100\nkey_key.jump:32\n");
    let r = sync(&data, &a, "a", &mut m, Pass::CleanExit).unwrap();
    assert_eq!(r.pushed, vec!["fov".to_string()]);
    assert_eq!(
        read_shared(&data, "main").unwrap().get("fov").unwrap(),
        b"100"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn an_unknown_local_format_is_never_compatible() {
    let (root, data, a, b) = scratch("unknownfmt");
    put(&a, "version:1343\nkey_key.jump:32\n");
    let (_m, _) = join(&data, &a, "main", None).unwrap();
    // A file with no marker at all: its format is unknown, and unknown is not compatible.
    put(&b, "key_key.jump:key.keyboard.space\n");
    let err = join(&data, &b, "main", Some(Side::Theirs)).unwrap_err();
    assert!(err.contains("no `version` marker"), "{err}");
    assert_eq!(
        get(&b),
        "key_key.jump:key.keyboard.space\n",
        "nothing was written"
    );
    assert_eq!(
        read_shared(&data, "main")
            .unwrap()
            .get("key_key.jump")
            .unwrap(),
        b"32"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn resolve_checks_the_format_every_time() {
    let (root, data, a, _b) = scratch("resolvegate");
    put(&a, "version:1343\nfov:90\n");
    let (mut m, _) = join(&data, &a, "main", None).unwrap();
    // Both sides move: a conflict is recorded while the formats match.
    fs::write(
        shared_file(&data, "main").unwrap(),
        b"version:1343\nfov:80\n",
    )
    .unwrap();
    put(&a, "version:1343\nfov:100\n");
    let r = sync(&data, &a, "a", &mut m, Pass::Manual).unwrap();
    assert_eq!(r.conflicts.len(), 1);
    // The instance's game updates to a new format before the conflict is settled.
    put(&a, "version:3465\nfov:100\n");
    let err = resolve(&data, &a, "a", &mut m, "fov", false).unwrap_err();
    assert!(err.contains("formats differ"), "{err}");
    assert_eq!(get(&a), "version:3465\nfov:100\n", "nothing was written");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_held_lock_blocks_a_second_writer_until_dropped() {
    let (root, data, _a, _b) = scratch("lock");
    let one = GroupLock::take(&data, "main").unwrap();
    let err = GroupLock::take(&data, "main").unwrap_err();
    // Wording differs per platform (unix advisory lock vs token lock); the contract is
    // that the second writer wrote nothing.
    assert!(err.contains("nothing was written"), "{err}");
    drop(one);
    let _two = GroupLock::take(&data, "main").unwrap();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn corrupt_baselines_stop_the_pass_before_any_write() {
    let (root, data, a, _b) = scratch("corruptbase");
    put(&a, "version:3465\nfov:90\n");
    let (mut m, _) = join(&data, &a, "main", None).unwrap();
    put(&a, "version:3465\nfov:100\n");
    let group_before = fs::read(shared_file(&data, "main").unwrap()).unwrap();
    m.local
        .insert("fov".to_string(), Bytes::Hex { hex: "zz".into() });
    let err = sync(&data, &a, "a", &mut m, Pass::Manual).unwrap_err();
    assert!(err.contains("corrupt"), "{err}");
    assert_eq!(
        fs::read(shared_file(&data, "main").unwrap()).unwrap(),
        group_before,
        "the group file is untouched"
    );
    assert_eq!(
        get(&a),
        "version:3465\nfov:100\n",
        "the local file is untouched"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_running_game_refuses_the_pass_until_it_releases() {
    let (root, data, a, _b) = scratch("running");
    put(&a, "version:3465\nfov:90\n");
    let (mut m, _) = join(&data, &a, "main", None).unwrap();
    put(&a, "version:3465\nfov:100\n");
    let mark = claim_running(&a).unwrap().expect("no mark held yet");
    // A live mark is never taken over: a second run gets no claim.
    assert!(
        claim_running(&a).unwrap().is_none(),
        "no takeover of a live mark"
    );
    let err = sync(&data, &a, "a", &mut m, Pass::Manual).unwrap_err();
    assert!(err.contains("running"), "{err}");
    assert_eq!(get(&a), "version:3465\nfov:100\n", "nothing was written");
    mark.release().unwrap();
    assert!(!a.join(RUNNING_FILE).exists(), "the mark is gone at exit");
    // The pass runs once the game is done.
    let r = sync(&data, &a, "a", &mut m, Pass::Manual).unwrap();
    assert_eq!(r.pushed, vec!["fov".to_string()]);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_released_mark_is_never_taken_by_someone_elses_token() {
    let (root, _data, a, _b) = scratch("markowner");
    let mark = claim_running(&a).unwrap().expect("no mark held yet");
    // Another run's token never deletes this run's mark, and vice versa.
    fs::write(a.join(RUNNING_FILE), b"another-run 1 2").unwrap();
    mark.release().unwrap();
    assert_eq!(
        fs::read(a.join(RUNNING_FILE)).unwrap(),
        b"another-run 1 2",
        "a foreign mark survives this run's release"
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn a_dead_launcher_with_a_live_game_still_counts_as_running() {
    let (root, _data, a, _b) = scratch("childlives");
    // Launcher pid dead, game pid = this test process (alive): still running.
    fs::write(
        a.join(RUNNING_FILE),
        format!("token 99999999 {}", std::process::id()),
    )
    .unwrap();
    let err = guard_not_running(&a).unwrap_err();
    assert!(err.contains("running"), "{err}");
    // Both pids dead: the crashed launcher's leftover is cleaned up, not a permanent block.
    fs::write(a.join(RUNNING_FILE), b"token 99999998 99999999").unwrap();
    guard_not_running(&a).unwrap();
    assert!(
        !a.join(RUNNING_FILE).exists(),
        "the stale mark was cleaned up"
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(not(unix))]
#[test]
fn without_a_liveness_check_every_mark_counts_as_running() {
    let (root, _data, a, _b) = scratch("nonunixmark");
    fs::write(a.join(RUNNING_FILE), b"token 1 2").unwrap();
    assert!(
        guard_not_running(&a).is_err(),
        "conservative: no liveness check means no stale guessing"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_token_lock_is_exclusive_and_only_its_owner_releases_it() {
    let (root, _data, a, _b) = scratch("tokenlock");
    let path = a.join(".lock");
    let lock = TokenLock::take(path.clone()).unwrap();
    let err = TokenLock::take(path.clone()).unwrap_err();
    assert!(err.contains("held"), "{err}");
    // A token that is no longer ours is never deleted on drop.
    fs::write(&path, b"another-writer").unwrap();
    drop(lock);
    assert_eq!(fs::read(&path).unwrap(), b"another-writer");
    fs::remove_file(&path).unwrap(); // that writer finished and released its own token
                                     // A token that IS ours is released on drop.
    let lock = TokenLock::take(path.clone()).unwrap();
    drop(lock);
    assert!(!path.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_fresh_join_writes_nothing_until_the_games_first_run_establishes_the_file() {
    let (root, data, a, b) = scratch("freshseed");
    put(&a, "version:3465\nfov:90\n");
    let (_m, _) = join(&data, &a, "main", None).unwrap();
    // A brand-new instance has no file at all: the join writes NOTHING into its game
    // folder. Values copied before the game's own format is known could be settings the
    // game misreads on its first read.
    let (mut m_b, report) = join(&data, &b, "main", None).unwrap();
    assert!(
        !b.join(OPTIONS_FILE).exists(),
        "no options payload before the game's first run"
    );
    assert!(report.pulled.is_empty(), "{report:?}");
    assert!(
        report.notes.iter().any(|n| n.contains("first run")),
        "{report:?}"
    );
    // The game's first run creates the file with its own marker and defaults; the group's
    // standing values land on the next pass, never the fresh defaults upward.
    put(&b, "version:3465\nfov:70\n");
    let r = sync(&data, &b, "b", &mut m_b, Pass::Manual).unwrap();
    assert_eq!(r.pulled, vec!["fov".to_string()], "{r:?}");
    assert_eq!(get(&b), "version:3465\nfov:90\n");
    // A game on another format writes a different marker: the pass holds, nothing moves.
    put(&b, "version:9999\nfov:90\n");
    let group_before = fs::read(shared_file(&data, "main").unwrap()).unwrap();
    let r = sync(&data, &b, "b", &mut m_b, Pass::Manual).unwrap();
    assert!(
        r.notes.iter().any(|n| n.contains("formats differ")),
        "{r:?}"
    );
    assert!(r.pushed.is_empty() && r.pulled.is_empty());
    assert_eq!(
        fs::read(shared_file(&data, "main").unwrap()).unwrap(),
        group_before
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn an_unknown_format_on_both_sides_is_still_unknown_and_refused() {
    let (root, data, _a, b) = scratch("unknownboth");
    // A group with values but no format marker (hand-written, or an older tool).
    fs::create_dir_all(group_dir(&data, "main").unwrap()).unwrap();
    write_shared(
        &data,
        "main",
        &BTreeMap::from([("fov".to_string(), b"90".to_vec())]),
    )
    .unwrap();
    put(&b, "fov:90\n"); // a real file whose game never wrote a marker
    let err = join(&data, &b, "main", None).unwrap_err();
    assert!(err.contains("format"), "{err}");
    assert_eq!(get(&b), "fov:90\n", "nothing was written");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_joining_real_file_establishes_the_group_by_replacing_untrusted_bytes() {
    let (root, data, _a, b) = scratch("establish");
    // A group with values but no format marker (hand-written, or seeded from a file whose
    // game never ran): those bytes are untrusted, never relabelled compatible.
    fs::create_dir_all(group_dir(&data, "main").unwrap()).unwrap();
    write_shared(
        &data,
        "main",
        &BTreeMap::from([
            ("fov".to_string(), b"90".to_vec()),
            ("key_key.jump".to_string(), b"32".to_vec()),
        ]),
    )
    .unwrap();
    put(&b, "version:3465\nkey.keyboard.jump:57\nfov:70\n");
    let (_m, report) = join(&data, &b, "main", Some(Side::Theirs)).unwrap();
    assert!(
        report.notes.iter().any(|n| n.contains("now established")),
        "{report:?}"
    );
    // The group now holds exactly the known-format source's settings; the replaced file
    // was backed up, and the joining file is untouched (nothing differed from itself).
    let shared = read_shared(&data, "main").unwrap();
    assert_eq!(shared.get("version").unwrap(), b"3465");
    assert_eq!(shared.get("fov").unwrap(), b"70");
    assert!(
        !shared.contains_key("key_key.jump"),
        "untrusted bytes dropped"
    );
    assert_eq!(shared.get("key.keyboard.jump").unwrap(), b"57");
    let dir = group_dir(&data, "main").unwrap();
    assert_eq!(
        backups(&dir).len(),
        1,
        "the replaced group file was backed up"
    );
    assert_eq!(get(&b), "version:3465\nkey.keyboard.jump:57\nfov:70\n");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn join_refuses_to_write_when_the_conflict_record_is_corrupt() {
    let (root, data, a, b) = scratch("joincorrupt");
    put(&a, "version:3465\nfov:90\n");
    let (_m, _) = join(&data, &a, "main", None).unwrap();
    fs::write(conflicts_file(&data, "main").unwrap(), b"not json").unwrap();
    put(&b, "version:3465\nfov:70\n");
    let group_before = fs::read(shared_file(&data, "main").unwrap()).unwrap();
    let err = join(&data, &b, "main", Some(Side::Theirs)).unwrap_err();
    assert!(err.contains("conflict"), "{err}");
    assert_eq!(get(&b), "version:3465\nfov:70\n", "nothing was written");
    assert_eq!(
        fs::read(shared_file(&data, "main").unwrap()).unwrap(),
        group_before,
        "the group file is untouched either - validation precedes every write"
    );
    assert!(backups(&b).is_empty(), "no backup was made either");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn resolve_validates_everything_before_any_write() {
    let (root, data, a, _b) = scratch("resolvepre");
    put(&a, "version:3465\nfov:90\n");
    let (mut m, _) = join(&data, &a, "main", None).unwrap();
    // A corrupt remembered baseline errors before anything is written or resolved.
    m.shared.insert(
        "fov".to_string(),
        Bytes::Hex {
            hex: "zz".to_string(),
        },
    );
    let group_before = fs::read(shared_file(&data, "main").unwrap()).unwrap();
    let file_before = get(&a);
    let err = resolve(&data, &a, "a", &mut m, "fov", true).unwrap_err();
    assert!(err.contains("corrupt"), "{err}");
    assert_eq!(
        fs::read(shared_file(&data, "main").unwrap()).unwrap(),
        group_before
    );
    assert_eq!(get(&a), file_before);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn set_child_records_the_pid_and_fails_closed_when_the_mark_is_gone_or_foreign() {
    let (root, _data, a, _b) = scratch("childrecord");
    let mark = claim_running(&a).unwrap().expect("no mark held yet");
    // Positive: the pid is recorded in the live mark.
    mark.set_child(std::process::id()).unwrap();
    let text = fs::read_to_string(a.join(RUNNING_FILE)).unwrap();
    assert_eq!(text.split_whitespace().count(), 3, "{text}");
    assert!(
        text.ends_with(&std::process::id().to_string()),
        "child pid recorded: {text}"
    );
    // A mark that disappeared: error, not a silent Ok with no protection.
    fs::remove_file(a.join(RUNNING_FILE)).unwrap();
    let err = mark.set_child(12345).unwrap_err();
    assert!(err.contains("unreadable"), "{err}");
    // A mark replaced by another run: error, never recorded over.
    let mark2 = claim_running(&a).unwrap().expect("mark is free again");
    fs::write(a.join(RUNNING_FILE), b"another-run 1 2").unwrap();
    let err = mark2.set_child(12345).unwrap_err();
    assert!(err.contains("NOT"), "{err}");
    assert_eq!(
        fs::read(a.join(RUNNING_FILE)).unwrap(),
        b"another-run 1 2",
        "the foreign mark is untouched"
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn set_child_fails_closed_when_the_mark_cannot_be_written() {
    use std::os::unix::fs::PermissionsExt;
    let (root, _data, a, _b) = scratch("childwrite");
    let mark = claim_running(&a).unwrap().expect("no mark held yet");
    let mut perms = fs::metadata(&a).unwrap().permissions();
    perms.set_mode(0o555);
    fs::set_permissions(&a, perms).unwrap();
    let err = mark.set_child(12345).unwrap_err();
    assert!(err.contains("not recorded"), "{err}");
    // The durable hold written at claim time is intact (the failed write never
    // half-replaced it) and still blocks passes - no new guard write was needed at the
    // failure moment.
    let text = fs::read_to_string(a.join(RUNNING_FILE)).unwrap();
    assert_eq!(text.split_whitespace().nth(1), Some("unsafe"), "{text}");
    assert!(
        guard_not_running(&a).is_err(),
        "the pending hold still protects the folder"
    );
    let mut perms = fs::metadata(&a).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&a, perms).unwrap();
    mark.release().unwrap();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_fail_closed_mark_holds_forever_until_its_owner_or_a_person_clears_it() {
    let (root, _data, a, _b) = scratch("failclosed");
    let mark = claim_running(&a).unwrap().expect("no mark held yet");
    // The mark is the durable hold from claim time; fail_closed restores it after the
    // pid was recorded.
    mark.set_child(99999999).unwrap();
    assert_eq!(
        fs::read_to_string(a.join(RUNNING_FILE))
            .unwrap()
            .split_whitespace()
            .nth(2),
        Some("99999999"),
        "recorded: stale-cleanable once both processes are gone"
    );
    mark.fail_closed().unwrap();
    assert_eq!(
        fs::read_to_string(a.join(RUNNING_FILE))
            .unwrap()
            .split_whitespace()
            .nth(1),
        Some("unsafe")
    );
    let err = guard_not_running(&a).unwrap_err();
    assert!(err.contains("not confirmed stopped"), "{err}");
    // Never stale-cleaned, even though the sentinel names no live pid at all.
    assert!(
        a.join(RUNNING_FILE).exists(),
        "the hold survives the guard's stale cleanup"
    );
    // The owning run can still clear it; a foreign token cannot.
    mark.release().unwrap();
    assert!(!a.join(RUNNING_FILE).exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn fail_closed_reports_missing_or_foreign_marks_instead_of_noop() {
    let (root, _data, a, _b) = scratch("failclosedneg");
    let mark = claim_running(&a).unwrap().expect("no mark held yet");
    // A missing mark: error, never a silent Ok while protection is gone.
    fs::remove_file(a.join(RUNNING_FILE)).unwrap();
    let err = mark.fail_closed().unwrap_err();
    assert!(err.contains("unreadable"), "{err}");
    // A foreign mark: error, and the foreign content is untouched.
    let mark = claim_running(&a).unwrap().expect("mark is free again");
    fs::write(a.join(RUNNING_FILE), b"another-run 1 2").unwrap();
    let err = mark.fail_closed().unwrap_err();
    assert!(err.contains("NOT"), "{err}");
    assert_eq!(fs::read(a.join(RUNNING_FILE)).unwrap(), b"another-run 1 2");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ensure_hold_rebuilds_a_malformed_own_token_mark() {
    let (root, _data, a, _b) = scratch("malformed");
    let mark = claim_running(&a).unwrap().expect("no mark held yet");
    // Only the matching token (a torn or partial write): NO protective state, and the
    // guard's own stale-cleanup would delete it. Verification must rebuild the hold,
    // never "verify" this.
    fs::write(a.join(RUNNING_FILE), mark.token.as_bytes()).unwrap();
    mark.ensure_hold().unwrap();
    let text = fs::read_to_string(a.join(RUNNING_FILE)).unwrap();
    assert_eq!(text.split_whitespace().nth(1), Some("unsafe"), "{text}");
    assert!(
        guard_not_running(&a).is_err(),
        "the rebuilt hold really guards the folder"
    );
    mark.release().unwrap();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ensure_hold_rebuilds_a_numeric_shaped_corrupt_own_token_mark() {
    let (root, _data, a, _b) = scratch("numcorrupt");
    let mark = claim_running(&a).unwrap().expect("no mark held yet");
    // Two DEAD arbitrary pids with our token: parses as the recorded form but
    // protects nothing - the guard's stale-cleanup would delete it. Verification must
    // rebuild the non-expiring hold, never take protection from syntax alone.
    let corrupt = format!("{} 4294967294 4294967293", mark.token);
    fs::write(a.join(RUNNING_FILE), corrupt.as_bytes()).unwrap();
    mark.ensure_hold().unwrap();
    let text = fs::read_to_string(a.join(RUNNING_FILE)).unwrap();
    assert_eq!(text.split_whitespace().nth(1), Some("unsafe"), "{text}");
    assert!(
        guard_not_running(&a).is_err(),
        "the rebuilt hold really guards the folder"
    );
    mark.release().unwrap();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_lost_hold_is_re_established_without_overwriting_anyone() {
    let (root, _data, a, _b) = scratch("losthold");
    let mark = claim_running(&a).unwrap().expect("no mark held yet");
    fs::remove_file(a.join(RUNNING_FILE)).unwrap();
    mark.ensure_hold().unwrap();
    let text = fs::read_to_string(a.join(RUNNING_FILE)).unwrap();
    assert_eq!(text.split_whitespace().nth(1), Some("unsafe"), "{text}");
    assert!(
        guard_not_running(&a).is_err(),
        "the re-established hold protects the folder again"
    );
    mark.release().unwrap();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ensure_hold_never_overwrites_a_foreign_mark() {
    let (root, _data, a, _b) = scratch("foreignhold");
    let mark = claim_running(&a).unwrap().expect("no mark held yet");
    fs::write(a.join(RUNNING_FILE), b"another-run 1 2").unwrap();
    let err = mark.ensure_hold().unwrap_err();
    assert!(err.contains("another run"), "{err}");
    assert_eq!(
        fs::read(a.join(RUNNING_FILE)).unwrap(),
        b"another-run 1 2",
        "the foreign mark is untouched"
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn ensure_hold_fails_loudly_when_the_folder_is_unwritable() {
    use std::os::unix::fs::PermissionsExt;
    let (root, _data, a, _b) = scratch("holdwrite");
    let mark = claim_running(&a).unwrap().expect("no mark held yet");
    fs::remove_file(a.join(RUNNING_FILE)).unwrap();
    let mut perms = fs::metadata(&a).unwrap().permissions();
    perms.set_mode(0o555);
    fs::set_permissions(&a, perms).unwrap();
    let err = mark.ensure_hold().unwrap_err();
    assert!(err.contains("re-establish"), "{err}");
    assert!(
        guard_not_running(&a).is_ok(),
        "no hold on disk means no guard - the caller must surface this"
    );
    let mut perms = fs::metadata(&a).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&a, perms).unwrap();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_fresh_claim_is_a_durable_hold_until_the_child_is_recorded() {
    let (root, _data, a, _b) = scratch("pending");
    let mark = claim_running(&a).unwrap().expect("no mark held yet");
    assert_eq!(
        fs::read_to_string(a.join(RUNNING_FILE))
            .unwrap()
            .split_whitespace()
            .nth(1),
        Some("unsafe"),
        "the claim itself is the non-expiring hold"
    );
    // A launcher crash before recording never leaves a cleanable mark.
    assert!(guard_not_running(&a).is_err());
    mark.release().unwrap();
    let _ = fs::remove_dir_all(root);
}
