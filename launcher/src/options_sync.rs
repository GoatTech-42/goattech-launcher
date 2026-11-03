//! Shared game options across instances, opt-in per instance: Luke's "syncs your keybinds
//! across profiles". Nothing syncs until the player turns it on for that instance; an instance
//! that never opted in is byte-for-byte untouched. The shared store lives in the launcher's
//! data folder; each instance keeps playing from its own `options.txt`.
//!
//! Scope is incremental and labeled: the only group so far is `keybinds` (the `key_` lines of
//! options.txt). New groups join `GROUPS` deliberately.
//!
//! Precedence, per synced key: an instance's pinned keys are always its own; otherwise the
//! shared store wins at launch, and the store learns the instance's values when the game exits
//! (last writer wins). Enabling backs the instance's file up first and never overwrites that
//! backup; every write is atomic (temp file + rename) and lines we do not touch stay
//! byte-identical, in place.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The sync groups a player can share. v1: keybinds only - the `key_` options. More groups
/// (video settings, servers) join here deliberately, each an explicit opt-in of the same shape.
const GROUPS: &[&str] = &["keybinds"];

/// An option key belongs to the sync scope when any group claims it.
fn in_scope(key: &str) -> bool {
    // keybinds: key_key.attack, key_key.jump, ...
    key.starts_with("key_")
}

/// The shared values: one map per group, so a later group never disturbs another's keys.
#[derive(Default, Serialize, Deserialize)]
struct Shared {
    /// Group name -> option key -> value.
    #[serde(default)]
    groups: BTreeMap<String, BTreeMap<String, String>>,
}

fn store_path(data: &Path) -> PathBuf {
    data.join("shared-options.json")
}

fn load_store(data: &Path) -> Shared {
    fs::read_to_string(store_path(data))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Write a file atomically: a temp sibling, then rename over the target, so a crash mid-write
/// never leaves a half-written options file or store.
fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    let tmp = path.with_extension("sync-tmp");
    fs::write(&tmp, text).map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("Could not replace {}: {e}", path.display()))
}

fn save_store(data: &Path, shared: &Shared) -> Result<(), String> {
    let text = serde_json::to_string_pretty(shared).map_err(|e| e.to_string())?;
    write_atomic(&store_path(data), &text)
}

/// An options.txt line, kept losslessly: pairs we can read, and anything else verbatim
/// (blanks, comments, lines from a newer game), in the original order.
enum Line {
    Pair(String, String),
    Raw(String),
}

fn parse(text: &str) -> Vec<Line> {
    text.lines()
        .map(|line| match line.split_once(':') {
            Some((key, value)) if !key.is_empty() => {
                Line::Pair(key.to_string(), value.to_string())
            }
            _ => Line::Raw(line.to_string()),
        })
        .collect()
}

fn render(lines: &[Line]) -> String {
    let mut out = String::new();
    for line in lines {
        match line {
            Line::Pair(key, value) => {
                out.push_str(key);
                out.push(':');
                out.push_str(value);
            }
            Line::Raw(raw) => out.push_str(raw),
        }
        out.push('\n');
    }
    out
}

/// The instance's `options.txt` follows the store for its synced, unpinned keys; keys the
/// store does not know yet are learned from the instance. Returns what the player sees.
pub fn merge_at_launch_in(home: &Path, data: &Path, pinned: &[String]) -> Result<bool, String> {
    let file = home.join("options.txt");
    let text = fs::read_to_string(&file).unwrap_or_default();
    let mut lines = parse(&text);
    let mut shared = load_store(data);
    let mut store_changed = false;
    let mut file_changed = false;
    let mut seen = vec![];
    for line in &mut lines {
        let Line::Pair(key, value) = line else { continue };
        if !in_scope(key) || pinned.iter().any(|p| p == key) {
            continue;
        }
        seen.push(key.clone());
        let group = shared.groups.entry("keybinds".to_string()).or_default();
        match group.get(key) {
            Some(shared_value) if shared_value != value => {
                *value = shared_value.clone();
                file_changed = true;
            }
            None => {
                group.insert(key.clone(), value.clone());
                store_changed = true;
            }
            _ => {}
        }
    }
    // Keys the store knows but the file does not (a fresh or trimmed file) join it.
    let shared_group = shared.groups.get("keybinds").cloned().unwrap_or_default();
    for (key, value) in shared_group {
        if !seen.contains(&key) && !pinned.iter().any(|p| p == &key) {
            lines.push(Line::Pair(key, value));
            file_changed = true;
        }
    }
    if file_changed {
        write_atomic(&file, &render(&lines))?;
    }
    if store_changed {
        save_store(data, &shared)?;
    }
    Ok(file_changed)
}

/// The game has closed: the store learns the instance's synced, unpinned keys, so the next
/// opted-in instance picks them up. Pinned keys stay the instance's own and never propagate.
pub fn learn_at_exit_in(home: &Path, data: &Path, pinned: &[String]) -> Result<(), String> {
    let file = home.join("options.txt");
    let Ok(text) = fs::read_to_string(&file) else {
        return Ok(()); // The game never wrote one: nothing to learn.
    };
    let lines = parse(&text);
    let mut shared = load_store(data);
    let mut changed = false;
    for line in &lines {
        let Line::Pair(key, value) = line else { continue };
        if !in_scope(key) || pinned.iter().any(|p| p == key) {
            continue;
        }
        let group = shared.groups.entry("keybinds".to_string()).or_default();
        if group.get(key) != Some(value) {
            group.insert(key.clone(), value.clone());
            changed = true;
        }
    }
    if changed {
        save_store(data, &shared)?;
    }
    Ok(())
}

/// First opt-in: back the instance's options up once (never overwriting that backup), then
/// merge. The backup is the way back to exactly what the instance had before syncing.
pub fn enable_in(home: &Path, data: &Path, pinned: &[String]) -> Result<(), String> {
    let file = home.join("options.txt");
    let backup = home.join("options.txt.pre-sync-backup");
    if file.exists() && !backup.exists() {
        fs::copy(&file, &backup).map_err(|e| format!("Could not back up options.txt: {e}"))?;
    }
    merge_at_launch_in(home, data, pinned)?;
    Ok(())
}

/// The sync groups on offer, for the CLI to say honestly what syncs today.
pub fn groups() -> &'static [&'static str] {
    GROUPS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("horizon-sync-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let home = root.join("home");
        let data = root.join("data");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&data).unwrap();
        (home, data)
    }

    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_changed_keybind_reaches_the_next_opted_in_instance() {
        let (home_a, data) = dirs("a");
        let home_b = home_a.parent().unwrap().join("b");
        fs::create_dir_all(&home_b).unwrap();
        let _clean = Cleanup(home_a.parent().unwrap().to_path_buf());
        fs::write(
            home_a.join("options.txt"),
            "key_key.attack:key.keyboard.attack.mouse.left\ngamma:1.0\n",
        )
        .unwrap();
        fs::write(
            home_b.join("options.txt"),
            "key_key.attack:key.keyboard.attack.mouse.left\n",
        )
        .unwrap();
        // A opts in and changes its attack key; the store learns it when A exits.
        enable_in(&home_a, &data, &[]).unwrap();
        fs::write(
            home_a.join("options.txt"),
            "key_key.attack:key.keyboard.g\ngamma:1.0\n",
        )
        .unwrap();
        learn_at_exit_in(&home_a, &data, &[]).unwrap();
        // B opts in later: its existing file already has the key, and still gets the change.
        enable_in(&home_b, &data, &[]).unwrap();
        let text = fs::read_to_string(home_b.join("options.txt")).unwrap();
        assert!(text.contains("key_key.attack:key.keyboard.g"), "{text}");
        // Out-of-scope keys are never shared.
        assert!(!text.contains("gamma"), "{text}");
    }

    #[test]
    fn an_instance_that_never_opts_in_is_untouched() {
        let (home, data) = dirs("b");
        let _clean = Cleanup(home.parent().unwrap().to_path_buf());
        let before = "key_key.jump:key.keyboard.space\nweird newer line\n\n";
        fs::write(home.join("options.txt"), before).unwrap();
        // Store activity for other instances must not touch this file.
        let other = home.parent().unwrap().join("other");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("options.txt"), "key_key.jump:key.keyboard.j\n").unwrap();
        enable_in(&other, &data, &[]).unwrap();
        learn_at_exit_in(&other, &data, &[]).unwrap();
        assert_eq!(fs::read_to_string(home.join("options.txt")).unwrap(), before);
        assert!(!home.join("options.txt.pre-sync-backup").exists());
    }

    #[test]
    fn pinned_keys_win_and_unknown_lines_survive() {
        let (home, data) = dirs("c");
        let _clean = Cleanup(home.parent().unwrap().to_path_buf());
        fs::write(
            home.join("options.txt"),
            "key_key.sneak:key.keyboard.left.shift\nnot-an-option line\nkey_key.jump:key.keyboard.space\n",
        )
        .unwrap();
        let pinned = vec!["key_key.sneak".to_string()];
        enable_in(&home, &data, &pinned).unwrap();
        learn_at_exit_in(&home, &data, &pinned).unwrap();
        // Another instance sets sneak differently and shares it.
        let other = home.parent().unwrap().join("other-c");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("options.txt"), "key_key.sneak:key.keyboard.c\n").unwrap();
        enable_in(&other, &data, &[]).unwrap();
        learn_at_exit_in(&other, &data, &[]).unwrap();
        // The pinned instance launches: its sneak stays, its jump follows the store.
        merge_at_launch_in(&home, &data, &pinned).unwrap();
        let text = fs::read_to_string(home.join("options.txt")).unwrap();
        assert!(text.contains("key_key.sneak:key.keyboard.left.shift"), "{text}");
        assert!(text.contains("not-an-option line"), "{text}");
        // And the pinned value never reached the store.
        let store = fs::read_to_string(data.join("shared-options.json")).unwrap();
        assert!(!store.contains("left.shift"), "{store}");
        assert!(store.contains("key.keyboard.c"), "{store}");
    }

    #[test]
    fn enabling_backs_up_once_and_enable_is_idempotent() {
        let (home, data) = dirs("d");
        let _clean = Cleanup(home.parent().unwrap().to_path_buf());
        let before = "key_key.jump:key.keyboard.space\n";
        fs::write(home.join("options.txt"), before).unwrap();
        enable_in(&home, &data, &[]).unwrap();
        let backup = home.join("options.txt.pre-sync-backup");
        assert_eq!(fs::read_to_string(&backup).unwrap(), before);
        // Later changes and a re-enable never overwrite the original backup.
        fs::write(home.join("options.txt"), "key_key.jump:key.keyboard.j\n").unwrap();
        enable_in(&home, &data, &[]).unwrap();
        assert_eq!(fs::read_to_string(&backup).unwrap(), before);
    }
}
