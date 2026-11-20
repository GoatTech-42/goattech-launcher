//! Per-user setup on Windows, no administrator needed.
//!
//! `horizon-launcher-setup.exe` (the launcher's own binary under another name, made by the
//! release workflow) copies itself to `%LOCALAPPDATA%\Programs\GoatTech Launcher\horizon-launcher.exe`,
//! makes the Start menu shortcut (and the desktop one if asked) and the Uninstall entry under HKCU,
//! and starts the installed copy. `horizon-launcher.exe uninstall` takes it all away again: the
//! launcher's data folder only when asked, the game's folder never.
//!
//! Nothing here deletes a folder it didn't make: the install folder is always called
//! "GoatTech Launcher" ([`folder_for`]) and is removed with `remove_dir`, which only takes an empty one.
//!
//! Command lines (handled in `main` before the CLI parser, so MCP never sees them):
//! `horizon-launcher-setup.exe` (window), `... setup --quiet [--dir <folder>] [--desktop-shortcut]`,
//! `horizon-launcher.exe uninstall` (window), `... uninstall --quiet [--remove-data]`.
use crate::backend;
use crate::install_identity as identity;
use crate::shortcut;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

pub const APP_NAME: &str = "GoatTech Launcher";
/// What earlier builds called the install: its folder, shortcuts and registry keys. Only read to
/// take over an install a goattech build made ([`identity`]); never written.
const OLD_APP_NAME: &str = "Horizon Launcher";
const BINARY: &str = "horizon-launcher.exe";
const PUBLISHER: &str = "GoatTech";
const ABOUT_URL: &str = "https://launcher.goattech.us";
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\GoatTechLauncher";
const OWN_KEY: &str = r"Software\GoatTech\Launcher";
// The upstream Horizon Launcher uses these two names too (publisher "Horizon").
const OLD_UNINSTALL_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Uninstall\HorizonLauncher";
const OLD_OWN_KEY: &str = r"Software\Horizon\Launcher";
const ASIDE_PREFIX: &str = "horizon-launcher-uninstall-";

/// What the setup window shows: installing, or removing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Install,
    Uninstall,
}

/// The parts of the work, for the progress line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Files,
    Shortcuts,
    Registry,
    Removing,
    Data,
}

#[derive(Clone, Debug)]
pub struct Options {
    /// Where the launcher goes (a new install: a folder called "GoatTech Launcher").
    pub dir: PathBuf,
    pub desktop: bool,
    /// Uninstall only: also delete the launcher's data folder.
    pub remove_data: bool,
}

impl Options {
    /// The folder of an earlier install if there is one, else the default.
    pub fn new() -> Self {
        Self {
            dir: installed().map(|i| i.dir).unwrap_or_else(default_dir),
            desktop: true,
            remove_data: false,
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Installed {
    pub dir: PathBuf,
    pub version: String,
}

fn args() -> Vec<String> {
    std::env::args().skip(1).collect()
}

/// Whether this binary is the setup: its file name starts with `horizon-launcher-setup` (the
/// release workflow copies the launcher under that name). `HORIZON_LAUNCHER_SETUP=1` forces it,
/// for screenshots and tests.
pub fn is_setup_exe() -> bool {
    if std::env::var_os("HORIZON_LAUNCHER_SETUP").is_some_and(|v| v.to_string_lossy() == "1") {
        return true;
    }
    backend::exe_path()
        .file_stem()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.to_ascii_lowercase().starts_with("horizon-launcher-setup"))
}

/// The window to open instead of the launcher: the wizard for the setup exe started without
/// arguments, the uninstaller for `uninstall`. None: this is the launcher.
pub fn window_mode() -> Option<Mode> {
    let args = args();
    match args.as_slice() {
        [] if is_setup_exe() => Some(Mode::Install),
        [a] if a.as_str() == "uninstall" => Some(Mode::Uninstall),
        _ => None,
    }
}

/// `%LOCALAPPDATA%\Programs\GoatTech Launcher`; `HORIZON_LAUNCHER_SETUP_DIR` replaces it (tests).
pub fn default_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("HORIZON_LAUNCHER_SETUP_DIR").filter(|d| !d.is_empty()) {
        return folder_for(Path::new(&dir));
    }
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Programs")
        .join(APP_NAME)
}

/// The install folder for a folder the player picked: always one called "GoatTech Launcher", so
/// uninstalling never has to guess what is ours.
pub fn folder_for(chosen: &Path) -> PathBuf {
    if chosen
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case(APP_NAME))
    {
        chosen.to_path_buf()
    } else {
        chosen.join(APP_NAME)
    }
}

/// The install this account has, if its launcher is still there.
pub fn installed() -> Option<Installed> {
    let installed = registered()?;
    installed.dir.join(BINARY).is_file().then_some(installed)
}

/// The install this account has in the registry, even if its launcher is gone (a removal still
/// clears the keys and the shortcuts then).
fn registered() -> Option<Installed> {
    read_registered(OWN_KEY).or_else(|| {
        let o = old_keys(None)?;
        let r = o.registration.clone()?;
        identity::is_ours(&o, false).then(|| Installed {
            dir: PathBuf::from(r.dir),
            version: r.version,
        })
    })
}

fn read_registered(own_key: &str) -> Option<Installed> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(own_key, KEY_READ)
        .ok()?;
    let dir: String = key.get_value("InstallDir").ok()?;
    let version: String = key.get_value("Version").unwrap_or_default();
    Some(Installed {
        dir: PathBuf::from(dir),
        version,
    })
}

pub fn installed_exe() -> Option<PathBuf> {
    installed().map(|i| i.dir.join(BINARY))
}

fn start_menu_link() -> Option<PathBuf> {
    start_menu_link_named(APP_NAME)
}

fn start_menu_link_named(name: &str) -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| {
        PathBuf::from(a)
            .join(r"Microsoft\Windows\Start Menu\Programs")
            .join(format!("{name}.lnk"))
    })
}

fn desktop_link() -> Option<PathBuf> {
    desktop_link_named(APP_NAME)
}

fn desktop_link_named(name: &str) -> Option<PathBuf> {
    shortcut::desktop_dir().map(|d| d.join(format!("{name}.lnk")))
}

fn write_link(path: &Path, exe: &Path) -> io::Result<()> {
    shortcut::write_windows_link(path, exe, "", APP_NAME).map_err(io::Error::other)
}

/// Whether the shortcut file names `exe` (its path, as the Shell Link keeps it, in UTF-16).
fn points_to(link: &Path, exe: &Path) -> bool {
    let want: Vec<u8> = exe
        .to_string_lossy()
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    fs::read(link)
        .is_ok_and(|b| !want.is_empty() && b.windows(want.len()).any(|w| w == want.as_slice()))
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .trim_end_matches('\\')
        .eq_ignore_ascii_case(b.to_string_lossy().trim_end_matches('\\'))
}

/// Puts `src` at `dest`, as a self-update does (`backend::replace_binary`): a running `dest` can't
/// be written or deleted but can be renamed, so it steps aside and `remove_replaced_binaries`
/// clears it at the next start.
fn place(src: &Path, dest: &Path) -> io::Result<()> {
    let dir = dest
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no folder"))?;
    let name = dest.file_name().unwrap_or_default().to_string_lossy();
    let pid = std::process::id();
    let temp = dir.join(format!("{name}.update-{pid}"));
    // A leftover from an earlier run (the same pid, a crash) must not stop the rename.
    let mut old = dir.join(format!("{name}.old-{pid}-0"));
    for n in 1..100 {
        if !old.exists() {
            break;
        }
        old = dir.join(format!("{name}.old-{pid}-{n}"));
    }
    if let Err(e) = fs::copy(src, &temp) {
        let _ = fs::remove_file(&temp);
        return Err(e);
    }
    match fs::rename(dest, &old) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => {
            let _ = fs::remove_file(&temp);
            return Err(e);
        }
        _ => {}
    }
    if let Err(e) = fs::rename(&temp, dest) {
        let _ = fs::rename(&old, dest);
        let _ = fs::remove_file(&temp);
        return Err(e);
    }
    let _ = fs::remove_file(&old);
    Ok(())
}

fn size_kb(exe: &Path) -> u32 {
    fs::metadata(exe).map_or(0, |m| (m.len() / 1024) as u32)
}

fn display_icon(exe: &str) -> String {
    format!("{exe},0")
}

fn uninstall_string(exe: &str) -> String {
    format!("\"{exe}\" uninstall")
}

fn quiet_uninstall_string(exe: &str) -> String {
    format!("\"{exe}\" uninstall --quiet")
}

fn register(dir: &Path, exe: &Path, version: &str) -> io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(UNINSTALL_KEY)?;
    let exe_s = exe.to_string_lossy().to_string();
    let text = |name: &str, value: &str| key.set_value(name, &value.to_string());
    text("DisplayName", APP_NAME)?;
    text("DisplayVersion", version)?;
    text("Publisher", PUBLISHER)?;
    text("InstallLocation", &dir.to_string_lossy())?;
    text("DisplayIcon", &display_icon(&exe_s))?;
    text("UninstallString", &uninstall_string(&exe_s))?;
    text("QuietUninstallString", &quiet_uninstall_string(&exe_s))?;
    text("URLInfoAbout", ABOUT_URL)?;
    text(
        "InstallDate",
        &chrono::Local::now().format("%Y%m%d").to_string(),
    )?;
    key.set_value("EstimatedSize", &size_kb(exe))?;
    key.set_value("NoModify", &1u32)?;
    key.set_value("NoRepair", &1u32)?;
    let (own, _) = hkcu.create_subkey(OWN_KEY)?;
    own.set_value("InstallDir", &dir.to_string_lossy().to_string())?;
    own.set_value("Version", &version.to_string())?;
    Ok(())
}

/// Uninstall copies that stepped aside in %TEMP% in earlier runs and are no longer in use.
/// Only %TEMP% is swept: the same wildcard in the install folder's parent could delete files
/// the launcher does not own (a player's own `horizon-launcher-uninstall-*.exe` backup).
fn clean_aside() {
    let own = format!("{ASIDE_PREFIX}{}.exe", std::process::id());
    let Ok(entries) = fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(ASIDE_PREFIX) && name.ends_with(".exe") && name != own {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Moves the running launcher out of `dir` so the folder can go: into %TEMP% when it is on the
/// same drive, else beside `dir` as a hidden file (a rename never crosses volumes: os error 17).
/// Windows deletes it at the next restart when the account may ask that (administrators);
/// otherwise the hidden copy stays beside the install folder, unused, until the player
/// removes it - it is never swept by wildcard, so only the launcher's own aside file is
/// ever deleted, and a leftover is honest rather than removed by guessing ownership.
fn step_aside(own: &Path, dir: &Path) -> io::Result<()> {
    let name = format!("{ASIDE_PREFIX}{}.exe", std::process::id());
    let temp = std::env::temp_dir();
    let mut last = io::Error::new(io::ErrorKind::NotFound, "no folder to step aside to");
    for place in crate::uninstall_aside::aside_dirs(&dir.to_string_lossy(), &temp.to_string_lossy())
    {
        let aside = Path::new(&place).join(&name);
        match fs::rename(own, &aside) {
            Ok(()) => {
                if !same_path(Path::new(&place), &temp) {
                    hide(&aside);
                }
                delete_at_restart(&aside);
                return Ok(());
            }
            Err(e) => last = e,
        }
    }
    Err(last)
}

fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain([0]).collect()
}

fn hide(path: &Path) {
    use windows_sys::Win32::Storage::FileSystem::{SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN};
    let path = wide(path);
    // SAFETY: `path` is a NUL-terminated UTF-16 string that outlives the call.
    unsafe { SetFileAttributesW(path.as_ptr(), FILE_ATTRIBUTE_HIDDEN) };
}

/// Best effort: without administrator rights Windows refuses; the aside copy then stays,
/// unused, for the player to delete (no wildcard sweep ever guesses ownership).
fn delete_at_restart(path: &Path) {
    use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT};
    let path = wide(path);
    // SAFETY: as in `hide`; a null new name means "delete".
    unsafe { MoveFileExW(path.as_ptr(), std::ptr::null(), MOVEFILE_DELAY_UNTIL_REBOOT) };
}

/// Installs the launcher this process is into `options.dir`. Returns the installed `.exe`.
pub fn install(options: &Options, progress: impl Fn(Step, f32)) -> Result<PathBuf, String> {
    let source = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = options.dir.clone();
    clean_aside();
    if dir.file_name().is_none() {
        return Err(format!("{} is not a folder to install in.", dir.display()));
    }
    let exe = dir.join(BINARY);
    progress(Step::Files, 0.05);
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // Running the installed copy as the setup (`HORIZON_LAUNCHER_SETUP=1`) rewrites the shortcuts
    // and the registry, but does not copy the file onto itself.
    if !same_path(&source, &exe) {
        place(&source, &exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    }
    progress(Step::Shortcuts, 0.6);
    if let Some(link) = start_menu_link() {
        write_link(&link, &exe).map_err(|e| format!("{}: {e}", link.display()))?;
    }
    if let Some(link) = desktop_link().filter(|_| options.desktop) {
        write_link(&link, &exe).map_err(|e| format!("{}: {e}", link.display()))?;
    }
    progress(Step::Registry, 0.85);
    register(&dir, &exe, backend::VERSION).map_err(|e| format!("The registry: {e}"))?;
    // An earlier goattech install in this folder: its shortcuts and old keys go over to the new
    // names. Not fatal: the new install already stands, and the next run finishes the clean-up.
    if let identity::Outcome::Failed(e) = adopt_legacy() {
        eprintln!("Older install not tidied: {e}");
    }
    progress(Step::Registry, 1.0);
    Ok(exe)
}

fn remove_file(path: &Path, left: &mut Vec<String>) {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => left.push(format!("{}: {e}", path.display())),
    }
}

/// Removes a folder this launcher made; what stays behind is listed rather than ignored.
fn remove_all(path: &Path, left: &mut Vec<String>) {
    if !path.exists() {
        return;
    }
    if let Err(e) = fs::remove_dir_all(path) {
        left.push(format!("{}: {e}", path.display()));
    }
}

/// Deletes `Software\<name>` when it holds no keys and no values.
fn remove_empty_parent(hkcu: &RegKey, path: &str) {
    if let Ok(parent) = hkcu.open_subkey_with_flags(path, KEY_READ) {
        let empty = parent
            .query_info()
            .is_ok_and(|info| info.sub_keys == 0 && info.values == 0);
        drop(parent);
        if empty {
            let _ = hkcu.delete_subkey(path);
        }
    }
}

/// Whether the journal flag in the new key says a clean-up of the old keys was begun. Only a
/// DWORD counts: a value of another type reads as no flag, which leaves old keys alone (an
/// orphan at worst) and never deletes one that should stay.
fn cleanup_pending() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(OWN_KEY, KEY_READ)
        .ok()
        .and_then(|k| k.get_value::<u32, _>("LegacyCleanup").ok())
        .is_some_and(|v| v != 0)
}

/// What the old keys hold, if either is still there. `fallback_dir` (the new key's folder) is
/// where the program is looked for when the old own key, which holds the folder, is gone.
fn old_keys(fallback_dir: Option<&str>) -> Option<identity::OldKeys> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let own = hkcu.open_subkey_with_flags(OLD_OWN_KEY, KEY_READ).ok();
    let uninstall = hkcu
        .open_subkey_with_flags(OLD_UNINSTALL_KEY, KEY_READ)
        .ok();
    if own.is_none() && uninstall.is_none() {
        return None;
    }
    let registration = own.and_then(|k| {
        let dir: String = k.get_value("InstallDir").ok()?;
        let version: String = k.get_value("Version").unwrap_or_default();
        Some(identity::Registration { dir, version })
    });
    let publisher: Option<String> = uninstall
        .as_ref()
        .and_then(|k| k.get_value("Publisher").ok());
    // Where the program should be: the old own key's folder, else the old Uninstall entry's own
    // location (a lone entry), else the new key's folder.
    let location: Option<String> = uninstall
        .as_ref()
        .and_then(|k| k.get_value("InstallLocation").ok());
    let dir = registration
        .as_ref()
        .map(|r| r.dir.as_str())
        .or(location.as_deref())
        .or(fallback_dir);
    let exe_present = dir.is_some_and(|d| Path::new(d).join(BINARY).is_file());
    Some(identity::OldKeys {
        registration,
        uninstall_present: uninstall.is_some(),
        publisher,
        exe_present,
    })
}

/// Deletes one key; a key that is not there is fine.
fn delete_key(key: &str) -> Result<(), String> {
    match RegKey::predef(HKEY_CURRENT_USER).delete_subkey(key) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!(r"HKCU\{key}: {e}")),
    }
}

/// The new registration, only when all of it stands: the own key with both values, and the
/// Uninstall entry with the exact values `register` writes for that folder and version (name,
/// version, publisher, location, icon, and the plain and quiet uninstall commands). Not checked:
/// URLInfoAbout, InstallDate, EstimatedSize, NoModify, NoRepair, which nothing here depends on.
fn read_new_validated() -> Option<identity::Registration> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let own = hkcu.open_subkey_with_flags(OWN_KEY, KEY_READ).ok()?;
    let dir: String = own.get_value("InstallDir").ok()?;
    let version: String = own.get_value("Version").ok()?;
    let uninstall = hkcu.open_subkey_with_flags(UNINSTALL_KEY, KEY_READ).ok()?;
    let text = |name: &str| uninstall.get_value::<String, _>(name).ok();
    let exe = Path::new(&dir).join(BINARY).to_string_lossy().to_string();
    let ok = text("DisplayName").is_some_and(|v| v == APP_NAME)
        && text("DisplayVersion").is_some_and(|v| v == version)
        && text("Publisher").is_some_and(|v| v == PUBLISHER)
        && text("InstallLocation").is_some_and(|v| same_path(Path::new(&v), Path::new(&dir)))
        && text("DisplayIcon").is_some_and(|v| v == display_icon(&exe))
        && text("UninstallString").is_some_and(|v| v == uninstall_string(&exe))
        && text("QuietUninstallString").is_some_and(|v| v == quiet_uninstall_string(&exe));
    ok.then_some(identity::Registration { dir, version })
}

/// The new keys as they stand: absent, whole, or broken (something is there that does not read
/// back in full). Broken carries the own key's folder and version when it has both: the values
/// the entry is written again from. Version is never taken from the old keys when the new own
/// key has one.
fn new_keys_state() -> identity::NewKeys {
    if let Some(whole) = read_new_validated() {
        return identity::NewKeys::Whole(whole);
    }
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let own = hkcu.open_subkey_with_flags(OWN_KEY, KEY_READ).ok();
    let uninstall_there = hkcu.open_subkey_with_flags(UNINSTALL_KEY, KEY_READ).is_ok();
    if own.is_none() && !uninstall_there {
        return identity::NewKeys::Absent;
    }
    let values = own.and_then(|k| {
        let dir: String = k.get_value("InstallDir").ok()?;
        let version: String = k.get_value("Version").ok()?;
        (!dir.trim().is_empty()).then_some(identity::Registration { dir, version })
    });
    identity::NewKeys::Broken(values)
}

/// The registry and shortcut effects [`identity::run`] uses.
struct WindowsIo;

impl identity::Io for WindowsIo {
    fn write_new(&mut self, reg: &identity::Registration) -> Result<(), String> {
        let dir = PathBuf::from(&reg.dir);
        let exe = dir.join(BINARY);
        register(&dir, &exe, &reg.version).map_err(|e| e.to_string())
    }

    fn read_new(&mut self) -> Option<identity::Registration> {
        read_new_validated()
    }

    fn rename_links(&mut self, reg: &identity::Registration) -> Result<(), String> {
        let exe = Path::new(&reg.dir).join(BINARY);
        let pairs = [
            (
                start_menu_link_named(OLD_APP_NAME),
                start_menu_link_named(APP_NAME),
            ),
            (
                desktop_link_named(OLD_APP_NAME),
                desktop_link_named(APP_NAME),
            ),
        ];
        for (old, new) in pairs {
            let (Some(old), Some(new)) = (old, new) else {
                continue;
            };
            // Only a shortcut to this install's program: another launcher's link of the same
            // name stays.
            if !old.is_file() || !points_to(&old, &exe) {
                continue;
            }
            // A link that already has the new name and points somewhere else is not ours to
            // overwrite: both links stay and this pair is skipped.
            if new.is_file() && !points_to(&new, &exe) {
                continue;
            }
            write_link(&new, &exe).map_err(|e| format!("{}: {e}", new.display()))?;
            match fs::remove_file(&old) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("{}: {e}", old.display())),
            }
        }
        Ok(())
    }

    fn mark_cleanup(&mut self) -> Result<(), String> {
        let (own, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(OWN_KEY)
            .map_err(|e| format!(r"HKCU\{OWN_KEY}: {e}"))?;
        own.set_value("LegacyCleanup", &1u32)
            .map_err(|e| format!(r"HKCU\{OWN_KEY}: {e}"))
    }

    // The Uninstall key first (it holds the publisher), the own key last (it holds the folder).
    fn delete_old_uninstall(&mut self) -> Result<(), String> {
        delete_key(OLD_UNINSTALL_KEY)
    }

    fn delete_old_own(&mut self) -> Result<(), String> {
        delete_key(OLD_OWN_KEY)?;
        remove_empty_parent(&RegKey::predef(HKEY_CURRENT_USER), r"Software\Horizon");
        Ok(())
    }

    fn clear_cleanup(&mut self) -> Result<(), String> {
        let own = match RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(OWN_KEY, KEY_WRITE)
        {
            Ok(own) => own,
            // No new key, so no flag. Any other error (access denied) means the flag may still be
            // there and the run must not report Done.
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(format!(r"HKCU\{OWN_KEY}: {e}")),
        };
        match own.delete_value("LegacyCleanup") {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!(r"HKCU\{OWN_KEY}: {e}")),
        }
    }
}

/// Takes over an install an earlier goattech build made under the old names: new keys, new
/// shortcut names, old keys gone, the folder untouched. Anything else under the old names (the
/// upstream Horizon Launcher) is left alone. Safe to run again.
pub fn adopt_legacy() -> identity::Outcome {
    // A new entry that does not read back in full is written again from the new own key's
    // values (or, when that key lacks one, from the old install that is ours), never from an
    // older version than the new key holds.
    let new = new_keys_state();
    let fallback = match &new {
        identity::NewKeys::Whole(n) | identity::NewKeys::Broken(Some(n)) => Some(n.dir.as_str()),
        _ => None,
    };
    let old = old_keys(fallback);
    let decision = identity::decide(&new, old.as_ref(), cleanup_pending());
    identity::run(&decision, &mut WindowsIo)
}

/// At launcher start: an earlier goattech install that is still under the old names is taken
/// over, but only by the launcher that runs from the registered folder (an unpacked copy or a
/// setup file touches nothing). Quick, and a failure is not fatal: the next start finishes it.
pub fn adopt_on_start() {
    let Some(registered) = registered() else {
        return;
    };
    let running = backend::exe_path();
    let Some(running_dir) = running.parent() else {
        return;
    };
    let registered_dir = registered.dir.to_string_lossy().to_string();
    // Not as the setup file or the setup and uninstall windows, even from inside the folder.
    let setup_mode = is_setup_exe() || window_mode().is_some();
    if !identity::may_tidy_at_start(
        &running_dir.to_string_lossy(),
        Some(&registered_dir),
        setup_mode,
    ) {
        return;
    }
    if let identity::Outcome::Failed(e) = adopt_legacy() {
        eprintln!("Older install not tidied: {e}");
    }
}

/// Takes the install away. Returns what could not be removed (a file in use, say).
pub fn uninstall(remove_data: bool, progress: impl Fn(Step, f32)) -> Result<Vec<String>, String> {
    let dir = registered()
        .ok_or("GoatTech Launcher is not installed for this account.")?
        .dir;
    clean_aside();
    let exe = dir.join(BINARY);
    let own = backend::exe_path().to_path_buf();
    let mut left = Vec::new();

    progress(Step::Shortcuts, 0.15);
    for name in [APP_NAME, OLD_APP_NAME] {
        if let Some(link) = start_menu_link_named(name).filter(|l| points_to(l, &exe)) {
            remove_file(&link, &mut left);
        }
    }
    // The desktop's shortcuts to this launcher: its own and the ones that play one instance.
    if let Some(desktop) = shortcut::desktop_dir() {
        for entry in fs::read_dir(desktop).into_iter().flatten().flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
                && points_to(&path, &exe)
            {
                remove_file(&path, &mut left);
            }
        }
    }

    progress(Step::Registry, 0.3);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let mut keys = vec![UNINSTALL_KEY, OWN_KEY];
    // The old keys only when they are a goattech install of this folder (a clean-up that stopped
    // half way counts, by the journal flag): the upstream launcher's keys have the same names and
    // are not ours to delete. The old Uninstall key goes before the old own key.
    let pending = cleanup_pending();
    let dir_s = dir.to_string_lossy().to_string();
    let old = old_keys(Some(&dir_s));
    if old.as_ref().is_some_and(|o| {
        identity::is_ours(o, pending)
            && o.registration
                .as_ref()
                .is_some_and(|r| identity::same_dir(&r.dir, &dir_s))
    }) {
        keys.extend([OLD_UNINSTALL_KEY, OLD_OWN_KEY]);
    }
    for key in keys {
        match hkcu.delete_subkey(key) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => left.push(format!(r"HKCU\{key}: {e}")),
        }
    }
    // `Software\Horizon` and `Software\GoatTech` go only when nothing else keeps anything in them.
    for parent in [r"Software\Horizon", r"Software\GoatTech"] {
        remove_empty_parent(&hkcu, parent);
    }

    progress(Step::Removing, 0.55);
    // The running file can be renamed but not deleted: it steps out of the folder ([`step_aside`]).
    if own.starts_with(&dir) {
        if let Err(e) = step_aside(&own, &dir) {
            left.push(format!("{}: {e}", own.display()));
        }
    }
    // Only what setup and self-updates put there: the launcher and what they moved aside.
    for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name == BINARY || name.starts_with("horizon-launcher.exe.") {
            remove_file(&entry.path(), &mut left);
        }
    }
    if let Err(e) = fs::remove_dir(&dir) {
        // Something else lives there now (a file the launcher didn't make): say so.
        if !dir.exists() || !left.is_empty() {
            // Gone already, or what stayed is listed above.
        } else {
            left.push(format!("{}: {e}", dir.display()));
        }
    }

    if remove_data {
        progress(Step::Data, 0.8);
        let data = backend::data_dir();
        // Only the folder the launcher itself names: `HORIZON_LAUNCHER_HOME` may point anywhere.
        if data
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("horizon-launcher"))
        {
            remove_all(&data, &mut left);
            // A profile whose name isn't ASCII keeps the build folder in %ProgramData%
            // (`backend::build_dir`); it is the launcher's too. Already gone with `data` otherwise.
            let builds = backend::build_dir();
            remove_all(&builds, &mut left);
        } else {
            left.push(format!(
                "{} (not the launcher's own folder)",
                data.display()
            ));
        }
    }
    progress(Step::Data, 1.0);
    Ok(left)
}

/// After a self-update: the Uninstall entry names the new version and size. Only for the install
/// this launcher runs from; a launcher unpacked from the ZIP touches nothing.
pub fn after_update(version: &str) {
    let Some(registered) = registered() else {
        return;
    };
    let exe = backend::exe_path();
    if !exe
        .parent()
        .is_some_and(|dir| same_path(dir, &registered.dir))
    {
        return;
    }
    // An install made under the old names moves to the new keys first, so they exist to update
    // (this call has the same precondition as the start hook: the check above is that this
    // launcher runs from the registered folder). If that fails, the versions stay as they were:
    // writing a newer one into a half-adopted entry would leave the two keys disagreeing.
    if let identity::Outcome::Failed(e) = adopt_legacy() {
        eprintln!("Version not recorded: {e}");
        return;
    }
    // The own key's version first, the Uninstall entry's second. The new entry is repaired from
    // the own key, so a stop between the two writes is repaired at the next start with the newer
    // version, and a failed first write writes nothing else.
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(own) = hkcu.open_subkey_with_flags(OWN_KEY, KEY_WRITE) else {
        return;
    };
    if own.set_value("Version", &version.to_string()).is_err() {
        return;
    }
    if let Ok(key) = hkcu.open_subkey_with_flags(UNINSTALL_KEY, KEY_WRITE) {
        let _ = key.set_value("DisplayVersion", &version.to_string());
        let _ = key.set_value("EstimatedSize", &size_kb(exe));
    }
}

/// `setup --quiet [--dir <folder>] [--desktop-shortcut]` (the setup exe only) and
/// `uninstall --quiet [--remove-data]`: no window, for scripts. None: not a quiet command.
pub fn run_quiet() -> Option<i32> {
    let args = args();
    let flag = |name: &str| args.iter().any(|a| a == name);
    if !flag("--quiet") {
        return None;
    }
    match args.first().map(String::as_str) {
        Some("uninstall") => Some(match uninstall(flag("--remove-data"), |_, _| {}) {
            Ok(left) => {
                for item in left {
                    eprintln!("Not removed: {item}");
                }
                0
            }
            Err(e) => {
                eprintln!("{e}");
                1
            }
        }),
        Some("setup") if is_setup_exe() => {
            let mut options = Options::new();
            if let Some(dir) = args
                .iter()
                .position(|a| a == "--dir")
                .and_then(|i| args.get(i + 1))
            {
                options.dir = folder_for(Path::new(dir));
            }
            options.desktop = flag("--desktop-shortcut");
            Some(match install(&options, |_, _| {}) {
                Ok(exe) => {
                    println!("{}", exe.display());
                    0
                }
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_install_folder_is_always_ours() {
        assert_eq!(
            folder_for(Path::new(r"C:\Games")),
            PathBuf::from(r"C:\Games\GoatTech Launcher")
        );
        assert_eq!(
            folder_for(Path::new(r"C:\Games\goattech launcher")),
            PathBuf::from(r"C:\Games\goattech launcher")
        );
    }

    /// What the shell itself reads as the shortcut's target: the field Explorer starts from.
    fn target_of(link: &Path) -> String {
        use windows::core::{Interface, HSTRING};
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED, STGM_READ,
        };
        use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
        // SAFETY: COM calls on interfaces made and dropped here, on this test's own thread.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let shell: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
            let file: IPersistFile = shell.cast().unwrap();
            file.Load(&HSTRING::from(link.as_os_str()), STGM_READ)
                .unwrap();
            let mut buf = [0u16; 520];
            shell.GetPath(&mut buf, std::ptr::null_mut(), 0).unwrap();
            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            String::from_utf16_lossy(&buf[..end])
        }
    }

    #[test]
    fn the_shell_reads_the_target_of_the_shortcut_it_gets() {
        // GitHub's Windows runners give TEMP as an 8.3 short path (C:\Users\RUNNER~1\...), which the
        // shell hands back expanded: start from the long path, as an install folder is.
        let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
        let temp = temp
            .to_str()
            .and_then(|t| t.strip_prefix(r"\\?\"))
            .map_or(temp.clone(), PathBuf::from);
        let dir = temp.join(format!("horizon-link-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let exe = dir.join("GoatTech Launcher").join("horizon-launcher.exe");
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::write(&exe, b"x").unwrap();
        let link = dir.join("GoatTech Launcher.lnk");
        write_link(&link, &exe).unwrap();
        // The bug this guards: a link whose target the shell read as empty, so a double click did nothing.
        assert_eq!(
            target_of(&link).to_lowercase(),
            exe.to_string_lossy().to_lowercase()
        );
        // The uninstaller finds its shortcuts by the program's path inside them.
        assert!(points_to(&link, &exe));
        assert!(!points_to(
            &link,
            Path::new(r"C:\Other\horizon-launcher.exe")
        ));
        let _ = fs::remove_dir_all(&dir);
    }
}
