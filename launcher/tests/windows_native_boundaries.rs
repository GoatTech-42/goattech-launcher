//! Explicit hosted-Windows qualification. Never uses a player account or contacts Microsoft.
//! Run only the ignored parent test on an ephemeral Windows runner; it owns all child stages.
#![cfg(windows)]
use horizon_launcher::{
    account::{SecretStore, SystemStore},
    backend::{self, Job, Msg},
    java, java_launch, mojang,
};
use serde_json::json;
use sha1::{Digest, Sha1};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{atomic::AtomicBool, mpsc},
    time::{Duration, Instant},
};

fn check(ok: bool, label: &str) {
    assert!(ok, "{label}");
}
fn checked(mut c: Command) {
    let o = c.output().expect("owned helper launch");
    check(
        o.status.success(),
        "owned helper failed (output retained locally, not printed)",
    );
}
fn nonce() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("random namespace");
    b.iter().map(|v| format!("{v:02x}")).collect()
}
fn value(long: bool) -> String {
    if long {
        "synthetic-not-a-credential-é-🔑".repeat(180)
    } else {
        "synthetic-short".into()
    }
}
struct CredentialCleanup(String);
impl Drop for CredentialCleanup {
    fn drop(&mut self) {
        let _ = SystemStore.delete(&self.0);
    }
}

#[test]
#[ignore = "child stage owned by hosted_windows_native_boundaries"]
fn credential_child_stage() {
    let Ok(key) = std::env::var("GT_NATIVE_KEY") else {
        panic!("parent namespace absent")
    };
    check(
        key.starts_with("goattech-native-test-"),
        "unexpected namespace",
    );
    match std::env::var("GT_NATIVE_STAGE").expect("stage").as_str() {
        "long" => {
            check(
                SystemStore.get(&key).expect("credential read").is_none(),
                "unique key already exists",
            );
            SystemStore
                .set(&key, &value(true))
                .expect("native credential write");
        }
        "read-long" => check(
            SystemStore
                .get(&key)
                .expect("native credential read")
                .is_some_and(|v| v == value(true)),
            "fresh-process long credential mismatch",
        ),
        "short" => SystemStore
            .set(&key, &value(false))
            .expect("native shorter write"),
        "read-short" => {
            check(
                SystemStore
                    .get(&key)
                    .expect("native shorter read")
                    .is_some_and(|v| v == value(false)),
                "fresh-process shorter credential mismatch",
            );
            for n in 1..16 {
                check(
                    matches!(
                        keyring::Entry::new(
                            horizon_launcher::account::SERVICE,
                            &format!("{key}.{n}")
                        )
                        .expect("piece entry")
                        .get_password(),
                        Err(keyring::Error::NoEntry)
                    ),
                    "stale credential piece after shortening",
                );
            }
        }
        "delete" => SystemStore.delete(&key).expect("native credential delete"),
        "missing" => {
            check(
                SystemStore
                    .get(&key)
                    .expect("native missing read")
                    .is_none(),
                "deleted credential remains",
            );
            for n in 0..16 {
                let k = if n == 0 {
                    key.clone()
                } else {
                    format!("{key}.{n}")
                };
                check(
                    matches!(
                        keyring::Entry::new(horizon_launcher::account::SERVICE, &k)
                            .expect("piece entry")
                            .get_password(),
                        Err(keyring::Error::NoEntry)
                    ),
                    "deleted credential piece remains",
                );
            }
        }
        _ => panic!("unknown stage"),
    }
}
fn credential_roundtrip() {
    let key = format!("goattech-native-test-{}", nonce());
    let _cleanup = CredentialCleanup(key.clone());
    for stage in [
        "long",
        "read-long",
        "short",
        "read-short",
        "delete",
        "missing",
    ] {
        let mut c = Command::new(std::env::current_exe().expect("test executable"));
        c.args(["--ignored", "--exact", "credential_child_stage"])
            .env("GT_NATIVE_KEY", &key)
            .env("GT_NATIVE_STAGE", stage);
        checked(c);
    }
}
fn sha(b: &[u8]) -> String {
    format!("{:x}", Sha1::digest(b))
}
fn zip(path: &Path, name: &str, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut z = zip::ZipWriter::new(fs::File::create(path).unwrap());
    z.start_file(name, zip::write::SimpleFileOptions::default())
        .unwrap();
    z.write_all(bytes).unwrap();
    z.finish().unwrap();
}
fn fixture_runtime(root: &Path, actual: &Path) -> java::Runtime {
    // The product installs raw files from a hash-checked manifest, not a runtime archive.
    // Only network retrieval is replaced. Actual Java25 bytes, layout and launch stay real.
    let mut files = serde_json::Map::new();
    let mut inputs = HashMap::<String, PathBuf>::new();
    fn visit(
        base: &Path,
        dir: &Path,
        files: &mut serde_json::Map<String, serde_json::Value>,
        inputs: &mut HashMap<String, PathBuf>,
    ) {
        for e in fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            let p = e.path();
            let t = e.file_type().unwrap();
            check(!t.is_symlink(), "runtime fixture symlink unsupported");
            if t.is_dir() {
                visit(base, &p, files, inputs);
            } else {
                let rel = p
                    .strip_prefix(base)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace('\\', "/");
                let url = format!("fixture-file://{rel}");
                let b = fs::read(&p).unwrap();
                files.insert(rel,json!({"type":"file","executable":false,"downloads":{"raw":{"url":url,"sha1":sha(&b),"size":b.len()}}}));
                inputs.insert(url, p);
            }
        }
    }
    visit(actual, actual, &mut files, &mut inputs);
    let manifest = serde_json::to_vec(&json!({"files":files})).unwrap();
    let index=serde_json::to_vec(&json!({"windows-x64":{"java-runtime-epsilon":[{"manifest":{"url":"fixture://manifest","sha1":sha(&manifest)},"version":{"name":"synthetic-layout-real-java25"}}]}})).unwrap();
    let fetch = |url: &str| -> Result<Vec<u8>, String> {
        if url == "fixture://manifest" {
            return Ok(manifest.clone());
        }
        if url=="https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json" {return Ok(index.clone())}
        inputs
            .get(url)
            .ok_or_else(|| "unexpected fixture fetch".to_string())
            .and_then(|p| fs::read(p).map_err(|_| "fixture read failed".to_string()))
    };
    let home = mojang::java_runtime(
        "java-runtime-epsilon",
        root,
        2,
        &fetch,
        &AtomicBool::new(false),
        &|_| {},
    )
    .expect("production runtime install");
    check(
        home == root.join("java-runtime-epsilon/windows-x64/java-runtime-epsilon"),
        "Windows runtime layout",
    );
    let need = java::Need {
        major: 25,
        component: "java-runtime-epsilon".into(),
    };
    let all = java::mojang_runtimes(root);
    let runtime = java::pick(&need, None, &all, &[])
        .unwrap()
        .expect("production runtime discovery/pick");
    check(
        runtime.major == 25
            && runtime.home == home
            && runtime.java().file_name().unwrap() == "javaw.exe",
        "production Windows Java25 pick",
    );
    let mut v = Command::new(home.join("bin/java.exe"));
    v.arg("-version");
    let o = v.output().unwrap();
    check(
        o.status.success() && String::from_utf8_lossy(&o.stderr).contains("25"),
        "installed actual Java25 invocation",
    );
    runtime
}

#[link(name = "user32")]
extern "system" {
    fn EnumWindows(f: unsafe extern "system" fn(isize, isize) -> i32, data: isize) -> i32;
    fn IsWindowVisible(w: isize) -> i32;
    fn GetWindowThreadProcessId(w: isize, pid: *mut u32) -> u32;
}
fn own_window(pid: u32) -> Option<isize> {
    unsafe extern "system" fn each(w: isize, data: isize) -> i32 {
        let pair = &mut *(data as *mut (u32, isize));
        let mut p = 0;
        GetWindowThreadProcessId(w, &mut p);
        if p == pair.0 && IsWindowVisible(w) != 0 {
            pair.1 = w;
            return 0;
        }
        1
    }
    let mut pair = (pid, 0isize);
    unsafe { EnumWindows(each, &mut pair as *mut _ as isize) };
    if pair.1 == 0 {
        None
    } else {
        Some(pair.1)
    }
}
struct ChildCleanup(backend::Running);
impl Drop for ChildCleanup {
    fn drop(&mut self) {
        let mut c = self.0.child.lock().unwrap();
        if c.try_wait().ok().flatten().is_none() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
const JAVA: &str = r#"import java.awt.*; import java.nio.file.*; import java.nio.charset.StandardCharsets;
public class BoundaryProbe {
 static native int marker();
 public static void main(String[] args) throws Exception {
  if (!args[0].equals("--gameDir") || !Path.of(args[1]).equals(Path.of("").toAbsolutePath()) || !args[2].equals("--username") || !args[3].equals("SyntheticPlayer")) throw new Exception("argument boundary");
  if (ClassLoader.getSystemResourceAsStream("second-jar-proof.txt")==null) throw new Exception("second classpath JAR missing");
  System.loadLibrary("gtqa"); if(marker()!=42) throw new Exception("native marker");
  Frame[] box=new Frame[1];EventQueue.invokeAndWait(()->{Frame f=new Frame("GoatTech native boundary fixture");f.setSize(640,240);f.add(new Label("Synthetic Java25 / second JAR / extracted Windows JNI: PASS",Label.CENTER));f.setVisible(true);box[0]=f;});
  System.out.println("fixture-ready");System.out.flush();
  if(!new java.io.BufferedReader(new java.io.InputStreamReader(System.in,StandardCharsets.UTF_8)).readLine().equals("quit")) throw new Exception("quit command");
  EventQueue.invokeAndWait(()->box[0].dispose());System.exit(0);
 }
}"#;
const C: &str = r#"#include <jni.h>
JNIEXPORT jint JNICALL Java_BoundaryProbe_marker(JNIEnv* env,jclass cls){(void)env;(void)cls;return 42;}
"#;

#[test]
#[ignore = "ephemeral Windows only: actual OS credential writes and Java/JNI window"]
fn hosted_windows_native_boundaries() {
    check(
        std::env::var("GT_NATIVE_ADMITTED").as_deref() == Ok("ephemeral-hosted"),
        "explicit ephemeral admission required",
    );
    let root = PathBuf::from(std::env::var_os("RUNNER_TEMP").expect("runner temp"))
        .join(format!("GoatTech native é space {}", nonce()));
    fs::create_dir(&root).unwrap();
    credential_roundtrip();
    let actual = PathBuf::from(std::env::var_os("GT_JAVA25_HOME").expect("actual Java25 home"));
    check(
        java::release_version(&actual).is_some_and(|(_, m)| m == 25),
        "fixture must be actual Java25",
    );
    let runtime = fixture_runtime(&root.join("runtime space é"), &actual);
    let mc = root.join("minecraft space é");
    let game = root.join("game space é");
    let native = root.join("native space é");
    let classes = root.join("classes");
    fs::create_dir_all(&game).unwrap();
    fs::create_dir(&classes).unwrap();
    fs::write(root.join("BoundaryProbe.java"), JAVA).unwrap();
    fs::write(root.join("probe.c"), C).unwrap();
    let mut javac = Command::new(actual.join("bin/javac.exe"));
    javac
        .args(["-encoding", "UTF-8", "-d"])
        .arg(&classes)
        .arg(root.join("BoundaryProbe.java"));
    checked(javac);
    let mut cl = Command::new("cl.exe");
    cl.current_dir(&root);
    cl.args(["/nologo", "/LD"])
        .arg(format!("/I{}", actual.join("include").display()))
        .arg(format!("/I{}", actual.join("include/win32").display()))
        .arg(root.join("probe.c"))
        .arg(format!("/Fe{}", root.join("gtqa.dll").display()))
        .arg(format!("/Fo{}", root.join("probe.obj").display()));
    checked(cl);
    zip(
        &mc.join("versions/boundary/boundary.jar"),
        "BoundaryProbe.class",
        &fs::read(classes.join("BoundaryProbe.class")).unwrap(),
    );
    zip(
        &mc.join("libraries/second/proof.jar"),
        "second-jar-proof.txt",
        b"literal independent resource",
    );
    zip(
        &mc.join("libraries/native/windows.jar"),
        "gtqa.dll",
        &fs::read(root.join("gtqa.dll")).unwrap(),
    );
    let version = json!({"id":"boundary","mainClass":"BoundaryProbe","javaVersion":{"majorVersion":25,"component":"java-runtime-epsilon"},"arguments":{"jvm":["-Djava.library.path=${natives_directory}","-cp","${classpath}"],"game":["--gameDir","${game_directory}","--username","${auth_player_name}"]},"libraries":[{"downloads":{"artifact":{"path":"second/proof.jar"}}},{"rules":[{"action":"allow","os":{"name":"windows"}}],"natives":{"windows":"natives-windows","linux":"natives-linux"},"downloads":{"classifiers":{"natives-windows":{"path":"native/windows.jar"},"natives-linux":{"path":"native/does-not-exist.jar"}}}}]});
    let player = java_launch::Player {
        name: "SyntheticPlayer".into(),
        uuid: "synthetic-not-account".into(),
        token: "synthetic-not-token".into(),
        xuid: "synthetic".into(),
    };
    let java = runtime.java();
    let cmd = java_launch::command(
        &version,
        &java_launch::Launch {
            minecraft: &mc,
            game_dir: &game,
            natives: &native,
            java: &java,
            memory_mb: 256,
            jvm_args: &[],
            player: &player,
            os: "windows",
            window: None,
            server: None,
        },
    )
    .expect("production Java command");
    check(
        fs::read(native.join("gtqa.dll")).unwrap() == fs::read(root.join("gtqa.dll")).unwrap(),
        "actual Windows native extraction",
    );
    let (tx, rx) = mpsc::channel();
    let child =
        ChildCleanup(backend::spawn(Job::Client, cmd, tx, 771).expect("production native spawn"));
    let pid = child.0.child.lock().unwrap().id();
    let end = Instant::now() + Duration::from_secs(30);
    let mut ready = false;
    while Instant::now() < end {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(Msg::Line(Job::Client, l)) if l == "fixture-ready" => {
                ready = true;
                break;
            }
            Ok(Msg::Exit(..) | Msg::ExitUnknown(..)) => panic!("fixture exited before ready"),
            _ => {}
        }
    }
    check(ready, "Java window readiness timeout");
    check(
        own_window(pid).is_some(),
        "actual visible window must belong to Java process",
    );
    // Synthetic pixels only, while the PID-owned window is still present.
    let screenshot = PathBuf::from(
        std::env::var_os("GT_NATIVE_CAPTURE_SCRIPT").expect("reviewed capture script"),
    );
    let mut cap = Command::new("pwsh.exe");
    cap.args(["-NoProfile", "-File"])
        .arg(screenshot)
        .arg("-OwnedPid")
        .arg(pid.to_string())
        .arg("-Out")
        .arg(root.join("native-window.png"));
    checked(cap);
    check(
        child.0.send("quit"),
        "clean quit command reached actual Java",
    );
    let end = Instant::now() + Duration::from_secs(15);
    let mut exited = false;
    while Instant::now() < end {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(Msg::Exit(Job::Client, result, 771)) => {
                check(result.is_ok(), "actual Java clean exit");
                exited = true;
                break;
            }
            Ok(Msg::ExitUnknown(..)) => panic!("Java exit unobservable"),
            _ => {}
        }
    }
    check(
        exited && own_window(pid).is_none(),
        "clean exit and owned window gone",
    );
    let receipt = json!({"credentials":"long/fresh-process/short/fresh-process/delete/fresh-process-no-pieces PASS","runtime":"production Windows raw-manifest install/discover/pick/actual Java25 PASS","launch":"production command/two-JAR/spaced-Unicode/Windows-classifier/extracted-JNI/owned-AWT/clean-exit PASS","not_proven":"OAuth, real Minecraft, official runtime download, GUI Play", "capture":root.join("native-window.png")});
    fs::write(
        PathBuf::from(std::env::var_os("RUNNER_TEMP").unwrap())
            .join("goattech-native-receipt.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
}
