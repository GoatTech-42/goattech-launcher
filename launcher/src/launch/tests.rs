use super::*;

fn os(words: &[&str]) -> Vec<OsString> {
    words.iter().map(OsString::from).collect()
}

#[test]
fn variables_are_read_and_written_back() {
    let env = parse_env("MANGOHUD=1  DRI_PRIME=1\nNAME=\"two words\" EMPTY=").unwrap();
    assert_eq!(env["MANGOHUD"], "1");
    assert_eq!(env["NAME"], "two words");
    assert_eq!(env["EMPTY"], "");
    assert_eq!(parse_env(&env_line(&env)).unwrap(), env);
    assert_eq!(
        env_line(&env),
        "DRI_PRIME=1 EMPTY=\"\" MANGOHUD=1 NAME=\"two words\""
    );
    assert!(parse_env("JUSTANAME").unwrap_err().contains("NAME=value"));
    assert!(parse_env("1X=2")
        .unwrap_err()
        .contains("letters, digits and _"));
    assert!(parse_env("").unwrap().is_empty());
}

#[test]
fn a_variable_written_back_is_read_the_same_whatever_it_holds() {
    // Any whitespace in a value (a tab or a line break from a paste, a no-break space) is
    // written quoted, or reading it back would split the value and fail on its second half.
    for value in [
        "a\tb",
        "a\nb",
        "a\r\nb",
        "a\u{a0}b",
        "\t",
        " lead",
        "trail ",
        "",
        "it's",
        "say \"hi\"",
        "back\\slash",
        "ends\\",
        "\\\"",
        "plain",
        "a=b",
        "100%",
        "$HOME",
        "#x",
    ] {
        let env = BTreeMap::from([("K".to_string(), value.to_string())]);
        let line = env_line(&env);
        assert_eq!(parse_env(&line).as_ref(), Ok(&env), "{value:?} as {line:?}");
    }
    let env = BTreeMap::from([
        ("A".to_string(), "x\ty".to_string()),
        ("B".to_string(), "2".to_string()),
    ]);
    assert_eq!(parse_env(&env_line(&env)).unwrap(), env);
}

#[test]
fn the_memory_slider_has_no_lowest_value_and_ends_in_no_limit() {
    assert_eq!(memory_at(1., 16384), None);
    assert_eq!(memory_at(0., 16384), Some(1));
    assert_eq!(memory_at(0.25, 16384), Some(4096));
    assert_eq!(memory_at(0.5, 8000), Some(4032));
    assert_eq!(memory_fraction(None, 16384), 1.);
    assert_eq!(memory_fraction(Some(4096), 16384), 0.25);
    // Saved on a computer with more memory.
    assert_eq!(memory_fraction(Some(32768), 16384), 1.);
}

#[test]
fn a_typed_memory_limit_is_read_in_mb_or_gb_with_no_lowest_value() {
    assert_eq!(parse_memory("2048"), Ok(Some(2048)));
    assert_eq!(parse_memory(" 512 MB "), Ok(Some(512)));
    assert_eq!(parse_memory("4gb"), Ok(Some(4096)));
    assert_eq!(parse_memory("2 G"), Ok(Some(2048)));
    assert_eq!(parse_memory("1"), Ok(Some(1)));
    assert_eq!(parse_memory(""), Ok(None));
    assert_eq!(parse_memory("off"), Ok(None));
    // Zero in any unit is refused: it is not a limit a game starts under, and the slider's own
    // lowest value is 1.
    assert_eq!(memory_at(0., 16384), Some(1));
    for zero in ["0", "0mb", "0 M", "0gb", "0 g", " 00 "] {
        assert_eq!(parse_memory(zero), Err(NotMemory), "{zero}");
    }
    for bad in ["lots", "-5", "1.5gb", "4 tb", "99999999999", "9999999gb"] {
        assert_eq!(parse_memory(bad), Err(NotMemory), "{bad}");
    }
}

fn memory_variable(launch: &Launch) -> Option<String> {
    let mut cmd = Command::new("game");
    limit_memory(&mut cmd, launch);
    wrap(cmd, launch)
        .get_envs()
        .find(|(k, _)| *k == "HORIZON_MEMORY_LIMIT_MB")
        .and_then(|(_, v)| v.map(|v| v.to_string_lossy().into_owned()))
}

#[test]
fn the_game_has_no_limit_until_the_player_chooses_one() {
    let mut launch = Launch::default();
    assert_eq!(launch.memory_limit(), None);
    assert_eq!(memory_variable(&launch), None);

    launch.set_memory_limit(Some(1024));
    assert_eq!(memory_variable(&launch).as_deref(), Some("1024"));
    let saved = serde_json::to_string(&launch).unwrap();
    let loaded: Launch = serde_json::from_str(&saved).unwrap();
    assert_eq!(loaded.memory_limit(), Some(1024));

    launch.set_memory_limit(None);
    assert_eq!(memory_variable(&launch), None);
    let none: Launch = serde_json::from_str("{}").unwrap();
    assert_eq!(none.memory_limit(), None);
}

#[test]
fn the_players_own_memory_variable_wins() {
    let own = Launch {
        env: parse_env("HORIZON_MEMORY_LIMIT_MB=4096").unwrap(),
        ..Launch::default()
    };
    assert_eq!(memory_variable(&own).as_deref(), Some("4096"));
}

#[test]
fn a_summary_says_what_runs() {
    assert_eq!(Launch::default().summary(), None);
    let launch = Launch {
        pre_launch: "./sync.sh".into(),
        wrapper: "nice -n 5".into(),
        env: parse_env("A=1 B=2").unwrap(),
        ..Launch::default()
    };
    assert_eq!(
        launch.summary().unwrap(),
        "runs ./sync.sh first, through nice -n 5, with 2 variables"
    );
}

#[test]
fn each_system_gets_its_plan() {
    let game = os(&["/opt/horizon-client", "--world", "My World"]);
    let wrapper = Launch {
        wrapper: "nice -n 5".into(),
        ..Launch::default()
    };
    assert_eq!(
        plan(&wrapper, game.clone(), false, &Found::default()),
        Plan::Direct(os(&[
            "nice",
            "-n",
            "5",
            "/opt/horizon-client",
            "--world",
            "My World"
        ]))
    );
    let both = Launch {
        pre_launch: "echo hi".into(),
        ..wrapper.clone()
    };
    let Plan::Shell { script, words } = plan(&both, game.clone(), false, &Found::default()) else {
        panic!()
    };
    assert!(script.starts_with("(\necho hi\n) || "));
    assert!(script.ends_with("\nexec \"$@\""));
    assert_eq!(words[0], "nice");
    assert_eq!(
        plan(
            &both,
            os(&[r"C:\Horizon\horizon-client.exe", "--world", "My World"]),
            true,
            &Found::default()
        ),
        Plan::Cmd {
            line: r#"echo hi && nice -n 5 C:\Horizon\horizon-client.exe --world "My World""#.into()
        }
    );
}

#[test]
fn windows_words_are_quoted_as_the_c_runtime_reads_them() {
    let q = |w: &str| windows_quote(OsStr::new(w));
    assert_eq!(q("plain"), "plain");
    assert_eq!(q("two words"), "\"two words\"");
    assert_eq!(q(""), "\"\"");
    assert_eq!(q(r#"say "hi""#), r#""say \"hi\"""#);
    assert_eq!(q(r"C:\My Folder\"), r#""C:\My Folder\\""#);
}

#[test]
fn a_windows_word_cmd_would_act_on_is_quoted_so_it_stays_one_word() {
    let q = |w: &str| windows_quote(OsStr::new(w));
    // Each of these, bare, ends or redirects the command in cmd.exe.
    for c in ["&", "|", "<", ">", "^", "(", ")"] {
        let word = format!("-Dname=a{c}b");
        assert_eq!(q(&word), format!("\"{word}\""), "{c}");
    }
    assert_eq!(q(r"C:\Users\A&B\java.exe"), r#""C:\Users\A&B\java.exe""#);
    // A word without any of them is as it was.
    assert_eq!(q(r"C:\jre\bin\java.exe"), r"C:\jre\bin\java.exe");
    assert_eq!(q("-Xmx4096M"), "-Xmx4096M");
    // In a pre-launch line, the game's words follow `&&` as single words.
    let launch = Launch {
        pre_launch: "echo hi".into(),
        ..Launch::default()
    };
    let words = vec![OsString::from(r"C:\A&B\java.exe"), OsString::from("-Dx=1")];
    let Plan::Cmd { line } = plan(&launch, words, true, &Found::default()) else {
        panic!("not a cmd plan")
    };
    assert_eq!(line, r#"echo hi && "C:\A&B\java.exe" -Dx=1"#);
}

/// The child of [`arguments_reach_a_child_whole_through_the_pre_launch_cmd_line`]: when that
/// test starts this test binary again with `HORIZON_ARGV_ECHO` set, it writes the words it was
/// given after `--` (leaving the first, this test's name, for the test harness) as JSON. In an
/// ordinary run the variable is not set and it does nothing.
#[test]
fn argv_echo_child() {
    let Some(out) = std::env::var_os("HORIZON_ARGV_ECHO") else {
        return;
    };
    let words: Vec<String> = std::env::args().skip_while(|a| a != "--").skip(2).collect();
    std::fs::write(out, serde_json::to_string(&words).unwrap()).unwrap();
}

/// A real `cmd /D /S /C` run, as the launcher does it with a pre-launch command: every word
/// reaches the child as it was given and no word is run by cmd as a command. A word that cmd
/// had acted on would start `ver` writing `sentinel.txt` or redirect into a file of its name.
#[cfg(windows)]
#[test]
fn arguments_reach_a_child_whole_through_the_pre_launch_cmd_line() {
    let dir = std::env::temp_dir().join(format!("horizon-launch-cmd-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // The program is in a folder with an & and its name has no space: the case the quoting is for.
    let exe = dir.join("A&B").join("echochild.exe");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    std::fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
    let out = dir.join("argv.json");
    let words = [
        "plain",
        "",
        "a&b",
        "a|b",
        "a<b",
        "a>b",
        "a^b",
        "(a)",
        "-Dx=a&ver>sentinel.txt",
        r#"say "hi""#,
        r#"mixed 'q' "d" end"#,
        r"C:\trail\",
        r"C:\My Folder\",
        r#"a\"b"#,
        r#"a"b&c"#,
        r#""x"&y"#,
        r#"a&"b c""#,
        r#"p|"q"^r"#,
    ];
    let mut cmd = Command::new(&exe);
    cmd.args(["--exact", "--", "launch::tests::argv_echo_child"])
        .args(words)
        .env("HORIZON_ARGV_ECHO", &out)
        .current_dir(&dir);
    let launch = Launch {
        pre_launch: "echo pre".into(),
        ..Launch::default()
    };
    let ran = wrap(cmd, &launch).output().unwrap();
    let said = format!(
        "{}\n{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(ran.status.success(), "{said}");
    let got: Vec<String> = serde_json::from_slice(&std::fs::read(&out).expect(&said)).unwrap();
    assert_eq!(got, words, "{said}");
    let mut left: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["A&B", "argv.json"], "{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_to_add_leaves_the_command_as_it_is() {
    let mut cmd = Command::new("/opt/game");
    cmd.arg("--x").env("A", "1");
    let out = wrap(cmd, &Launch::default());
    assert_eq!(out.get_program(), "/opt/game");
    assert_eq!(out.get_args().collect::<Vec<_>>(), ["--x"]);
}

#[cfg(unix)]
#[test]
fn the_wrapped_game_runs_after_its_pre_launch_command_in_its_folder() {
    let dir = std::env::temp_dir().join(format!("horizon-launch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut game = Command::new("/bin/sh");
    // The "game" prints its words and two variables: one from the launcher, one the instance's.
    game.args([
        "-c",
        "echo \"$1|$2|$FROM_LAUNCHER|$FROM_INSTANCE\"",
        "game",
        "a b",
        "$HOME",
    ])
    .current_dir(&dir)
    .env("FROM_LAUNCHER", "yes");
    let launch = Launch {
        pre_launch: "echo ran > pre.txt".into(),
        wrapper: "/usr/bin/env WRAPPED=1".into(),
        env: parse_env("FROM_INSTANCE=\"it works\"").unwrap(),
        ..Launch::default()
    };
    let out = wrap(game, &launch).output().unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "a b|$HOME|yes|it works\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("pre.txt")).unwrap(),
        "ran\n"
    );

    let failing = Launch {
        pre_launch: "exit 3".into(),
        ..Launch::default()
    };
    let mut game = Command::new("/bin/sh");
    game.args(["-c", "echo started > started.txt"])
        .current_dir(&dir);
    let out = wrap(game, &failing).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stderr).contains("pre-launch command failed (exit 3)"));
    assert!(!dir.join("started.txt").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_linux_switches_act_only_where_their_tools_are() {
    let all = Found {
        gamemode: true,
        mangohud: true,
        two_gpus: true,
        nvidia: false,
    };
    let launch = Launch {
        wrapper: "nice".into(),
        gamemode: true,
        mangohud: true,
        dedicated_gpu: true,
        ..Launch::default()
    };
    assert_eq!(
        plan(&launch, os(&["game"]), false, &all),
        Plan::Direct(os(&["gamemoderun", "mangohud", "nice", "game"]))
    );
    assert_eq!(switch_env(&launch, &all), [("DRI_PRIME", "1")]);
    let nvidia = Found {
        nvidia: true,
        ..all
    };
    assert_eq!(
        switch_env(&launch, &nvidia),
        [
            ("__NV_PRIME_RENDER_OFFLOAD", "1"),
            ("__VK_LAYER_NV_optimus", "NVIDIA_only"),
            ("__GLX_VENDOR_LIBRARY_NAME", "nvidia")
        ]
    );
    // Not installed (or not Linux): switched on, but nothing is added.
    let none = Found::default();
    assert_eq!(
        plan(&launch, os(&["game"]), false, &none),
        Plan::Direct(os(&["nice", "game"]))
    );
    assert!(switch_env(&launch, &none).is_empty());
    assert!(!none.any() && all.any());
    assert_eq!(
        launch.summary().unwrap(),
        "through nice, with gamemode, with mangohud, on the dedicated gpu"
    );
    // Off switches aren't written.
    let text = serde_json::to_string(&Launch::default()).unwrap();
    assert_eq!(text, "{}");
}

#[test]
fn the_post_exit_command_runs_with_the_instance_variables() {
    let dir = std::env::temp_dir().join(format!("horizon-post-exit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert!(after_exit(&Launch::default(), &dir, &[]).is_none());
    let launch = Launch {
        post_exit: if cfg!(windows) {
            // The redirection first: `1> after.txt` would redirect stdout and drop the 1.
            "> after.txt echo %INST_NAME% %INST_ID% %INST_JAVA% %MINE%".into()
        } else {
            "echo \"$INST_NAME $INST_ID $INST_JAVA $MINE\" > after.txt".into()
        },
        env: parse_env("MINE=1").unwrap(),
        ..Launch::default()
    };
    assert_eq!(
        launch.summary().unwrap(),
        format!("runs {} after, with 1 variable", launch.post_exit)
    );
    let vars = instance_vars("My Pack", "my-pack", &dir, Some(OsStr::new("java")));
    let names: Vec<&str> = vars.iter().map(|(k, _)| *k).collect();
    assert_eq!(
        names,
        [
            "INST_NAME",
            "INST_ID",
            "INST_DIR",
            "INST_MC_DIR",
            "INST_JAVA"
        ]
    );
    assert!(after_exit(&launch, &dir, &vars)
        .unwrap()
        .status()
        .unwrap()
        .success());
    let written = std::fs::read_to_string(dir.join("after.txt")).unwrap();
    assert_eq!(written.trim(), "My Pack my-pack java 1");
    // A Horizon instance has no Java.
    assert!(!instance_vars("H", "horizon", &dir, None)
        .iter()
        .any(|(k, _)| *k == "INST_JAVA"));
    let _ = std::fs::remove_dir_all(&dir);
}
