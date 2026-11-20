//! Which older Windows install the goattech setup may take over, and in what order.
//!
//! Pure logic: no registry, no files, so it is tested on every platform. `installer` (Windows
//! only) reads the old keys into [`OldKeys`], asks [`decide`], and runs the answer with [`run`]
//! through its own [`Io`]. The rules:
//! - An old registration is ours only when its Uninstall entry names the publisher "GoatTech"
//!   and its program is still there. The upstream Horizon Launcher writes the same key names
//!   with the publisher "Horizon"; it is never read for deletion, renamed or moved.
//! - The install folder is never moved. An adopted install keeps its folder.
//! - New keys are written and read back before a shortcut is renamed, and shortcuts are
//!   renamed before the old keys go. A run that stops anywhere can be run again.
//! - Recovery policy: the new own key (folder and version) is what the new entry is repaired
//!   from, because it is never older than the old keys. A new entry that does not read back in
//!   full is written again from those values, and a version already recorded is never replaced
//!   by an older one. Only when the new own key itself is missing a value does an old install
//!   that is ours supply the values (nothing newer exists then). This is a repair rule, not a
//!   promise that an update stopped at any point converges: the update writes the own key's
//!   version first and the Uninstall entry second, and a stop between them is repaired by the
//!   next start.
//! - The old keys go one at a time, after a journal flag is set in the new key. The old Uninstall
//!   key (it holds the publisher) goes first and the old own key (it holds the folder) last, so
//!   a run that stops between them still shows what is left, and the flag says it is ours.

/// The publisher a goattech build writes into the Uninstall entry.
pub const PUBLISHER_MARK: &str = "GoatTech";

/// An install as the registry records it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registration {
    pub dir: String,
    pub version: String,
}

/// What the old keys hold, read by the caller. `None` from the reader means neither key exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OldKeys {
    /// The old own key (folder and version), if it is still there.
    pub registration: Option<Registration>,
    /// The old Uninstall key is still there.
    pub uninstall_present: bool,
    /// The Publisher value of the old Uninstall entry, if it has one.
    pub publisher: Option<String>,
    /// The program file exists in the old folder.
    pub exe_present: bool,
}

/// The new keys as the caller read them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NewKeys {
    /// Neither the new own key nor the new Uninstall entry exists.
    Absent,
    /// The own key and the whole Uninstall entry stand and agree.
    Whole(Registration),
    /// Something is there but it does not read back in full. `Some` when the own key still has
    /// both its values (the repair source), `None` when it has not.
    Broken(Option<Registration>),
}

/// What to do about the old keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// No old keys, no new keys: a fresh machine.
    Nothing,
    /// New keys exist and the old keys are not ours (or point elsewhere): leave everything.
    AlreadyNew,
    /// Old keys that are not ours (an upstream install, or one we cannot tell): leave alone.
    LeaveAlone,
    /// Old keys are ours and there are no new keys: write new ones, then clean up.
    Adopt(Registration),
    /// New keys exist for the same folder as our old keys, or a clean-up was begun: only the
    /// clean-up is left (after the new entry is read back, or written again from `reg`).
    FinishCleanup(Registration),
    /// A lone old Uninstall entry of ours (publisher GoatTech, its own key gone) whose program
    /// is gone: delete that entry. See [`decide`] for what is left alone.
    SweepUninstall,
    /// The new entry does not read back in full and no clean-up is due: write it again from
    /// the own key's values.
    Repair(Registration),
}

/// Whether the old keys were made by a goattech build and its program is still there.
/// `pending` is the journal flag in the new key: a clean-up of our old keys was begun, so an
/// Uninstall key that is already gone (and its publisher with it) is our own doing.
pub fn is_ours(old: &OldKeys, pending: bool) -> bool {
    old.exe_present
        && match old.publisher.as_deref() {
            Some(p) => p.trim().eq_ignore_ascii_case(PUBLISHER_MARK),
            None => pending && !old.uninstall_present,
        }
}

/// Two folder paths as the registry writes them: compared without case or a trailing slash.
/// Unicode lowercasing is not Windows' own comparison: a case-sensitive directory could hold
/// two folders this calls the same, and a junction or 8.3 short name compares unequal (which
/// leaves things alone). The new and old entries come from the same strings, so they match.
pub fn same_dir(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim().trim_end_matches(['\\', '/']).to_lowercase();
    norm(a) == norm(b)
}

/// The rules above, as one function of what is in the registry. `pending` is the journal flag
/// in the new key.
pub fn decide(new: &NewKeys, old: Option<&OldKeys>, pending: bool) -> Decision {
    // A dead leftover: an old build's uninstall removes the Uninstall key first and can stop
    // before the own key, or the reverse, so an Uninstall entry of ours may be left with no own
    // key and no program. It points at nothing, so only it goes. A lone entry whose program is
    // still there, and a lone old own key with no flag, are left alone (the player may still
    // use that folder, and nothing here knows enough to decide for them).
    if let Some(o) = old {
        if o.registration.is_none()
            && o.uninstall_present
            && !o.exe_present
            && o.publisher
                .as_deref()
                .is_some_and(|p| p.trim().eq_ignore_ascii_case(PUBLISHER_MARK))
        {
            return Decision::SweepUninstall;
        }
    }
    match new {
        NewKeys::Whole(n) | NewKeys::Broken(Some(n)) => {
            let cleanup = match old {
                Some(o) => {
                    o.registration
                        .as_ref()
                        .is_some_and(|r| same_dir(&r.dir, &n.dir))
                        && is_ours(o, pending)
                }
                // Both old keys are gone but the flag is still set: clear it.
                None => pending,
            };
            if cleanup {
                Decision::FinishCleanup(n.clone())
            } else if matches!(new, NewKeys::Broken(_)) {
                Decision::Repair(n.clone())
            } else {
                Decision::AlreadyNew
            }
        }
        NewKeys::Absent | NewKeys::Broken(None) => match old {
            Some(o) if is_ours(o, false) => match &o.registration {
                Some(r) if !r.dir.trim().is_empty() => Decision::Adopt(r.clone()),
                _ => Decision::LeaveAlone,
            },
            Some(_) => Decision::LeaveAlone,
            None => Decision::Nothing,
        },
    }
}

/// Whether a launcher that starts from `running_dir` is the registered install and may tidy it.
/// The start-up check uses it so an unpacked copy touches nothing, and neither does the setup
/// or uninstall window, even from inside the registered folder (`setup_mode`).
pub fn may_tidy_at_start(
    running_dir: &str,
    registered_dir: Option<&str>,
    setup_mode: bool,
) -> bool {
    !setup_mode && registered_dir.is_some_and(|d| same_dir(running_dir, d))
}

/// The effects, in the order [`run`] uses them. `installer` implements them on the registry
/// and the shortcut files; the tests implement them in memory. Each one is safe to repeat.
pub trait Io {
    /// Writes the new registration.
    fn write_new(&mut self, reg: &Registration) -> Result<(), String>;
    /// Reads the new registration back, only if all of it stands (the own key and the whole
    /// Uninstall entry); a half-written one reads as `None`.
    fn read_new(&mut self) -> Option<Registration>;
    /// Gives the shortcuts that point at the program in `reg.dir` their new names.
    fn rename_links(&mut self, reg: &Registration) -> Result<(), String>;
    /// Sets the journal flag in the new key: the old keys are about to go.
    fn mark_cleanup(&mut self) -> Result<(), String>;
    /// Deletes the old Uninstall key (a missing key is fine).
    fn delete_old_uninstall(&mut self) -> Result<(), String>;
    /// Deletes the old own key (a missing key is fine).
    fn delete_old_own(&mut self) -> Result<(), String>;
    /// Clears the journal flag: nothing of the old install is left.
    fn clear_cleanup(&mut self) -> Result<(), String>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing was needed.
    Skipped,
    /// The old keys are gone and the new ones stand.
    Done,
    /// A step failed; the steps after it did not run. Some old keys may be gone, and the next
    /// run reads what is left.
    Failed(String),
}

/// Carries out `decision`, stopping at the first failure.
pub fn run(decision: &Decision, io: &mut dyn Io) -> Outcome {
    match decision {
        Decision::Nothing | Decision::AlreadyNew | Decision::LeaveAlone => Outcome::Skipped,
        Decision::SweepUninstall => match io.delete_old_uninstall() {
            Ok(()) => Outcome::Done,
            Err(e) => Outcome::Failed(e),
        },
        Decision::Repair(reg) => {
            if let Err(e) = io.write_new(reg) {
                return Outcome::Failed(e);
            }
            if io.read_new().as_ref() != Some(reg) {
                return Outcome::Failed("the new registration did not read back".to_string());
            }
            Outcome::Done
        }
        Decision::Adopt(reg) => {
            if let Err(e) = io.write_new(reg) {
                return Outcome::Failed(e);
            }
            if io.read_new().as_ref() != Some(reg) {
                return Outcome::Failed("the new registration did not read back".to_string());
            }
            finish(reg, io)
        }
        Decision::FinishCleanup(reg) => {
            // The new entry must stand in full before an old key goes. If it does not read back,
            // write it again from the same values and read it once more.
            if io.read_new().as_ref() != Some(reg) {
                if let Err(e) = io.write_new(reg) {
                    return Outcome::Failed(e);
                }
                if io.read_new().as_ref() != Some(reg) {
                    return Outcome::Failed("the new registration did not read back".to_string());
                }
            }
            finish(reg, io)
        }
    }
}

fn finish(reg: &Registration, io: &mut dyn Io) -> Outcome {
    let done = io
        .rename_links(reg)
        .and_then(|()| io.mark_cleanup())
        .and_then(|()| io.delete_old_uninstall())
        .and_then(|()| io.delete_old_own())
        .and_then(|()| io.clear_cleanup());
    match done {
        Ok(()) => Outcome::Done,
        Err(e) => Outcome::Failed(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD_DIR: &str = r"C:\Users\p\AppData\Local\Programs\Horizon Launcher";

    fn reg(dir: &str) -> Registration {
        reg_v(dir, "1.0.0")
    }

    fn reg_v(dir: &str, version: &str) -> Registration {
        Registration {
            dir: dir.to_string(),
            version: version.to_string(),
        }
    }

    fn old(publisher: Option<&str>, exe_present: bool) -> OldKeys {
        OldKeys {
            registration: Some(reg(OLD_DIR)),
            uninstall_present: publisher.is_some(),
            publisher: publisher.map(str::to_string),
            exe_present,
        }
    }

    fn whole(dir: &str) -> NewKeys {
        NewKeys::Whole(reg(dir))
    }

    /// The registry and shortcuts in memory, each key on its own. `fail_at` makes the n-th
    /// call (from 0, reads count) fail: before it changes anything, or after it has (`after`,
    /// as an access error after the write would). A write that fails before stops between
    /// values: with the own key still missing its version, or (`stops_after_own`) with the
    /// own key whole and the Uninstall entry not.
    #[derive(Default)]
    struct Fake {
        new: Option<Registration>,
        /// The Uninstall entry and the own key are all there and agree.
        new_valid: bool,
        pending: bool,
        old_uninstall: bool,
        old_own: bool,
        /// The Publisher value of the old Uninstall key while it exists.
        publisher: String,
        links_new: bool,
        calls: Vec<&'static str>,
        fail_at: Option<usize>,
        after: bool,
        stops_after_own: bool,
        /// A failure was injected during the run.
        fired: bool,
        /// Makes every read of the new keys come back empty.
        read_fails: bool,
    }

    impl Fake {
        fn with_old(publisher: &str) -> Self {
            Fake {
                old_uninstall: true,
                old_own: true,
                publisher: publisher.to_string(),
                ..Fake::default()
            }
        }

        /// Records the call and says whether it is the one that fails.
        fn hit(&mut self, name: &'static str) -> bool {
            let n = self.calls.len();
            self.calls.push(name);
            let fails = self.fail_at == Some(n);
            self.fired |= fails;
            fails
        }

        /// What the reader would see now.
        fn old_keys(&self) -> Option<OldKeys> {
            if !self.old_uninstall && !self.old_own {
                return None;
            }
            Some(OldKeys {
                registration: self.old_own.then(|| reg(OLD_DIR)),
                uninstall_present: self.old_uninstall,
                publisher: self.old_uninstall.then(|| self.publisher.clone()),
                exe_present: true,
            })
        }

        fn new_keys(&self) -> NewKeys {
            match &self.new {
                None => NewKeys::Absent,
                Some(r) if self.new_valid => NewKeys::Whole(r.clone()),
                Some(r) if !r.version.is_empty() => NewKeys::Broken(Some(r.clone())),
                Some(_) => NewKeys::Broken(None),
            }
        }

        fn decide(&self) -> Decision {
            decide(&self.new_keys(), self.old_keys().as_ref(), self.pending)
        }

        fn nothing_left(&self) -> bool {
            !self.old_uninstall && !self.old_own && !self.pending
        }
    }

    fn failed(name: &str) -> Result<(), String> {
        Err(format!("{name} failed"))
    }

    impl Io for Fake {
        fn write_new(&mut self, r: &Registration) -> Result<(), String> {
            let fails = self.hit("write_new");
            if fails && !self.after {
                self.new = Some(if self.stops_after_own {
                    r.clone()
                } else {
                    reg_v(&r.dir, "")
                });
                self.new_valid = false;
                return failed("write_new");
            }
            self.new = Some(r.clone());
            self.new_valid = true;
            if fails {
                return failed("write_new");
            }
            Ok(())
        }
        fn read_new(&mut self) -> Option<Registration> {
            self.calls.push("read_new");
            if self.read_fails || !self.new_valid {
                return None;
            }
            self.new.clone()
        }
        fn rename_links(&mut self, _r: &Registration) -> Result<(), String> {
            let fails = self.hit("rename_links");
            if fails && !self.after {
                return failed("rename_links");
            }
            self.links_new = true;
            if fails {
                return failed("rename_links");
            }
            Ok(())
        }
        fn mark_cleanup(&mut self) -> Result<(), String> {
            let fails = self.hit("mark_cleanup");
            if fails && !self.after {
                return failed("mark_cleanup");
            }
            self.pending = true;
            if fails {
                return failed("mark_cleanup");
            }
            Ok(())
        }
        fn delete_old_uninstall(&mut self) -> Result<(), String> {
            let fails = self.hit("delete_old_uninstall");
            if fails && !self.after {
                return failed("delete_old_uninstall");
            }
            self.old_uninstall = false;
            if fails {
                return failed("delete_old_uninstall");
            }
            Ok(())
        }
        fn delete_old_own(&mut self) -> Result<(), String> {
            let fails = self.hit("delete_old_own");
            if fails && !self.after {
                return failed("delete_old_own");
            }
            self.old_own = false;
            if fails {
                return failed("delete_old_own");
            }
            Ok(())
        }
        fn clear_cleanup(&mut self) -> Result<(), String> {
            let fails = self.hit("clear_cleanup");
            if fails && !self.after {
                return failed("clear_cleanup");
            }
            self.pending = false;
            if fails {
                return failed("clear_cleanup");
            }
            Ok(())
        }
    }

    // Case 1: a fresh machine.
    #[test]
    fn a_fresh_machine_needs_nothing() {
        let d = decide(&NewKeys::Absent, None, false);
        assert_eq!(d, Decision::Nothing);
        let mut io = Fake::default();
        assert_eq!(run(&d, &mut io), Outcome::Skipped);
        assert!(io.calls.is_empty());
    }

    // Case 2: an install made by a goattech build is adopted, in the safe order.
    #[test]
    fn a_goattech_install_is_adopted_in_order() {
        let mut io = Fake::with_old("GoatTech");
        let d = io.decide();
        assert_eq!(d, Decision::Adopt(reg(OLD_DIR)));
        assert_eq!(run(&d, &mut io), Outcome::Done);
        assert_eq!(
            io.calls,
            [
                "write_new",
                "read_new",
                "rename_links",
                "mark_cleanup",
                "delete_old_uninstall",
                "delete_old_own",
                "clear_cleanup"
            ]
        );
        assert_eq!(io.new, Some(reg(OLD_DIR)));
        assert!(io.links_new);
        assert!(io.nothing_left());
    }

    #[test]
    fn the_folder_is_kept_by_adoption() {
        let o = old(Some("goattech "), true);
        match decide(&NewKeys::Absent, Some(&o), false) {
            Decision::Adopt(r) => assert_eq!(r.dir, OLD_DIR),
            other => panic!("expected adoption, got {other:?}"),
        }
    }

    // Case 3: an upstream Horizon Launcher install is never touched.
    #[test]
    fn an_upstream_install_is_left_alone() {
        let mut io = Fake::with_old("Horizon");
        let d = io.decide();
        assert_eq!(d, Decision::LeaveAlone);
        assert_eq!(run(&d, &mut io), Outcome::Skipped);
        assert!(io.calls.is_empty());
        assert!(io.old_uninstall && io.old_own);
        assert!(io.new.is_none());
    }

    // Case 4: old keys we cannot tell apart are left alone too.
    #[test]
    fn an_old_install_without_the_mark_is_left_alone() {
        let no_publisher = OldKeys {
            publisher: None,
            ..old(Some("x"), true)
        };
        let absent = NewKeys::Absent;
        assert_eq!(
            decide(&absent, Some(&no_publisher), false),
            Decision::LeaveAlone
        );
        assert_eq!(
            decide(&absent, Some(&old(Some(""), true)), false),
            Decision::LeaveAlone
        );
        assert_eq!(
            decide(&absent, Some(&old(Some("Horizon"), true)), false),
            Decision::LeaveAlone
        );
    }

    #[test]
    fn a_marked_install_whose_program_is_gone_is_left_alone() {
        assert_eq!(
            decide(&NewKeys::Absent, Some(&old(Some("GoatTech"), false)), false),
            Decision::LeaveAlone
        );
    }

    // The publisher must equal the mark: not start with it, contain it, or differ in the name.
    #[test]
    fn only_the_exact_publisher_is_ours() {
        for p in [
            "GoatTech Ltd",
            "not goattech",
            "GoatTechX",
            "xGoatTech",
            "Goat Tech",
        ] {
            assert_eq!(
                decide(&NewKeys::Absent, Some(&old(Some(p), true)), false),
                Decision::LeaveAlone,
                "{p}"
            );
        }
        for p in ["GoatTech", " goattech ", "GOATTECH"] {
            assert!(
                matches!(
                    decide(&NewKeys::Absent, Some(&old(Some(p), true)), false),
                    Decision::Adopt(_)
                ),
                "{p}"
            );
        }
    }

    // An old Uninstall key that is still there without a publisher is not ours, flag or not.
    #[test]
    fn an_uninstall_key_without_a_publisher_is_not_ours_even_with_the_flag() {
        let o = OldKeys {
            publisher: None,
            uninstall_present: true,
            ..old(Some("x"), true)
        };
        assert_eq!(
            decide(&NewKeys::Absent, Some(&o), true),
            Decision::LeaveAlone
        );
        assert_eq!(
            decide(&whole(OLD_DIR), Some(&o), true),
            Decision::AlreadyNew
        );
    }

    // Folder missing from the old own key: nothing to adopt.
    #[test]
    fn an_old_install_with_an_empty_folder_is_not_adopted() {
        for registration in [Some(reg("")), Some(reg("  ")), None] {
            let o = OldKeys {
                registration,
                ..old(Some("GoatTech"), true)
            };
            assert_eq!(
                decide(&NewKeys::Absent, Some(&o), false),
                Decision::LeaveAlone
            );
        }
    }

    // Case 5, for every call and both kinds of failure (before the change, and after it as an
    // access error would): the run stops right there with the effects before it done and none
    // after it, and a second run from whatever the registry shows ends with the new keys
    // standing and nothing of the old install left.
    #[test]
    fn a_stop_at_any_step_is_finished_by_the_next_run() {
        for after in [false, true] {
            for partial_own in [false, true] {
                for stop in 0..8 {
                    let tag = format!("stop {stop} after {after} partial_own {partial_own}");
                    let mut io = Fake::with_old("GoatTech");
                    io.fail_at = Some(stop);
                    io.after = after;
                    io.stops_after_own = partial_own;
                    let first = io.decide();
                    let out = run(&first, &mut io);
                    if !io.fired {
                        // Index 1 is read_new, which cannot stop, and 7 is past the last call.
                        assert_eq!(out, Outcome::Done, "{tag}");
                        continue;
                    }
                    assert!(matches!(out, Outcome::Failed(_)), "{tag}");
                    // A failure ends the run: nothing is called after it.
                    assert_eq!(io.calls.len(), stop + 1, "{tag}");
                    // The old Uninstall key is deleted before the own key, never the reverse.
                    assert!(!(io.old_uninstall && !io.old_own), "{tag}");
                    io.fail_at = None;
                    io.calls.clear();
                    let second = io.decide();
                    assert!(
                        matches!(
                            second,
                            Decision::Adopt(_)
                                | Decision::FinishCleanup(_)
                                | Decision::Repair(_)
                                | Decision::AlreadyNew
                        ),
                        "{tag}: {second:?}"
                    );
                    let out = run(&second, &mut io);
                    assert!(
                        matches!(out, Outcome::Done | Outcome::Skipped),
                        "{tag}: {out:?}"
                    );
                    assert!(io.nothing_left(), "{tag}");
                    assert_eq!(io.new, Some(reg(OLD_DIR)), "{tag}");
                    assert!(io.new_valid, "{tag}");
                    // And a third start has nothing to do.
                    assert_eq!(io.decide(), Decision::AlreadyNew, "{tag}");
                }
            }
        }
    }

    // The case QA found: the old Uninstall key is gone, the old own key is not.
    #[test]
    fn a_stop_between_the_two_old_deletions_is_still_ours() {
        let mut io = Fake::with_old("GoatTech");
        // calls: write_new 0, read_new 1, rename_links 2, mark_cleanup 3,
        // delete_old_uninstall 4, delete_old_own 5.
        io.fail_at = Some(5);
        let d = io.decide();
        assert!(matches!(run(&d, &mut io), Outcome::Failed(_)));
        assert!(!io.old_uninstall && io.old_own && io.pending);
        let seen = io.old_keys().unwrap();
        assert_eq!(seen.publisher, None);
        assert!(matches!(io.decide(), Decision::FinishCleanup(_)));
        // Without the journal flag the same keys are not ours.
        assert_eq!(
            decide(&io.new_keys(), Some(&seen), false),
            Decision::AlreadyNew
        );
    }

    // F4: the same stop, and then the new entry stops reading back as a whole (a value drifted).
    // The leftover old own key and the flag are still cleaned up, after the new entry is
    // written again from the new own key.
    #[test]
    fn a_stopped_cleanup_is_finished_when_the_new_entry_stops_validating() {
        let mut io = Fake::with_old("GoatTech");
        io.fail_at = Some(5);
        let d = io.decide();
        let _ = run(&d, &mut io);
        io.fail_at = None;
        io.calls.clear();
        io.new_valid = false;
        assert_eq!(
            io.decide(),
            Decision::FinishCleanup(reg(OLD_DIR)),
            "a broken new entry must not strand the old keys"
        );
        assert_eq!(run(&io.decide(), &mut io), Outcome::Done);
        assert!(io.nothing_left() && io.new_valid);
        assert_eq!(io.calls[..3], ["read_new", "write_new", "read_new"]);
    }

    // F5: a new entry at a newer version that does not validate, with the old install still
    // ours at the older version. The newer version is kept.
    #[test]
    fn a_broken_newer_entry_is_repaired_without_going_back_to_the_old_version() {
        let mut io = Fake::with_old("GoatTech");
        io.new = Some(reg_v(OLD_DIR, "2.0.0"));
        io.new_valid = false;
        assert_eq!(
            io.decide(),
            Decision::FinishCleanup(reg_v(OLD_DIR, "2.0.0"))
        );
        assert_eq!(run(&io.decide(), &mut io), Outcome::Done);
        assert_eq!(io.new, Some(reg_v(OLD_DIR, "2.0.0")));
        assert!(io.new_valid && io.nothing_left());
    }

    // The same with no old keys: the entry is repaired from the own key, version kept.
    #[test]
    fn a_broken_entry_without_old_keys_is_repaired_from_the_own_key() {
        let mut io = Fake {
            new: Some(reg_v(OLD_DIR, "2.0.0")),
            new_valid: false,
            ..Default::default()
        };
        let d = io.decide();
        assert_eq!(d, Decision::Repair(reg_v(OLD_DIR, "2.0.0")));
        assert_eq!(run(&d, &mut io), Outcome::Done);
        assert_eq!(io.new, Some(reg_v(OLD_DIR, "2.0.0")));
        assert!(io.new_valid);
        // Nothing of the old install is involved.
        assert_eq!(io.calls, ["write_new", "read_new"]);
    }

    // When the own key itself lacks a value, nothing newer is known: the old install that is
    // ours supplies it.
    #[test]
    fn an_own_key_without_its_version_is_adopted_from_the_old_install() {
        let mut io = Fake::with_old("GoatTech");
        io.new = Some(reg_v(OLD_DIR, ""));
        io.new_valid = false;
        assert_eq!(io.new_keys(), NewKeys::Broken(None));
        assert_eq!(io.decide(), Decision::Adopt(reg(OLD_DIR)));
    }

    // A broken new entry never lets an upstream install be touched.
    #[test]
    fn a_broken_new_entry_does_not_make_an_upstream_install_ours() {
        let mut io = Fake::with_old("Horizon");
        io.new = Some(reg_v(OLD_DIR, "2.0.0"));
        io.new_valid = false;
        assert_eq!(io.decide(), Decision::Repair(reg_v(OLD_DIR, "2.0.0")));
        assert_eq!(run(&io.decide(), &mut io), Outcome::Done);
        assert!(io.old_uninstall && io.old_own && !io.pending);
    }

    // The flag does not make an upstream install ours when the upstream launcher is installed
    // again over a half-finished clean-up.
    #[test]
    fn an_upstream_install_after_a_stopped_cleanup_is_left_alone() {
        let mut io = Fake::with_old("GoatTech");
        io.fail_at = Some(5);
        let d = io.decide();
        let _ = run(&d, &mut io);
        io.old_uninstall = true;
        io.publisher = "Horizon".to_string();
        io.fail_at = None;
        io.calls.clear();
        assert_eq!(io.decide(), Decision::AlreadyNew);
        assert_eq!(run(&io.decide(), &mut io), Outcome::Skipped);
        assert!(io.old_uninstall && io.old_own);
    }

    #[test]
    fn only_the_flag_left_is_cleared() {
        let n = whole(OLD_DIR);
        assert!(matches!(decide(&n, None, true), Decision::FinishCleanup(_)));
        assert_eq!(decide(&n, None, false), Decision::AlreadyNew);
    }

    #[test]
    fn running_again_after_a_clean_adoption_does_nothing() {
        assert_eq!(decide(&whole(OLD_DIR), None, false), Decision::AlreadyNew);
    }

    #[test]
    fn new_keys_for_another_folder_leave_the_old_install_alone() {
        let o = old(Some("GoatTech"), true);
        let n = whole(r"D:\Games\GoatTech Launcher");
        assert_eq!(decide(&n, Some(&o), false), Decision::AlreadyNew);
        assert_eq!(decide(&n, Some(&o), true), Decision::AlreadyNew);
    }

    #[test]
    fn folders_compare_without_case_or_a_trailing_slash() {
        assert!(same_dir(
            r"C:\A\Horizon Launcher\",
            r"c:\a\horizon launcher"
        ));
        assert!(!same_dir(
            r"C:\A\Horizon Launcher",
            r"C:\A\GoatTech Launcher"
        ));
    }

    // A write that stops between values leaves a partial new entry that does not read back as
    // whole, so the next start writes it again; the old keys are untouched until then.
    #[test]
    fn a_partial_new_entry_is_written_again_not_trusted() {
        for partial_own in [false, true] {
            let mut io = Fake::with_old("GoatTech");
            io.fail_at = Some(0);
            io.stops_after_own = partial_own;
            let d = io.decide();
            assert!(matches!(run(&d, &mut io), Outcome::Failed(_)));
            assert!(io.new.is_some() && !io.new_valid);
            assert!(io.old_uninstall && io.old_own);
            io.fail_at = None;
            io.calls.clear();
            let again = io.decide();
            // Own key whole: repaired from it, with the old keys cleaned up. Own key short of
            // its version: adopted from the old install.
            if partial_own {
                assert_eq!(again, Decision::FinishCleanup(reg(OLD_DIR)));
            } else {
                assert_eq!(again, Decision::Adopt(reg(OLD_DIR)));
            }
            assert_eq!(run(&again, &mut io), Outcome::Done);
            assert!(io.new_valid && io.nothing_left());
        }
    }

    // FinishCleanup does not delete anything while the new entry is not whole: it writes it
    // again first.
    #[test]
    fn finish_cleanup_repairs_a_partial_new_entry_first() {
        let mut io = Fake::with_old("GoatTech");
        io.new = Some(reg(OLD_DIR));
        io.new_valid = false;
        let d = Decision::FinishCleanup(reg(OLD_DIR));
        assert_eq!(run(&d, &mut io), Outcome::Done);
        assert_eq!(
            io.calls[..4],
            ["read_new", "write_new", "read_new", "rename_links"]
        );
        assert!(io.new_valid && io.nothing_left());
    }

    // The repair write itself failing (before the change, or after it) stops the run at once,
    // with no old key touched.
    #[test]
    fn a_failed_repair_write_stops_the_run() {
        for after in [false, true] {
            let mut io = Fake::with_old("GoatTech");
            io.new = Some(reg(OLD_DIR));
            io.new_valid = false;
            io.fail_at = Some(1);
            io.after = after;
            let d = Decision::FinishCleanup(reg(OLD_DIR));
            assert!(
                matches!(run(&d, &mut io), Outcome::Failed(_)),
                "after {after}"
            );
            assert_eq!(io.calls, ["read_new", "write_new"], "after {after}");
            assert!(
                io.old_uninstall && io.old_own && !io.pending,
                "after {after}"
            );
        }
        let mut io = Fake {
            fail_at: Some(0),
            ..Default::default()
        };
        let d = Decision::Repair(reg(OLD_DIR));
        assert!(matches!(run(&d, &mut io), Outcome::Failed(_)));
        assert_eq!(io.calls, ["write_new"]);
    }

    // A write that errors after it changed things is still a failure: nothing runs after it,
    // not even the read-back.
    #[test]
    fn an_initial_write_error_is_not_swallowed() {
        for after in [false, true] {
            let mut io = Fake::with_old("GoatTech");
            io.fail_at = Some(0);
            io.after = after;
            let d = Decision::Adopt(reg(OLD_DIR));
            assert!(
                matches!(run(&d, &mut io), Outcome::Failed(_)),
                "after {after}"
            );
            assert_eq!(io.calls, ["write_new"], "after {after}");
            assert!(!io.links_new && !io.pending, "after {after}");
        }
    }

    // If the new entry cannot be read back at all, no old key is deleted, either way in.
    #[test]
    fn nothing_is_deleted_while_the_new_entry_does_not_read_back() {
        for d in [
            Decision::Adopt(reg(OLD_DIR)),
            Decision::FinishCleanup(reg(OLD_DIR)),
            Decision::Repair(reg(OLD_DIR)),
        ] {
            let mut io = Fake::with_old("GoatTech");
            io.read_fails = true;
            assert!(matches!(run(&d, &mut io), Outcome::Failed(_)), "{d:?}");
            assert!(io.old_uninstall && io.old_own && !io.pending, "{d:?}");
            assert!(!io.links_new, "{d:?}");
            for c in &io.calls {
                assert!(
                    ![
                        "rename_links",
                        "mark_cleanup",
                        "delete_old_uninstall",
                        "delete_old_own"
                    ]
                    .contains(c),
                    "{d:?} {c}"
                );
            }
        }
    }

    // A dead leftover of an old build's uninstall: only our own Uninstall entry, only when its
    // program is gone, and only that key.
    #[test]
    fn a_lone_dead_uninstall_entry_of_ours_is_swept() {
        let lone = OldKeys {
            registration: None,
            uninstall_present: true,
            publisher: Some("GoatTech".to_string()),
            exe_present: false,
        };
        for new in [NewKeys::Absent, whole(r"D:\Games\GoatTech Launcher")] {
            assert_eq!(decide(&new, Some(&lone), false), Decision::SweepUninstall);
        }
        let mut io = Fake::with_old("GoatTech");
        io.old_own = false;
        assert_eq!(run(&Decision::SweepUninstall, &mut io), Outcome::Done);
        assert_eq!(io.calls, ["delete_old_uninstall"]);
        assert!(!io.old_uninstall && !io.pending);
    }

    #[test]
    fn other_lone_leftovers_are_not_swept() {
        let lone = |publisher: Option<&str>, exe_present: bool, own: bool| OldKeys {
            registration: own.then(|| reg(OLD_DIR)),
            uninstall_present: publisher.is_some(),
            publisher: publisher.map(str::to_string),
            exe_present,
        };
        // Upstream's entry, an entry with no publisher, an entry whose program is still there.
        assert_eq!(
            decide(
                &NewKeys::Absent,
                Some(&lone(Some("Horizon"), false, false)),
                false
            ),
            Decision::LeaveAlone
        );
        assert_ne!(
            decide(
                &NewKeys::Absent,
                Some(&lone(Some("GoatTech"), true, false)),
                false
            ),
            Decision::SweepUninstall
        );
        assert_ne!(
            decide(
                &NewKeys::Absent,
                Some(&lone(Some("GoatTechX"), false, false)),
                false
            ),
            Decision::SweepUninstall
        );
        // An old own key with no flag and no Uninstall key stays as it is.
        assert_eq!(
            decide(&NewKeys::Absent, Some(&lone(None, true, true)), false),
            Decision::LeaveAlone
        );
        // A whole install (both keys) is never swept.
        assert_ne!(
            decide(
                &NewKeys::Absent,
                Some(&lone(Some("GoatTech"), false, true)),
                false
            ),
            Decision::SweepUninstall
        );
    }

    // The start-up check: only the launcher that runs from the registered folder tidies it,
    // and not as the setup or uninstall window.
    #[test]
    fn start_up_tidies_only_from_the_registered_folder() {
        assert!(may_tidy_at_start(OLD_DIR, Some(OLD_DIR), false));
        assert!(may_tidy_at_start(
            &format!("{OLD_DIR}\\"),
            Some(&OLD_DIR.to_lowercase()),
            false
        ));
        assert!(!may_tidy_at_start(
            r"C:\Users\p\Downloads",
            Some(OLD_DIR),
            false
        ));
        assert!(!may_tidy_at_start(OLD_DIR, None, false));
    }

    #[test]
    fn start_up_does_not_tidy_in_setup_mode() {
        assert!(!may_tidy_at_start(OLD_DIR, Some(OLD_DIR), true));
    }
}
