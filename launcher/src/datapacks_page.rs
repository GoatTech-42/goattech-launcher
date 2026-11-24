//! A world's data packs, opened under its row on Home (`horizon_launcher::datapacks`): what is
//! in `datapacks/`, Add (a file picker) or a zip dropped on the world, and Remove (to the trash,
//! after a confirmation).
use super::*;
use gpui::ExternalPaths;
use horizon_launcher::datapacks::{self, DataPack};
use horizon_launcher::worlds::World;
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct WorldPacks {
    /// The world whose packs are open, and its packs as last read.
    open: Option<PathBuf>,
    list: Vec<DataPack>,
    pub(super) confirm_remove: Option<String>,
    note: Option<Result<String, String>>,
}

impl WorldPacks {
    /// Reads the open world's packs again, after something else changed them.
    pub(super) fn reread(&mut self) {
        if let Some(world) = &self.open {
            self.list = datapacks::list(world);
        }
    }
}

impl Launcher {
    /// Opens or closes the packs of `world` under its row.
    pub(super) fn toggle_world_packs(&mut self, world: &World) {
        if self.world_packs.open.as_ref() == Some(&world.path) {
            self.world_packs = WorldPacks::default();
        } else {
            self.world_packs = WorldPacks {
                open: Some(world.path.clone()),
                list: datapacks::list(&world.path),
                ..WorldPacks::default()
            };
        }
    }

    pub(super) fn world_packs_open(&self, world: &World) -> bool {
        self.world_packs.open.as_ref() == Some(&world.path)
    }

    /// Copies dropped or picked zips into `world`, and opens its packs to show them.
    pub(super) fn add_world_packs(&mut self, world: PathBuf, files: Vec<PathBuf>) {
        let added: Result<Vec<String>, String> = files
            .iter()
            .map(|f| datapacks::add(&world, f).map(|p| p.file))
            .collect();
        self.world_packs = WorldPacks {
            list: datapacks::list(&world),
            note: Some(added.map(|names| {
                trf(
                    "added {names}: the game turns it on when the world opens.",
                    &[("names", &names.join(", "))],
                )
            })),
            open: Some(world),
            confirm_remove: None,
        };
    }

    fn choose_world_packs(&mut self, world: PathBuf, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(tr("add").into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                let _ = this.update(cx, |this, cx| {
                    this.add_world_packs(world, paths);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// A world row that takes dropped data packs.
    pub(super) fn world_drop_target(
        &self,
        i: usize,
        world: &World,
        row: Div,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let t = self.theme;
        let path = world.path.clone();
        div()
            .id(SharedString::from(format!("world-drop-{i}")))
            .border_2()
            .mx(px(-4.))
            .p(px(2.))
            .border_color(gpui::transparent_black())
            .drag_over::<ExternalPaths>(move |s, _, _, _| s.border_color(t.accent))
            .on_drop(cx.listener(move |this, paths: &ExternalPaths, _, cx| {
                this.add_world_packs(path.clone(), paths.paths().to_vec());
                // Not the window's import as well.
                cx.stop_propagation();
                cx.notify();
            }))
            .child(row)
    }

    /// The open world's packs, under its row.
    pub(super) fn world_packs_panel(
        &mut self,
        world: &World,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        if !self.world_packs_open(world) {
            return None;
        }
        let t = self.theme;
        let path = world.path.clone();
        let mut panel = div()
            .flex()
            .flex_col()
            .gap(px(theme::chrome(6.)))
            .ml(px(theme::chrome(62.)))
            .mb(px(theme::chrome(6.)));
        let mut head = plate(&t)
            .flex()
            .flex_col()
            .gap(px(theme::chrome(6.)))
            .px(px(theme::chrome(12.)))
            .py(px(theme::chrome(8.)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .flex_wrap()
                    .gap(px(theme::chrome(6.)))
                    .child(micro(tr("data packs")).text_color(t.muted))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(theme::chrome(6.)))
                            .child(
                                scaled_button("world-packs-add", "add data pack…", Kind::Ghost, &t)
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.choose_world_packs(path.clone(), cx);
                                        cx.notify();
                                    })),
                            )
                            .child({
                                let dir = world.path.join("datapacks");
                                scaled_button("world-packs-folder", "open folder", Kind::Ghost, &t)
                                    .on_click(move |_, _, cx| {
                                        let _ = std::fs::create_dir_all(&dir);
                                        cx.open_with_system(&dir);
                                    })
                            }),
                    ),
            );
        let mut failed = None;
        match &self.world_packs.note {
            Some(Ok(m)) => {
                head = head.child(text(theme::CAPTION).text_color(t.muted).child(m.clone()))
            }
            Some(Err(e)) => failed = Some(notice(e.clone(), &t)),
            None => {}
        }
        if self.world_packs.list.is_empty() {
            head = head.child(text(theme::CAPTION).text_color(t.muted).child(tr(
                "none yet: drop a data pack .zip on the world, or use add data pack…",
            )));
        }
        panel = panel.child(head).children(failed);
        for (i, pack) in self.world_packs.list.clone().into_iter().enumerate() {
            let confirming = self.world_packs.confirm_remove.as_ref() == Some(&pack.file);
            let mut actions = div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(theme::chrome(6.)));
            if confirming {
                let (world, gone) = (world.path.clone(), pack.clone());
                actions = actions
                    .child(
                        text(theme::CAPTION)
                            .text_color(t.danger)
                            .child(tr("move it to the trash?")),
                    )
                    .child(
                        scaled_button(
                            SharedString::from(format!("world-pack-keep-{i}")),
                            "keep",
                            Kind::Ghost,
                            &t,
                        )
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                this.world_packs.confirm_remove = None;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        scaled_button(
                            SharedString::from(format!("world-pack-trash-{i}")),
                            "move to trash",
                            Kind::Secondary,
                            &t,
                        )
                        .on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                let note = datapacks::remove(&world, &gone).map(|()| {
                                    trf("{name} moved to the trash.", &[("name", &gone.file)])
                                });
                                this.world_packs.list = datapacks::list(&world);
                                this.world_packs.note = Some(note);
                                this.world_packs.confirm_remove = None;
                                cx.notify();
                            },
                        )),
                    );
            } else {
                let file = pack.file.clone();
                if !pack.enabled {
                    actions = actions.child(
                        text(theme::CAPTION)
                            .text_color(t.muted)
                            .child(tr("off in the game")),
                    );
                }
                actions = actions.child(
                    scaled_button(
                        SharedString::from(format!("world-pack-remove-{i}")),
                        "remove",
                        Kind::Ghost,
                        &t,
                    )
                    .on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.world_packs.confirm_remove = Some(file.clone());
                            cx.notify();
                        },
                    )),
                );
            }
            panel = panel.child(
                plate(&t)
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(theme::chrome(12.)))
                    .px(px(theme::chrome(12.)))
                    .py(px(theme::chrome(6.)))
                    .child(blocks::icon("package", theme::chrome(16.), t.muted))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                text(theme::LABEL)
                                    .text_color(t.ink)
                                    .truncate()
                                    .child(pack.file.clone()),
                            )
                            .child(
                                text(theme::CAPTION)
                                    .text_color(t.muted)
                                    .truncate()
                                    .child(pack.description.clone()),
                            ),
                    )
                    .child(actions.w_full()),
            );
        }
        Some(panel)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn data_pack_actions_wrap_and_confirmation_gets_its_own_row() {
        let source = include_str!("datapacks_page.rs");
        let panel = source
            .split("pub(super) fn world_packs_panel(")
            .nth(1)
            .unwrap()
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        assert!(panel.matches(".flex_wrap()").count() >= 4);
        assert!(panel.contains(".child(actions.w_full())"));
        for id in [
            "world-packs-add",
            "world-packs-folder",
            "world-pack-keep-",
            "world-pack-trash-",
        ] {
            assert!(panel.contains(id));
        }
    }
}

// SCRATCH ONLY: local synthetic data; real data-pack panel, not the full Home shell.
impl Launcher {
    pub(super) fn datapack_capture_mode() -> Option<String> {
        std::env::var("HORIZON_DATAPACK_CAPTURE")
            .ok()
            .filter(|s| matches!(s.as_str(), "packs-en" | "packs-fr"))
    }

    fn capture_world() -> World {
        World {
            folder: "synthetic-world-p40".into(),
            path: std::env::temp_dir().join("synthetic-world-p40-not-on-disk"),
            name: "Synthetic data pack world".into(),
            mode: "Survival",
            last_played: std::time::UNIX_EPOCH,
            version: Some("fixture".into()),
            size: 0,
            icon: None,
        }
    }

    pub(super) fn seed_datapack_capture(&mut self) {
        let world = Self::capture_world();
        let list: Vec<DataPack> = (0..14).map(|i| {
            let file = format!("{i:02}-synthetic-long-filename-landscape-generation-and-village-trading-compatibility-pack.zip");
            DataPack {
                path: world.path.join("datapacks").join(&file), file,
                description: format!("PACK {i:02} - Synthetic long description for landscape generation, village trading and compatibility. Last pack is PACK 13."),
                enabled: i % 2 == 0,
            }
        }).collect();
        let confirm_remove = Some(list[0].file.clone());
        self.world_packs = WorldPacks {
            open: Some(world.path),
            list,
            confirm_remove,
            note: None,
        };
    }

    pub(super) fn datapack_capture_view(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let mode = Self::datapack_capture_mode()?;
        i18n::set(if mode == "packs-fr" {
            i18n::Language::French
        } else {
            i18n::Language::English
        });
        let t = self.theme;
        let panel = self.world_packs_panel(&Self::capture_world(), cx);
        let body = div()
            .flex()
            .flex_col()
            .w_full()
            .min_w_0()
            .px(px(theme::page_gutter()))
            .pt(px(24.))
            .pb(px(theme::chrome(48.)))
            .child(text(theme::LABEL).child(
                "SYNTHETIC P40: real panel, shell-width spacer. Scroll only; no control clicks.",
            ))
            .children(panel);
        let handle = self.content_scroll.handle.clone();
        let receipt = gpui::canvas(move |_, _, _| {
            use std::io::Write;
            thread_local! {
                static LAST: std::cell::RefCell<Option<(f32, f32, f32)>> = const { std::cell::RefCell::new(None) };
            }
            let state = (f32::from(handle.bounds().size.height), f32::from(handle.max_offset().y), f32::from(handle.offset().y));
            LAST.with(|last| {
                if *last.borrow() == Some(state) { return; }
                *last.borrow_mut() = Some(state);
                if let Ok(cap) = std::env::var("CAP") {
                    let path = std::path::PathBuf::from(cap).join("scroll-state.txt");
                    if std::fs::metadata(&path).map(|m| m.len() < 16384).unwrap_or(true) {
                        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                            let _ = writeln!(file, "scroll-state: viewport={} max={} offset={}", state.0, state.1, state.2);
                        }
                    }
                }
            });
        }, |_, _, _, _| {}).h(px(0.)).flex_none();
        Some(
            div()
                .size_full()
                .bg(t.page)
                .text_color(t.ink)
                .font_family(theme::FONT)
                .child(
                    div()
                        .flex()
                        .size_full()
                        .child(div().flex_none().w(px(theme::sidebar())))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .child(receipt)
                                .child(
                                    self.content_scroll
                                        .wrap(
                                            div()
                                                .id("datapack-capture-root")
                                                .flex_1()
                                                .min_h_0()
                                                .overflow_y_scroll()
                                                .child(body),
                                            &t,
                                        )
                                        .flex_1(),
                                ),
                        ),
                ),
        )
    }
}
