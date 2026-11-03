//! The owner gate from the outside: with no account that owns Minecraft: Java Edition in the
//! launcher, neither `horizon-launcher play` nor the MCP server's play starts the game.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const REFUSED: &str = "Sign in with a Microsoft account that owns Minecraft: Java Edition to play.";

/// Fresh homes with one installed (not prepared) version and an offline name, as a player who
/// played offline before the gate has.
fn home(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("horizon-owner-gate-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let version = root.join("launcher/versions/v2026.10.27.01");
    std::fs::create_dir_all(&version).unwrap();
    std::fs::write(version.join(".installed"), "").unwrap();
    std::fs::write(
        root.join("launcher/accounts.json"),
        r#"{"accounts":[],"selected":null,"offline":["Builder_1"],"offline_selected":"Builder_1"}"#,
    )
    .unwrap();
    root
}

fn launcher(root: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_horizon-launcher"));
    cmd.args(args)
        .env("HORIZON_HOME", root.join("game"))
        .env("HORIZON_LAUNCHER_HOME", root.join("launcher"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("HORIZON_LAUNCHER_NO_SELF_UPDATE", "1")
        .env("HORIZON_NO_AUDIO", "1")
        .env("HORIZON_TELEMETRY_URL", "off")
        // Memory only: the test never reads the player's keychain.
        .env("HORIZON_NO_KEYCHAIN", "1")
        .env("HORIZON_MSA_CLIENT_ID", "test-client");
    cmd
}

fn text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

#[test]
fn the_cli_refuses_to_play_without_an_owner() {
    let root = home("cli");
    let out = launcher(&root, &["play"]).output().unwrap();
    let said = text(&out);
    assert!(!out.status.success(), "{said}");
    assert!(said.contains(REFUSED), "{said}");
    assert!(said.contains("horizon-launcher account sign-in"), "{said}");
    // Nothing of the game ran, and the offline name is still there.
    assert!(!root.join("game/sessions").exists());
    let accounts = std::fs::read_to_string(root.join("launcher/accounts.json")).unwrap();
    assert!(accounts.contains("Builder_1"), "{accounts}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_mcp_server_refuses_to_play_without_an_owner() {
    let root = home("mcp");
    let mut child = launcher(&root, &["mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let requests = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"play","arguments":{}}}"#,
    ];
    let mut stdin = child.stdin.take().unwrap();
    for r in requests {
        writeln!(stdin, "{r}").unwrap();
    }
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    let reply = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find(|v| v["id"] == 2)
        .expect("a reply to play");
    assert_eq!(reply["result"]["isError"], true, "{reply}");
    let said = reply["result"]["content"][0]["text"].as_str().unwrap();
    assert!(said.contains(REFUSED), "{said}");
    let _ = std::fs::remove_dir_all(&root);
}

/// A home with one Java instance (`game/instances/mymc` + its instances.json entry), no accounts.
fn java_home(name: &str) -> PathBuf {
    let root = home(name);
    std::fs::create_dir_all(root.join("game/instances/mymc")).unwrap();
    std::fs::write(
        root.join("launcher/instances.json"),
        r#"{"instances":{"mymc":{"name":"My MC","java":{"minecraft":"1.21.4"}}}}"#,
    )
    .unwrap();
    root
}

fn mcp_play(root: &Path, arguments: &str) -> serde_json::Value {
    let mut child = launcher(root, &["mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let requests = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
        format!(
            r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"play","arguments":{arguments}}}}}"#
        ),
    ];
    let mut stdin = child.stdin.take().unwrap();
    for r in requests {
        writeln!(stdin, "{r}").unwrap();
    }
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find(|v| v["id"] == 2)
        .expect("a reply to play")
}

#[test]
fn the_mcp_server_gates_a_java_instance_on_ownership() {
    let root = java_home("mcp-java");
    let reply = mcp_play(&root, r#"{"instance":"mymc"}"#);
    assert_eq!(reply["result"]["isError"], true, "{reply}");
    let said = reply["result"]["content"][0]["text"].as_str().unwrap();
    // The Java instance reaches the owner gate: refused as unsigned-in, not as "Rust only".
    assert!(said.contains(REFUSED), "{said}");
    assert!(!said.contains("instances only"), "{said}");
    // The gate fired before anything of the game ran or downloaded, and no secret hit the disk.
    assert!(!root.join("game/sessions").exists());
    assert!(!root.join("launcher/minecraft").exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_mcp_server_rejects_horizon_flags_for_a_java_instance() {
    let root = java_home("mcp-java-flags");
    let reply = mcp_play(&root, r#"{"instance":"mymc","world":"x"}"#);
    assert_eq!(reply["result"]["isError"], true, "{reply}");
    let said = reply["result"]["content"][0]["text"].as_str().unwrap();
    assert!(said.contains("--world is for Horizon instances"), "{said}");
    assert!(!said.contains(REFUSED), "{said}");
    let _ = std::fs::remove_dir_all(&root);
}
