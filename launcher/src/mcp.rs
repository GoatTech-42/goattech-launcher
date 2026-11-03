//! `horizon-launcher mcp`: the Model Context Protocol over stdin and stdout (JSON-RPC 2.0, one
//! message per line), so AI agents can install, prepare and run Horizon. Every tool goes through
//! `backend`. A server started here lives as long as the session. The running game's own tools
//! come from its `horizon-ctl` as `game_*` (game_tools.rs).
use crate::cli_tools;
use crate::game_tools::{self, GameTools};
use crate::account::{self, Accounts, SignIn, SignInState};
use crate::backend::{self, Job, Logged};
use crate::instances;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

pub const PROTOCOL: &str = "2025-06-18";

struct Session {
    server: Option<Logged>,
    client: Option<std::sync::Arc<Logged>>,
    /// What `play` started: a Horizon client the game_* tools drive, or a Java client that only
    /// plays on the player's screen.
    client_kind: Option<Client>,
    /// The thread that does a Java client's exit work when the game closes, however the session
    /// goes from here (idle, disconnect, or requests that never come).
    exit_waiter: Option<std::thread::JoinHandle<()>>,
    sign_in: Option<SignIn>,
    /// The selected version's `horizon-ctl mcp`, once a version is installed.
    game: Option<GameTools>,
}

/// The client `play` started in this session.
enum Client {
    Horizon,
    Java,
}

/// What a Java client's end does, as the CLI's play does when the game closes: its play time
/// recorded and its post-exit command run. Problems are said on stderr, as the CLI says them.
fn java_exit_work(
    instance: &instances::Instance,
    vars: &crate::launch::Vars,
    started: std::time::Instant,
) {
    if let Err(e) = instances::record_play(&instance.id, started.elapsed()) {
        eprintln!("Play time not saved: {e}");
    }
    if let Some(mut cmd) = crate::launch::after_exit(&instance.launch, &instance.home, vars) {
        match cmd.status() {
            Ok(s) if s.success() => {}
            Ok(s) => eprintln!("The post-exit command failed ({s})."),
            Err(e) => eprintln!("The post-exit command didn't run: {e}."),
        }
    }
}

/// The tools the session runs itself, under their CLI names: they keep a process (the game, a
/// server) or a sign-in going between calls. Every other CLI command is generated (cli_tools.rs).
fn session_tools() -> Value {
    let version = json!({ "type": "string", "description": "An installed version tag such as v2026.10.13. Default: the selected version (play: the instance's own first)." });
    let none = json!({ "type": "object", "properties": {} });
    json!([
        { "name": "play", "description": "Start the selected instance: a Horizon client (a game window on the player's screen) under agent control, the game_* tools then play it; a Java instance starts its Minecraft: Java Edition as the CLI's play does (no game_* tools). Returns once it has started; game_wait_ready waits for a Horizon world. CLI: horizon-launcher play",
          "inputSchema": { "type": "object", "properties": { "version": version,
            "instance": { "type": "string", "description": "The instance to play, by name or id (instance_list). Default: the selected one." },
            "world": { "type": "string", "description": "Open or create this world (a folder under saves) at once." },
            "creative": { "type": "boolean", "description": "Creative mode for a new world." },
            "join": { "type": "string", "description": "Join a friend's world with its invite code. Only when the player asks." },
            "server": { "type": "string", "description": "Join this server at once: an address, or a server of the instance's list (multiplayer_list) by name or position. Only when the player asks." },
            "offscreen": { "type": "boolean", "description": "Render without showing a window (game_screenshot still works)." },
            "arg": { "type": "array", "items": { "type": "string" }, "description": "Extra game arguments, passed through as-is (for example \"--server-thread\")." } } } },
        { "name": "server", "description": "Start a Horizon dedicated server. It runs until server_stop or the end of this MCP session. Requires agreeing to the Minecraft EULA. CLI: horizon-launcher server",
          "inputSchema": { "type": "object", "properties": { "version": version, "dir": { "type": "string", "description": "The server's folder. Default: the launcher's server folder." }, "accept_eula": { "type": "boolean", "description": "Agree to the Minecraft EULA (https://aka.ms/MinecraftEULA) on the player's behalf. Only with their consent." } } } },
        { "name": "server_command", "description": "Send a console command to the server started with server (for example \"list\" or \"say hello\"), and return the lines it printed in reply.",
          "inputSchema": { "type": "object", "properties": { "command": { "type": "string" } }, "required": ["command"] } },
        { "name": "server_log", "description": "The most recent lines of the server's console.",
          "inputSchema": { "type": "object", "properties": { "lines": { "type": "integer", "minimum": 1, "maximum": 2000, "description": "How many lines (default 50)." } } } },
        { "name": "server_stop", "description": "Stop the server started with server cleanly (it saves the world), killing it if it does not stop within 30 seconds.", "inputSchema": none },
        { "name": "account_list", "description": "The Microsoft accounts signed in to the launcher (names and UUIDs, never tokens), which one the game plays as (none = offline), and how a sign-in started with account_sign_in is going. CLI: horizon-launcher account list", "inputSchema": none },
        { "name": "account_sign_in", "description": "Start signing in a Microsoft account: returns a link and a code the player enters in their browser. The sign-in finishes in the background; check it with account_list. Only when the player asks to sign in. CLI: horizon-launcher account sign-in", "inputSchema": none },
    ])
}

/// Every launcher tool: the session's and the generated ones.
fn tools() -> Vec<Value> {
    let mut list = session_tools().as_array().cloned().unwrap_or_default();
    list.extend(cli_tools::all().iter().map(cli_tools::Tool::describe));
    list
}

fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

impl Session {
    fn call(&mut self, name: &str, args: &Value) -> Result<String, String> {
        match name {
            "play" => {
                if self.client.as_ref().is_some_and(|c| c.finished().is_none()) {
                    return Err("The client is already running.".into());
                }
                let instance = match arg_str(args, "instance") {
                    Some(name) => instances::find(name)?,
                    None => {
                        let all = instances::list();
                        let saved = backend::Settings::load().instance;
                        if instances::needs_first_pick(
                            &all,
                            saved.as_deref(),
                            backend::any_prepared(),
                        ) {
                            return Err(
                                "No instance yet: create one in the launcher first, then play."
                                    .into(),
                            );
                        }
                        instances::selected()
                    }
                };
                if instance.java.is_some() {
                    // A Java instance plays Minecraft: Java Edition through the same backend as
                    // the CLI's `play`: the owner gate fires inside before anything downloads,
                    // and the account's session reaches the game on its command line (never
                    // logged). The sign-in this session's `account_sign_in` started lives in
                    // this process's store, so sign-in and play can share one run.
                    for flag in ["version", "world", "join"] {
                        if arg_str(args, flag).is_some() {
                            return Err(format!(
                                "{} plays Minecraft: Java Edition: --{flag} is for Horizon instances.",
                                instance.name
                            ));
                        }
                    }
                    for flag in ["creative", "offscreen"] {
                        if args.get(flag).and_then(Value::as_bool).unwrap_or(false) {
                            return Err(format!(
                                "{} plays Minecraft: Java Edition: --{flag} is for Horizon instances.",
                                instance.name
                            ));
                        }
                    }
                    let server = arg_str(args, "server").map(|server| {
                        let list = crate::server_list::List::load(&instance.home);
                        crate::server_list::find(&list, server)
                            .map_or(server.to_string(), |(_, s)| s.ip)
                    });
                    let mut cmd =
                        backend::java_game(&instance, server.as_deref(), &|_line| {})?;
                    let unsaved = account::take_unsaved();
                    if let Some(extra) = args.get("arg").and_then(Value::as_array) {
                        cmd.args(extra.iter().filter_map(Value::as_str));
                    }
                    let vars = instance.vars(cmd.get_program());
                    cmd.envs(vars.iter().map(|(k, v)| (k, v)));
                    let name = instance.name.clone();
                    let started = std::time::Instant::now();
                    let client =
                        match Logged::start(Job::Client, crate::launch::wrap(cmd, &instance.launch)) {
                            Ok(client) => client,
                            Err(e) => {
                                return Err(match unsaved {
                                    Some(unsaved) => format!("{e}\n{unsaved}"),
                                    None => e,
                                });
                            }
                        };
                    let client = std::sync::Arc::new(client);
                    // Exit-owned: the waiter does the exit work when the game closes, whether or
                    // not another message ever arrives, and serve() joins it at the session's
                    // end - as the CLI's play stays until the game closes.
                    let waiter = {
                        let client = client.clone();
                        std::thread::spawn(move || {
                            let _ = client.wait();
                            java_exit_work(&instance, &vars, started);
                        })
                    };
                    // A Java client has no horizon-ctl: drop a stale cached one so the game_*
                    // tools cannot reach another Horizon client while this one runs.
                    self.game = None;
                    self.client_kind = Some(Client::Java);
                    self.exit_waiter = Some(waiter);
                    self.client = Some(client);
                    // No control socket on a Java client: catch an instant crash, then report.
                    // The waiter above already owns the exit work, even for this early exit.
                    std::thread::sleep(Duration::from_secs(5));
                    let client = self.client.as_ref().unwrap();
                    if let Some(result) = client.finished() {
                        let mut said = format!(
                            "The game stopped: {}\n{}",
                            result.err().unwrap_or_default(),
                            client.tail(20).join("\n")
                        );
                        if let Some(unsaved) = unsaved {
                            said.push_str(&format!("\n{unsaved}"));
                        }
                        return Err(said);
                    }
                    let mut said = format!(
                        "Started {name} (Minecraft: Java Edition) on the player's screen. The game_* tools drive Horizon clients only, not this one."
                    );
                    if let Some(unsaved) = unsaved {
                        said.push_str(&format!("\n{unsaved}"));
                    }
                    return Ok(said);
                }
                let tag = instance.resolve_version(arg_str(args, "version"))?;
                let mut cmd = backend::client(&tag)?;
                instances::apply(&mut cmd, &instance)?;
                cmd.args(game_tools::control_args());
                if let Some(world) = arg_str(args, "world") {
                    cmd.arg("--world").arg(world);
                }
                for flag in ["creative", "offscreen"] {
                    if args.get(flag).and_then(Value::as_bool).unwrap_or(false) {
                        cmd.arg(format!("--{flag}"));
                    }
                }
                if let Some(code) = arg_str(args, "join") {
                    backend::join_friend(&mut cmd, code)?;
                }
                if let Some(server) = arg_str(args, "server") {
                    let list = crate::server_list::List::load(&instance.home);
                    let ip = crate::server_list::find(&list, server)
                        .map_or_else(|_| server.to_string(), |(_, s)| s.ip);
                    cmd.arg("--server").arg(ip);
                }
                if let Some(extra) = args.get("arg").and_then(Value::as_array) {
                    cmd.args(extra.iter().filter_map(Value::as_str));
                }
                let client = Logged::start(Job::Client, cmd)?;
                // Started once its control socket answers, as horizon-ctl launch waits.
                let deadline = std::time::Instant::now() + Duration::from_secs(120);
                while !game_tools::listening() {
                    if let Some(result) = client.finished() {
                        return Err(format!(
                            "The client stopped: {}\n{}",
                            result.err().unwrap_or_default(),
                            client.tail(20).join("\n")
                        ));
                    }
                    if std::time::Instant::now() > deadline {
                        return Err(format!(
                            "The client did not open its control socket within 2 minutes.\n{}",
                            client.tail(20).join("\n")
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                self.client_kind = Some(Client::Horizon);
                self.client = Some(std::sync::Arc::new(client));
                Ok(format!(
                    "Started the {tag} client. The game_* tools play it; game_wait_ready waits until the world is ready."
                ))
            }
            "server" => {
                if self.server.as_ref().is_some_and(|s| s.finished().is_none()) {
                    return Err("A server is already running. Stop it first.".into());
                }
                let tag = backend::resolve_version(arg_str(args, "version"))?;
                let dir = backend::server_dir(arg_str(args, "dir").map(PathBuf::from));
                let accept = args
                    .get("accept_eula")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let server = Logged::start(Job::Server, backend::server(&tag, &dir, accept)?)?;
                std::thread::sleep(Duration::from_secs(3));
                let tail = server.tail(15).join("\n");
                if let Some(result) = server.finished() {
                    return Err(format!(
                        "The server stopped: {}\n{tail}",
                        result.err().unwrap_or_default()
                    ));
                }
                self.server = Some(server);
                Ok(format!(
                    "Started the {tag} server in {}.\n{tail}",
                    dir.display()
                ))
            }
            "server_command" => {
                let server = self.running_server()?;
                let before = server.log.lock().unwrap().len();
                let command = arg_str(args, "command").ok_or("command is required")?;
                server.running.send(command.trim_start_matches('/'));
                std::thread::sleep(Duration::from_millis(700));
                let log = server.log.lock().unwrap();
                let reply: Vec<_> = log.iter().skip(before.min(log.len())).cloned().collect();
                Ok(if reply.is_empty() {
                    "(no output yet)".into()
                } else {
                    reply.join("\n")
                })
            }
            "server_log" => {
                let server = self
                    .server
                    .as_ref()
                    .ok_or("No server was started in this session.")?;
                let lines = args.get("lines").and_then(Value::as_u64).unwrap_or(50) as usize;
                let state = if server.finished().is_some() {
                    "stopped"
                } else {
                    "running"
                };
                Ok(format!(
                    "[server {state}]\n{}",
                    server.tail(lines).join("\n")
                ))
            }
            "server_stop" => {
                let mut server = self
                    .server
                    .take()
                    .ok_or("No server was started in this session.")?;
                server.stop(Duration::from_secs(30));
                Ok(format!(
                    "The server stopped.\n{}",
                    server.tail(5).join("\n")
                ))
            }
            "account_list" => {
                let accounts = Accounts::load();
                let sign_in = self.sign_in.as_ref().map(|s| match s.state() {
                    SignInState::Starting => json!({ "state": "starting" }),
                    SignInState::Code(c) => json!({ "state": "waiting for the player", "link": c.verification_uri, "code": c.user_code }),
                    SignInState::Done(Ok(a)) => json!({ "state": "signed in", "name": a.name }),
                    SignInState::Done(Err(e)) => json!({ "state": "failed", "error": e }),
                });
                Ok(serde_json::to_string_pretty(&json!({
                    "accounts": accounts.accounts.iter().map(|a| json!({ "name": a.name, "uuid": a.uuid })).collect::<Vec<_>>(),
                    "plays_as": accounts.selected().map(|a| a.name.clone()),
                    "sign_in_configured": account::client_id().is_some(),
                    "keychain": account::store().persistent(),
                    // Play needs an account that owns the game (the owner gate).
                    "owner": accounts.has_owner(account::store()),
                    "sign_in": sign_in,
                }))
                .unwrap())
            }
            "account_sign_in" => {
                account::client_id().ok_or(account::NOT_CONFIGURED)?;
                if let Some(s) = self.sign_in.take() {
                    s.cancel();
                }
                let sign_in = SignIn::start();
                let deadline = std::time::Instant::now() + Duration::from_secs(30);
                let state = loop {
                    match sign_in.state() {
                        SignInState::Starting if std::time::Instant::now() < deadline => {
                            std::thread::sleep(Duration::from_millis(100))
                        }
                        state => break state,
                    }
                };
                self.sign_in = Some(sign_in);
                match state {
                    SignInState::Code(c) => Ok(format!(
                        "Ask the player to open {} and enter the code {}. Then check accounts.",
                        c.verification_uri, c.user_code
                    )),
                    SignInState::Done(Err(e)) => Err(e),
                    _ => Err("Microsoft did not answer. Try again.".into()),
                }
            }
            _ => match cli_tools::all().into_iter().find(|t| t.name == name) {
                Some(tool) => tool.call(args),
                None => Err(format!("Unknown tool {name}")),
            },
        }
    }

    /// The game's tools, starting the selected version's horizon-ctl the first time.
    /// True while this session's client is a Java one that has not exited.
    fn java_client_running(&self) -> bool {
        matches!(self.client_kind, Some(Client::Java { .. }))
            && self.client.as_ref().is_some_and(|c| c.finished().is_none())
    }

    fn game(&mut self) -> Option<&mut GameTools> {
        if self.game.is_none() {
            self.game = backend::resolve_version(None)
                .and_then(|tag| GameTools::start(&tag))
                .ok();
        }
        self.game.as_mut()
    }

    fn running_server(&mut self) -> Result<&mut Logged, String> {
        match &mut self.server {
            Some(s) if s.finished().is_none() => Ok(s),
            _ => Err("No server is running. Start one with server.".into()),
        }
    }

    fn handle(&mut self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => Ok(json!({
                "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "horizon-launcher", "version": backend::VERSION },
                "instructions": "Manage Horizon, a Rust reimplementation of Minecraft Java Edition: install versions, prepare them from the player's own Minecraft, start the client or a dedicated server. Every tool is a horizon-launcher CLI command of the same name (mods_search is horizon-launcher mods search). Once play started the game, the game_* tools play it: game_state, game_ui (every widget of the open screen), game_click, game_goto, game_screenshot...",
            })),
            "ping" => Ok(json!({})),
            "tools/list" => {
                let mut list = tools();
                // A running Java client has no game_* tools; do not start a horizon-ctl for
                // another client just to list them.
                if !self.java_client_running() {
                    if let Some(game) = self.game() {
                        list.extend(game.tools().iter().cloned());
                    }
                }
                Ok(json!({ "tools": list }))
            }
            // No AI turned on during the session: nothing more runs.
            "tools/call" if backend::no_ai() => {
                Err((-32000, horizon_core::no_ai::REFUSAL.to_string()))
            }
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or((-32602, "name is required".to_string()))?;
                // The launcher's own tools first: game_args is one, not a game tool.
                if name.starts_with(game_tools::PREFIX)
                    && !tools().iter().any(|t| t["name"] == name)
                {
                    if self.java_client_running() {
                        return Err((
                            -32602,
                            "The game_* tools drive a Horizon client; the running game is a                              Minecraft: Java Edition client, which has no control socket."
                                .to_string(),
                        ));
                    }
                    let args = params.get("arguments").cloned().unwrap_or(json!({}));
                    let game = self.game().ok_or((
                        -32602,
                        "No installed version with horizon-ctl: install one first".to_string(),
                    ))?;
                    if !game.tools().iter().any(|t| t["name"] == name) {
                        return Err((-32602, format!("Unknown tool {name}")));
                    }
                    return game.call(name, &args).map_err(|e| (-32603, e));
                }
                if !tools().iter().any(|t| t["name"] == name) {
                    return Err((-32602, format!("Unknown tool {name}")));
                }
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let (text, error) = match self.call(name, &args) {
                    Ok(text) => (text, false),
                    Err(e) => (e, true),
                };
                Ok(json!({ "content": [{ "type": "text", "text": text }], "isError": error }))
            }
            _ => Err((-32601, format!("Method not found: {method}"))),
        }
    }
}

pub fn serve() -> Result<(), String> {
    let mut session = Session {
        server: None,
        client: None,
        client_kind: None,
        exit_waiter: None,
        sign_in: None,
        game: None,
    };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Err(e) => Some(
                json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } }),
            ),
            Ok(msg) => {
                let method = msg
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let params = msg.get("params").cloned().unwrap_or(json!({}));
                // Notifications (no id) get no reply.
                msg.get("id").cloned().map(|id| match session.handle(method, &params) {
                    Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                    Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
                })
            }
        };
        if let Some(reply) = reply {
            writeln!(stdout, "{reply}")
                .and_then(|_| stdout.flush())
                .map_err(|e| e.to_string())?;
        }
    }
    // The session is over: the server it started goes with it. A Java client keeps the process
    // until the game closes, so its exit work (play time, post-exit command) always runs - as
    // the CLI's play stays attached until then.
    if let Some(waiter) = session.exit_waiter.take() {
        let _ = waiter.join();
    }
    if let Some(mut server) = session.server.take() {
        server.stop(Duration::from_secs(30));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::{Instance, JavaGame, Kind};
    use std::process::Command;
    use std::sync::Mutex;

    /// The exit-work tests point HORIZON_LAUNCHER_HOME at a scratch folder; hold this so no two
    /// of them see another's home, and the guard restores the player's own on drop.
    static HOME_LOCK: Mutex<()> = Mutex::new(());

    struct HomeGuard(Option<std::ffi::OsString>, PathBuf);

    impl HomeGuard {
        fn new(name: &str) -> HomeGuard {
            let dir =
                std::env::temp_dir().join(format!("horizon-mcp-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let old = std::env::var_os("HORIZON_LAUNCHER_HOME");
            std::env::set_var("HORIZON_LAUNCHER_HOME", &dir);
            HomeGuard(old, dir)
        }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            match &self.0 {
                Some(old) => std::env::set_var("HORIZON_LAUNCHER_HOME", old),
                None => std::env::remove_var("HORIZON_LAUNCHER_HOME"),
            }
            let _ = std::fs::remove_dir_all(&self.1);
        }
    }

    fn java_instance(post_exit: &str) -> Instance {
        Instance {
            id: "mymc-mcp-test".into(),
            name: "My MC".into(),
            kind: Kind::Own,
            work: PathBuf::from("/tmp"),
            home: PathBuf::from("/tmp"),
            icon: None,
            group: None,
            notes: None,
            version: None,
            played: Duration::ZERO,
            last_played: None,
            launch: crate::launch::Launch {
                post_exit: post_exit.into(),
                ..Default::default()
            },
            java: Some(JavaGame {
                minecraft: "1.21.4".into(),
                ..Default::default()
            }),
        }
    }

    /// A session wired as the play handler wires a Java client: shared client, kind, and the
    /// waiter thread that owns the exit work.
    fn java_session(cmd: Command, instance: Instance) -> Session {
        let client = std::sync::Arc::new(Logged::start(Job::Client, cmd).unwrap());
        let waiter = {
            let client = client.clone();
            std::thread::spawn(move || {
                let _ = client.wait();
                java_exit_work(&instance, &vec![], std::time::Instant::now());
            })
        };
        Session {
            server: None,
            client: Some(client),
            client_kind: Some(Client::Java),
            exit_waiter: Some(waiter),
            sign_in: None,
            game: None,
        }
    }

    fn long_running() -> Command {
        if cfg!(windows) {
            let mut c = Command::new("cmd.exe");
            c.args(["/D", "/S", "/C", "ping -n 20 127.0.0.1 >nul"]);
            c
        } else {
            let mut c = Command::new("sleep");
            c.arg("20");
            c
        }
    }

    fn instant_exit() -> Command {
        if cfg!(windows) {
            let mut c = Command::new("cmd.exe");
            c.args(["/D", "/S", "/C", "exit 0"]);
            c
        } else {
            Command::new("true")
        }
    }

    #[test]
    fn game_tools_do_not_reach_past_a_running_java_client() {
        let _lock = HOME_LOCK.lock().unwrap();
        let _home = HomeGuard::new("guard");
        let mut s = java_session(long_running(), java_instance(""));
        let (code, message) = s
            .handle(
                "tools/call",
                &json!({ "name": "game_state", "arguments": {} }),
            )
            .unwrap_err();
        assert_eq!(code, -32602);
        assert!(message.contains("no control socket"), "{message}");
        // tools/list advertises no game_* tools and never starts a horizon-ctl for them.
        let list = s.handle("tools/list", &json!({})).unwrap();
        let names: Vec<&str> = list["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        // game_args is the launcher's own CLI tool of that name, not a game's tool.
        assert!(
            !names.iter().any(|n| n.starts_with("game_") && *n != "game_args"),
            "{names:?}"
        );
        assert!(s.game.is_none(), "no horizon-ctl started while a Java client runs");
        // Leave nothing running: kill the stand-in; its waiter finishes the (hookless) exit work.
        s.client.as_ref().unwrap().running.kill();
        let _ = s.exit_waiter.take().unwrap().join();
    }

    /// The post-exit command is a unix shell line; the waiter lifecycle it exercises is
    /// platform-neutral.
    #[cfg(unix)]
    #[test]
    fn a_java_clients_exit_work_runs_without_another_request() {
        let _lock = HOME_LOCK.lock().unwrap();
        let home = HomeGuard::new("idle");
        let marker = home.1.join("marker");
        let mut instance = java_instance(&format!("printf 'x\n' >> '{}'", marker.display()));
        instance.home = home.1.clone();
        instance.work = home.1.clone();
        let mut s = java_session(instant_exit(), instance);
        // No MCP message follows the exit: joining the waiter, as serve() does at the session's
        // end, is what runs the exit work - once.
        let waiter = s.exit_waiter.take().unwrap();
        waiter.join().unwrap();
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "x
");
        let store = std::fs::read_to_string(home.1.join("instances.json")).unwrap();
        assert!(store.contains("mymc-mcp-test"), "{store}");
    }
}
