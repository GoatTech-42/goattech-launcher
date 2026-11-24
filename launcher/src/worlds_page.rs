//! The selected instance's worlds on Home (`horizon_launcher::worlds`): each with its picture,
//! mode, version, when it was last played and its size, and Back up, Copy, Delete (to the
//! trash, after a confirmation) and Open folder. Reading sizes and copying run in the background.
use super::*;
use horizon_launcher::worlds::{self, World};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct Worlds {
    /// Switching away and back creates a different state, even for the same game folder.
    identity: Arc<()>,
    /// The game folder the list is of, and the list as last read: kept on screen while it is
    /// read again, so the page doesn't jump.
    home: Option<PathBuf>,
    list: Vec<World>,
    fresh: bool,
    reading: bool,
    /// The world being backed up, copied or deleted now.
    busy: Option<String>,
    pub(super) confirm_delete: Option<String>,
    /// What the last action did (a backup's zip), or why it failed.
    done: Option<Result<(String, Option<PathBuf>), String>>,
}

impl Worlds {
    fn finish_read(&mut self, identity: &Arc<()>, list: Vec<World>) {
        if self.reading && Arc::ptr_eq(&self.identity, identity) {
            self.list = list;
            self.fresh = true;
            self.reading = false;
        }
    }

    fn finish_action(
        &mut self,
        identity: &Arc<()>,
        done: Result<(String, Option<PathBuf>), String>,
    ) {
        if self.busy.is_some() && Arc::ptr_eq(&self.identity, identity) {
            self.busy = None;
            self.done = Some(done);
            self.stale();
        }
    }

    /// Read again on the next frame: the game may have saved, or an action changed the list.
    pub(super) fn stale(&mut self) {
        self.fresh = false;
    }
}

/// "today", "yesterday", "3 days ago", else the date.
pub(super) fn ago(when: std::time::SystemTime) -> String {
    let when: chrono::DateTime<chrono::Local> = when.into();
    let days = (chrono::Local::now().date_naive() - when.date_naive()).num_days();
    match days {
        ..=0 => tr("today").into(),
        1 => tr("yesterday").into(),
        2..=13 => trf("{days} days ago", &[("days", &days.to_string())]),
        _ => when.format("%Y-%m-%d").to_string(),
    }
}

enum Action {
    Backup,
    Copy,
    Delete,
}

impl Launcher {
    /// Reads the worlds of `home` in the background when the list is missing or of another
    /// instance.
    fn load_worlds(&mut self, home: PathBuf, cx: &mut Context<Self>) {
        if self.worlds.home.as_ref() != Some(&home) {
            self.worlds = Worlds {
                home: Some(home.clone()),
                ..Worlds::default()
            };
        }
        if self.worlds.fresh || self.worlds.reading {
            return;
        }
        let identity = self.worlds.identity.clone();
        self.worlds.reading = true;
        let read = home.clone();
        let task = cx
            .background_executor()
            .spawn(async move { worlds::list(&read) });
        cx.spawn(async move |this, cx| {
            let list = task.await;
            let _ = this.update(cx, |this, cx| {
                this.worlds.finish_read(&identity, list);
                cx.notify();
            });
        })
        .detach();
    }

    fn world_action(&mut self, action: Action, world: World, cx: &mut Context<Self>) {
        let Some(home) = self.worlds.home.clone() else {
            return;
        };
        if self.worlds.busy.is_some() {
            return;
        }
        let identity = self.worlds.identity.clone();
        self.worlds.busy = Some(world.folder.clone());
        self.worlds.confirm_delete = None;
        self.worlds.done = None;
        let task = cx.background_executor().spawn(async move {
            match action {
                Action::Backup => worlds::backup(&home, &world).map(|zip| {
                    (
                        trf("{name} backed up.", &[("name", &world.name)]),
                        Some(zip),
                    )
                }),
                Action::Copy => worlds::copy(&world)
                    .map(|c| (trf("copied to {name}.", &[("name", &c.name)]), None)),
                Action::Delete => worlds::delete(&world).map(|()| {
                    (
                        trf("{name} moved to the trash.", &[("name", &world.name)]),
                        None,
                    )
                }),
            }
        });
        cx.spawn(async move |this, cx| {
            let done = task.await;
            let _ = this.update(cx, |this, cx| {
                this.worlds.finish_action(&identity, done);
                cx.notify();
            });
        })
        .detach();
    }

    /// How many worlds the selected instance has; None while they are read.
    pub(super) fn worlds_count(&mut self, cx: &mut Context<Self>) -> Option<usize> {
        let home = self.selected_instance()?.home.clone();
        self.load_worlds(home, cx);
        self.worlds.fresh.then_some(self.worlds.list.len())
    }

    /// Home's Worlds tab, for the selected instance.
    pub(super) fn worlds_section(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let t = self.theme;
        let home = self.selected_instance()?.home.clone();
        self.load_worlds(home.clone(), cx);
        let list = self.worlds.list.clone();
        let mut section = div().flex().flex_col().gap(px(8.));
        match &self.worlds.done {
            Some(Ok((message, zip))) => {
                let mut line = div().flex().items_center().gap(px(12.)).child(
                    text(theme::CAPTION)
                        .text_color(t.muted)
                        .child(message.clone()),
                );
                if let Some(zip) = zip.clone() {
                    let dir = zip.parent().map(Path::to_path_buf).unwrap_or(zip);
                    line = line.child(
                        button("world-backups", "open backups", Kind::Ghost, &t)
                            .on_click(move |_, _, cx| cx.open_with_system(&dir)),
                    );
                }
                section = section.child(line);
            }
            Some(Err(e)) => section = section.child(notice(e.clone(), &t)),
            None => {}
        }
        if list.is_empty() {
            let saves = home.join("saves");
            return Some(
                section.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            text(theme::CAPTION)
                                .text_color(t.muted)
                                .child(tr("no worlds yet: they appear here once you play one.")),
                        )
                        .child(
                            button("worlds-folder", "open folder", Kind::Ghost, &t).on_click(
                                move |_, _, cx| {
                                    let _ = std::fs::create_dir_all(&saves);
                                    cx.open_with_system(&saves);
                                },
                            ),
                        ),
                ),
            );
        }
        for (i, w) in list.into_iter().enumerate() {
            let row = self.world_row(i, w.clone(), cx);
            section = section
                .child(self.world_drop_target(i, &w, row, cx))
                .children(self.world_packs_panel(&w, cx));
        }
        Some(section)
    }

    fn world_row(&mut self, i: usize, w: World, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let busy = self.worlds.busy.is_some();
        let this_busy = self.worlds.busy.as_ref() == Some(&w.folder);
        let confirming = self.worlds.confirm_delete.as_ref() == Some(&w.folder);
        let picture = match &w.icon {
            Some(icon) => div()
                .flex_none()
                .size(px(48.))
                .border_2()
                .border_color(t.outline)
                .child(gpui::img(icon.clone()).size(px(44.)))
                .into_any_element(),
            None => div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(48.))
                .border_2()
                .border_color(t.outline)
                .bg(t.well)
                .child(blocks::icon("image", 22., t.muted))
                .into_any_element(),
        };
        let mut facts = vec![tr(w.mode).to_string()];
        facts.extend(w.version.clone());
        facts.push(ago(w.last_played));
        facts.push(megabytes(w.size));
        let act = |id: &str, label: &'static str, kind: Kind| {
            let b = button(SharedString::from(format!("{id}-{i}")), label, kind, &t);
            if busy {
                disabled(b)
            } else {
                b
            }
        };
        let mut actions = div().flex().items_center().gap(px(6.));
        if confirming {
            let gone = w.clone();
            actions = actions
                .child(
                    text(theme::CAPTION)
                        .text_color(t.danger)
                        .child(tr("move it to the trash?")),
                )
                .child(act("world-keep", "keep", Kind::Ghost).on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.worlds.confirm_delete = None;
                        cx.notify();
                    },
                )))
                .child(
                    act("world-trash", "move to trash", Kind::Secondary).on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.world_action(Action::Delete, gone.clone(), cx);
                            cx.notify();
                        },
                    )),
                );
        } else if this_busy {
            actions = actions.child(
                text(theme::CAPTION)
                    .text_color(t.muted)
                    .child(tr("working…")),
            );
        } else {
            let (backup, copy, folder) = (w.clone(), w.clone(), w.path.clone());
            let (delete, play) = (w.folder.clone(), w.folder.clone());
            let play_button = act("world-play", "play", Kind::Secondary);
            let play_button = if !self.can_start_selected() {
                disabled(play_button)
            } else {
                play_button
            };
            actions = actions
                .child(
                    play_button.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.quick_play("--world", play.clone());
                        cx.notify();
                    })),
                )
                .child(
                    act("world-backup", "back up", Kind::Ghost).on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.world_action(Action::Backup, backup.clone(), cx);
                            cx.notify();
                        },
                    )),
                )
                .child(act("world-copy", "copy", Kind::Ghost).on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.world_action(Action::Copy, copy.clone(), cx);
                        cx.notify();
                    },
                )))
                .child(
                    act("world-delete", "delete", Kind::Ghost).on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            // Buttons look disabled while something runs, but still click.
                            if this.worlds.busy.is_none() {
                                this.worlds.confirm_delete = Some(delete.clone());
                            }
                            cx.notify();
                        },
                    )),
                )
                .child({
                    let world = w.clone();
                    let label = match self.world_packs_open(&w) {
                        true => "hide data packs",
                        false => "data packs",
                    };
                    act("world-packs", label, Kind::Ghost).on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.toggle_world_packs(&world);
                            cx.notify();
                        },
                    ))
                })
                .child(
                    act("world-folder", "open folder", Kind::Ghost)
                        .on_click(move |_, _, cx| cx.open_with_system(&folder)),
                );
        }
        plate(&t)
            .flex()
            .items_center()
            .gap(px(14.))
            .px(px(14.))
            .py(px(10.))
            .child(picture)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        blocks::pixel(theme::chrome(15.), FontWeight::MEDIUM)
                            .text_color(t.ink)
                            .truncate()
                            .child(w.name.clone()),
                    )
                    .child(
                        text(theme::CAPTION)
                            .text_color(t.muted)
                            .truncate()
                            .child(facts.join(" · ")),
                    ),
            )
            .child(actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active(home: &str) -> Worlds {
        Worlds {
            home: Some(home.into()),
            reading: true,
            fresh: true,
            busy: Some("world".into()),
            ..Worlds::default()
        }
    }

    #[test]
    fn matching_world_completions_update_the_active_state_once() {
        let mut state = active("first");
        let identity = state.identity.clone();
        state.finish_read(&identity, Vec::new());
        assert!(!state.reading);
        assert!(state.fresh);
        let result = ("backed up".into(), Some(PathBuf::from("backup.zip")));
        state.finish_action(&identity, Ok(result.clone()));
        assert!(state.busy.is_none());
        assert!(!state.fresh);
        assert_eq!(state.done, Some(Ok(result)));
        state.finish_action(&identity, Err("duplicate".into()));
        assert!(matches!(state.done, Some(Ok(_))));
    }

    #[test]
    fn stale_world_completions_cannot_touch_different_or_reopened_state() {
        let old = active("first");
        for home in ["second", "first"] {
            let mut current = active(home);
            current.fresh = false;
            current.finish_read(&old.identity, Vec::new());
            assert!(current.reading);
            assert!(!current.fresh);
            current.finish_action(
                &old.identity,
                Ok(("old backup".into(), Some("old.zip".into()))),
            );
            assert_eq!(current.busy.as_deref(), Some("world"));
            assert!(current.done.is_none());
            current.finish_action(&old.identity, Err("old error".into()));
            assert_eq!(current.busy.as_deref(), Some("world"));
            assert!(current.done.is_none());
        }
    }
}
