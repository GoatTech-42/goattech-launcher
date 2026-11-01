//! Home in the night look (docs/design-system.md, "Night"): the selected instance's name over the
//! sky with what it plays and whether it is ready, Play and the actions beside it, and a card with
//! what's new and joining a friend.
use super::*;
use crate::plate::alpha;
use horizon_launcher::instances::{Instance, Kind as InstanceKind};
use crate::shell::{action, c, label, Action};
use theme::night;

impl Launcher {
    /// The focal point (concept Home): a card naming what plays next - its name, fact chips
    /// (kind, loader, Minecraft, mods), its state - with Play at its foot. No `tag`: a Java
    /// instance's Minecraft (`game` then names its loader too).
    pub(super) fn hero(
        &mut self,
        tag: &str,
        game: &str,
        prepared: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let t = self.theme;
        let n = t.night;
        let instance = self.selected_instance().cloned();
        let name = instance
            .as_ref()
            .map_or(
                if tag.is_empty() { "Minecraft" } else { "Horizon" },
                |i| i.name.as_str(),
            )
            .to_string();
        let java = instance.as_ref().and_then(|i| i.java.as_ref()).cloned();
        // The subtitle's facts: kind, loader, Minecraft, mods. Real values only - a fact is
        // absent, never a placeholder, while it is unread.
        let mut facts: Vec<String> = Vec::new();
        match &java {
            Some(j) => {
                facts.push("Java".to_string());
                if let Some(l) = &j.loader {
                    facts.push(format!("{} {}", l.kind.name(), l.version));
                }
                facts.push(format!("Minecraft {}", j.minecraft));
            }
            None if !tag.is_empty() => {
                facts.push(
                    format!(
                        "Horizon {}",
                        horizon_launcher::nightly::label(tag)
                            .unwrap_or_else(|| tag.trim_start_matches('v').to_string())
                    ),
                );
                facts.push(format!("Minecraft {game}"));
            }
            None => {}
        }
        if let Some(mods) = self.instance_mods_count(cx) {
            facts.push(trf("{n} mods", &[("n", &mods.to_string())]));
        }
        let ready = prepared || java.is_some();
        // "Ready" means Play works, files and ownership both: the gates (account::java_player,
        // launch_for_game) let a Java instance start only with an account that owns Minecraft -
        // the developer offline option never exempts it - while a Horizon one also starts under
        // that explicit option. `owner` is the keychain's last ownership answer; unread is
        // unknown, never silently ready.
        let signed = match self.owner {
            Some(owner) => owner || (java.is_none() && self.settings.dev_offline),
            None => false,
        };
        // The concept Home: everything centered over the open sky - the instance's name, the
        // player stage, the split Play, its state as a word, then the fact chips.
        let mut hero = div()
            .relative()
            .flex()
            .flex_none()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .w_full()
            .child(
                shell::text(night::DISPLAY, c(n.ink))
                    .font_family(theme::PIXEL)
                    .text_size(px(26.))
                    .line_height(px(30.))
                    .child(name.clone()),
            )
            .child(self.home_stage())
            .child(self.play_action(ready, &facts.join("  ·  "), cx))
            .child(
                shell::text(
                    night::CAPTION,
                    if prepared && signed { t.success } else { t.warn },
                )
                .font_family(theme::PIXEL)
                .text_size(px(10.))
                .child(
                    tr(if !prepared {
                        "Needs preparing"
                    } else if signed {
                        "Ready to launch"
                    } else if self.owner.is_none() {
                        "Checking sign-in…"
                    } else {
                        "Sign in to launch"
                    })
                    .to_string()
                    .to_uppercase(),
                ),
            )
            .children(self.playing.map(|playing| {
                shell::text(night::CAPTION, c(n.muted))
                    .relative()
                    .child(trf("{n} playing Horizon now", &[("n", &thousands(playing))]))
            }))
            // Words sit on a plate where the sky would cross them (docs/design-system.md).
            .child(
                shell::text(night::CAPTION, c(n.muted))
                    .bg(alpha(n.card, 0.6))
                    .px(px(10.))
                    .py(px(4.))
                    .max_w(px(560.))
                    .child(
                        tr("Own runtime, mods and saves - nothing mixes with your other instances.")
                            .to_string(),
                    ),
            )
;
        // A Java instance (no Horizon tag) isn't offered Horizon's updates.
        if let Some(release) = self.updates.horizon.as_ref().filter(|_| !tag.is_empty()) {
            let installing = self.installing.as_ref().filter(|(t, _)| *t == release.tag);
            let line = shell::text(night::CAPTION, c(n.muted))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().size(px(6.)).bg(c(n.accent_hi)))
                .child(trf(
                    "{version} available",
                    &[("version", update_name(release))],
                ));
            hero = hero.child(match installing {
                Some((_, p)) => line
                    .child(
                        div()
                            .w(px(120.))
                            .flex()
                            .child(progress_bar(Some(self.bar), &t)),
                    )
                    .child(format!("{:.0}%", p * 100.)),
                None => line.child(
                    div()
                        .id("install-update")
                        .text_color(c(n.accent_hi))
                        .font_weight(FontWeight::SEMIBOLD)
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.8))
                        .child(tr("Install"))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.install_update();
                            cx.notify();
                        })),
                ),
            });
        }
        // Players who answered the first-run question before the ping existed read about it
        // once here.
        // Hidden while the sign-in modal is up: it would cover the notice's Got it.
        if !self.settings.ping_notice_seen && !self.collection.consent.needs_asking() && !self.asking_owner() {
            hero = hero.child(
                crate::plate::plate(&n)
                    .flex()
                    .flex_none()
                    .flex_col()
                    .gap(px(6.))
                    .p(px(16.))
                    .w_full()
                    .max_w(px(560.))
                    .child(
                        shell::text(night::UI, c(n.ink))
                            .relative()
                            .child(tr(telemetry::PING_NAME)),
                    )
                    .child(
                        shell::text(night::CAPTION, c(n.muted))
                            .relative()
                            .child(tr(telemetry::PING)),
                    )
                    .child(
                        shell::text(night::UI, c(n.accent_hi))
                            .relative()
                            .id("ping-notice-ok")
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.8))
                            .child(tr("Got it"))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.settings.ping_notice_seen = true;
                                this.settings.save();
                                cx.notify();
                            })),
                    ),
            );
        }
        hero
    }


    /// Home's top bar (the concept): the wordmark and version at the left, New instance and the
    /// account at the right.
    pub(super) fn home_topbar(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let n = t.night;
        let (name, how) = self.who_plays();
        let face = match self.accounts.player() {
            account::Player::Microsoft(_) => self.accounts.selected().map(|a| a.uuid.clone()),
            _ => None,
        };
        let picture = match face {
            Some(uuid) => self.head(&uuid, 28.).into_any_element(),
            None => div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(28.))
                .bg(alpha(n.card_hi, n.card_hi_alpha))
                .child(blocks::icon("user", 16., c(n.muted)))
                .into_any_element(),
        };
        let account = div()
            .id("topbar-account")
            .relative()
            .flex()
            .items_center()
            .gap(px(10.))
            .h(px(40.))
            .px(px(10.))
            .bg(alpha(n.card, n.card_alpha))
            .border_1()
            .border_color(alpha(n.stroke, n.stroke_alpha))
            .cursor_pointer()
            .child(div().relative().child(picture))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .child(
                        shell::text(night::UI, c(n.ink))
                            .font_weight(FontWeight::SEMIBOLD)
                            .line_height(px(16.))
                            .child(name),
                    )
                    .child(
                        shell::text(night::CAPTION, c(n.muted))
                            .line_height(px(13.))
                            .child(how),
                    ),
            )
            .child(
                div()
                    .relative()
                    .child(blocks::icon("chevron-down", 16., c(n.muted))),
            )
            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                if this.closed_by_this_press(event) {
                    return;
                }
                this.account_menu = !this.account_menu;
                if this.account_menu {
                    // Who can play may have changed elsewhere since the window opened.
                    this.accounts = Accounts::load();
                }
                cx.notify();
            }));
        let account = div()
            .relative()
            .child(account)
            .children(self.account_menu(cx));
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(10.))
                    .child(
                        shell::text(night::TITLE, c(n.ink))
                            .font_family(theme::PIXEL)
                            .text_size(px(20.))
                            .child("goattech"),
                    )
                    .child(
                        shell::text(night::CAPTION, c(n.muted))
                            .font_family(theme::MONO)
                            .child(backend::VERSION),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        shell::action("topbar-new-instance", shell::Action::Raised, &t)
                            .child(
                                div()
                                    .relative()
                                    .child(blocks::icon("plus", 16., c(n.ink))),
                            )
                            .child(shell::label(tr("New instance").to_string()))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.pick_kind(new_instance::NewKind::Modded, cx);
                                cx.notify();
                            })),
                    )
                    .child(account),
            )
    }

    /// The concept's bottom row: every instance as a square card that selects it, then a dashed
    /// New instance card.
    pub(super) fn playing_next(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let n = t.night;
        if self.instances.list.len() < 2 {
            return div();
        }
        let selected_id = self.selected_instance().map(|i| i.id.clone());
        let mut row = div().flex().flex_wrap().gap(px(12.));
        for instance in self.instances.list.clone() {
            let active = selected_id.as_ref() == Some(&instance.id);
            let played = instances_page::played_line(&instance);
            // A Java instance never plays a Horizon nightly: say its Minecraft and loader,
            // never the launcher's version.
            let sub = match &instance.java {
                Some(j) => match &j.loader {
                    Some(l) => format!("Minecraft {} · {} {}", j.minecraft, l.kind.name(), l.version),
                    None => format!("Minecraft {}", j.minecraft),
                },
                None => self
                    .version_of(&instance)
                    .map(|tag| {
                        horizon_launcher::nightly::label(tag)
                            .unwrap_or_else(|| tag.trim_start_matches('v').to_string())
                    })
                    .unwrap_or_default(),
            };
            let clicked = instance.clone();
            row = row.child(
                div()
                    .id(SharedString::from(format!("next-{}", instance.id)))
                    .relative()
                    .flex()
                    .flex_none()
                    .flex_col()
                    .justify_center()
                    .gap(px(4.))
                    .w(px(220.))
                    .h(px(84.))
                    .px(px(14.))
                    .bg(alpha(n.card, n.card_alpha))
                    .border_1()
                    .border_color(alpha(n.stroke, n.stroke_alpha))
                    .cursor_pointer()
                    .when(active, |d| {
                        d.child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .bottom_0()
                                .w(px(2.))
                                .bg(c(n.ink)),
                        )
                    })
                    .child(
                        shell::text(night::UI, c(n.ink))
                            .w_full()
                            .truncate()
                            .child(instance.name.clone()),
                    )
                    .child(
                        shell::text(night::CAPTION, c(n.muted))
                            .text_size(px(10.))
                            .w_full()
                            .truncate()
                            .child(sub),
                    )
                    .child(
                        shell::text(night::CAPTION, c(n.muted))
                            .text_size(px(10.))
                            .w_full()
                            .truncate()
                            .child(played),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.select_instance(&clicked);
                        cx.notify();
                    })),
            );
        }
        row = row.child(
            div()
                .id("next-new-instance")
                .relative()
                .flex()
                .flex_none()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .w(px(220.))
                .h(px(84.))
                .border_1()
                .border_dashed()
                .border_color(alpha(n.stroke, n.stroke_alpha * 2.))
                .cursor_pointer()
                .child(
                    div()
                        .relative()
                        .child(blocks::icon("plus", 20., c(n.muted))),
                )
                .child(
                    shell::text(night::CAPTION, c(n.muted))
                        .text_size(px(10.))
                        .child(tr("New instance").to_string().to_uppercase()),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.pick_kind(new_instance::NewKind::Modded, cx);
                    cx.notify();
                })),
        );
        div()
            .flex()
            .flex_none()
            .flex_col()
            .gap(px(10.))
            .child(
                shell::text(night::CAPTION, c(n.muted))
                    .font_family(theme::PIXEL)
                    .text_size(px(10.))
                    .child(tr("Playing next").to_string().to_uppercase()),
            )
            .child(row)
    }

    /// The home stage's center (concept Home): the player model, 3D-projected from its skin,
    /// under its pixel nametag. A labelled fixture skin until the signed-in player's own skin
    /// renders here.
    pub(super) fn home_stage(&mut self) -> Div {
        let t = self.theme;
        let n = t.night;
        let scale = self.scale;
        let current = match &self.home_model {
            Some((s, i)) if *s == scale => Some(i.clone()),
            _ => None,
        };
        let baked = match current {
            Some(i) => Some(i),
            None => {
                let b = player_model::fixture().map(|png| {
                    art::bake(
                        &player_model::render(&png, -0.5),
                        player_model::W,
                        player_model::H,
                        6.,
                        scale,
                    )
                });
                if let Some(i) = &b {
                    self.home_model = Some((scale, i.clone()));
                }
                b
            }
        };
        let (who, _) = self.who_plays();
        let mut stage = div()
            .relative()
            .flex()
            .flex_none()
            .flex_col()
            .items_center()
            .gap(px(4.))
            .pt(px(8.))
            .child(
                shell::text(night::CAPTION, c(n.ink))
                    .font_family(theme::PIXEL)
                    .child(who.to_uppercase()),
            );
        if let Some(i) = baked {
            stage = stage.child(
                gpui::img(i)
                    .w(px(player_model::W as f32 * 6.))
                    .h(px(player_model::H as f32 * 6.)),
            );
        }
        stage.child(
            shell::text(night::CAPTION, c(n.muted)).child(tr("Preview player").to_string()),
        )
    }

    /// Play itself, the hero card's big button: while it prepares, the busy state and Cancel.
    /// `ready`: prepared, or a Java instance (its game comes with Play).
    pub(super) fn play_action(&self, ready: bool, subtitle: &str, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let running = self.client.is_some() || self.starting_client.is_some();
        if self.preparing {
            // Play waits for the steps below it; a download can stop (the next one goes on from
            // there).
            let download = download_progress();
            let busy = match download {
                Some((done, total)) => format!(
                    "{} {:.0}%",
                    tr("Getting ready…"),
                    done as f32 * 100. / total as f32
                ),
                None => tr("Getting ready…").to_string(),
            };
            return div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    action("play", Action::Main, &t)
                        .min_w(px(260.))
                        .cursor_default()
                        .child(label(busy)),
                )
                .children(download.map(|_| {
                    action("download-cancel", Action::Quiet, &t)
                        .child(label(tr("Cancel").to_string()))
                        .on_click(|_, _, _| {
                            horizon_launcher::mojang::CANCEL.store(true, Ordering::Relaxed)
                        })
                }));
        }
        let press = play_press(ready, self.owner, self.preparing || running);
        let live = !matches!(press, PlayPress::Nothing);
        // The concept's Play wears the instance's facts as a small second line.
        let mut play = action("play", Action::Main, &t)
            .min_w(px(320.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(3.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(
                                div()
                                    .relative()
                                    .child(blocks::icon("play", 20., c(t.night.on_accent))),
                            )
                            .child(label(
                                tr(if running { "Running" } else { "LAUNCH" }).to_string(),
                            )),
                    )
                    .when(!subtitle.is_empty(), |d| {
                        d.child(
                            shell::text(theme::MICRO, c(t.night.on_accent))
                                .text_size(px(10.))
                                .child(subtitle.to_uppercase()),
                        )
                    }),
            );
        match press {
            PlayPress::Nothing => play = play.opacity(0.6).cursor_default().tab_stop(false),
            PlayPress::Play => {
                play = play.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.picker_open = false;
                    this.play();
                    cx.notify();
                }))
            }
            PlayPress::PrepareThenPlay => {
                play = play.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.picker_open = false;
                    this.play_when_prepared = this.selected().cloned();
                    this.prepare(backend::MinecraftFrom::Install);
                    cx.notify();
                }))
            }
        }
        // The concept's split Play: LAUNCH with its attached chevron opening the version menu.
        // Both sit on a solid dark lip: the sky never shows through a dimmed button, and the
        // control wears the concept's chunky base.
        let mut split = div()
            .relative()
            .flex()
            .items_center()
            .child(
                crate::plate::cut(night::PIXEL_CORNER, 1, Some(alpha(t.night.page, 1.)), None)
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(3.))
                    .bottom(px(-3.)),
            )
            .child(play)
            .child(
                keyboard(
                    track_hover(
                        div()
                            .id("play-versions")
                            .relative()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .h(px(night::PLAY))
                            .w(px(56.))
                            .ml(px(3.))
                            .opacity(if live { 1. } else { 0.45 })
                            .cursor_pointer()
                            .child(
                                crate::plate::cut(
                                    night::PIXEL_CORNER,
                                    1,
                                    Some(alpha(t.night.stroke, 0.30)),
                                    None,
                                )
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full(),
                            )
                            .child(
                                div().relative().child(blocks::icon(
                                    "chevron-down",
                                    20.,
                                    c(t.night.on_accent),
                                )),
                            )
                            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                                if !this.closed_by_this_press(event) {
                                    this.picker_open = !this.picker_open;
                                }
                                cx.notify();
                            })),
                        SharedString::from("play-versions"),
                    ),
                    &t,
                ),
            );
        if let Some(menu) = self.picker_menu(cx) {
            split = split.child(menu);
        }
        div().flex().relative().items_center().child(split)
    }

    /// What else the instance offers, under the hero card while it is not preparing: its
    /// version (Horizon), its mods, its folder.
    pub(super) fn home_actions(&self, java: bool, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let folder = self.selected_instance().map(|i| i.home.clone());
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(10.))
            .child(
                action("home-mods", Action::Quiet, &t)
                    .child(label(tr("Mods").to_string()))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.go(Page::Mods);
                        cx.notify();
                    })),
            )
            .children(folder.map(|home| {
                action("home-folder", Action::Quiet, &t)
                    .child(label(tr("Folder").to_string()))
                    .on_click(move |_, _, cx| {
                        let _ = std::fs::create_dir_all(&home);
                        cx.open_with_system(&home);
                    })
            }))
    }

    /// The concept Home's "you" card, while nobody plays as a Microsoft account: a preview
    /// stands in until sign-in. A signed-in player already has the sidebar's account card.
    pub(super) fn you_card(&self, cx: &mut Context<Self>) -> Option<Div> {
        if matches!(self.accounts.player(), account::Player::Microsoft(_)) {
            return None;
        }
        let t = self.theme;
        let n = t.night;
        Some(
            crate::plate::plate(&n)
                .flex()
                .flex_none()
                .flex_col()
                .items_center()
                .gap(px(10.))
                .p(px(18.))
                .w(px(300.))
                .child(
                    shell::text(night::CAPTION, c(n.muted))
                        .relative()
                        .w_full()
                        .child(tr("You").to_string()),
                )
                .child(
                    div()
                        .relative()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(56.))
                        .rounded(px(8.))
                        .bg(alpha(n.card_hi, n.card_hi_alpha))
                        .child(blocks::icon("user", 24., c(n.muted))),
                )
                .child(
                    shell::text(night::CAPTION, c(n.muted)).relative().child(
                        tr("A stand-in plays for now - sign in with your Microsoft account to play as yourself.")
                            .to_string(),
                    ),
                )
                .child(
                    microsoft::sign_in_button("home-sign-in", &t).on_click(cx.listener(
                        |this, _: &ClickEvent, _, cx| {
                            this.sign_in_to_play();
                            cx.notify();
                        },
                    )),
                ),
        )
    }

    /// Below the banner (concept Home): "What's new" - the three newest releases - and "Jump
    pub(super) fn home_card(&self, tag: &str, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let n = t.night;
        let summary = match &self.releases {
            Load::Ready(list) => list
                .iter()
                .find(|r| r.tag == tag)
                // A Nightly's notes name its build, nothing a player reads for.
                .filter(|r| r.nightly.is_none())
                .and_then(|r| backend::notes_summary(&r.body)),
            _ => None,
        };
        let heading = |s: &str| {
            shell::text(night::CAPTION, c(n.muted))
                .relative()
                .font_weight(FontWeight::SEMIBOLD)
                .child(tr(s).to_string())
        };
        let mut card = crate::plate::plate(&n)
            .flex()
            .flex_none()
            .flex_col()
            .gap(px(8.))
            .w(px(320.))
            .p(px(20.));
        if let Some(line) = summary {
            card = card
                .child(heading("What's new"))
                .child(
                    shell::text(night::UI, c(n.ink))
                        .relative()
                        .font_weight(FontWeight::BOLD)
                        .child(
                            horizon_launcher::nightly::label(tag)
                                .unwrap_or_else(|| tag.trim_start_matches('v').to_string()),
                        ),
                )
                .child(
                    shell::text(night::CAPTION, c(n.ink_soft))
                        .relative()
                        .line_clamp(3)
                        .child(line),
                )
                .child(
                    div()
                        .relative()
                        .my(px(8.))
                        .h(px(1.))
                        .bg(alpha(n.stroke, n.rule_alpha)),
                );
        }
        card.child(heading("Join a friend"))
            .child(self.friend_box(self.can_start(tag), cx))
    }

    /// The invite code field and Join.
    fn friend_box(&self, can_play: bool, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let n = t.night;
        let focused = self.friend_focused;
        let field = div()
            .id("friend-code")
            .track_focus(&self.friend_focus)
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .h(px(theme::CONTROL))
            .px(px(12.))
            .overflow_hidden()
            .rounded(px(night::RADIUS_CONTROL))
            .border_1()
            .border_color(if focused {
                c(n.accent_hi)
            } else {
                alpha(n.stroke, 2. * n.stroke_alpha).into()
            })
            .bg(alpha(n.page, 0.6))
            .font_family(theme::MONO)
            .text_size(px(12.))
            .cursor_text()
            .child(line_edit::content(
                &self.friend_code,
                &t,
                focused,
                false,
                false,
                tr("Paste an invite code"),
            ))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                window.focus(&this.friend_focus, cx);
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                match line_edit::handle(&mut this.friend_code, ev, cx) {
                    line_edit::Key::Pass => return,
                    line_edit::Key::Enter => this.join_friend(),
                    line_edit::Key::Escape => window.blur(cx),
                    _ => {}
                }
                cx.stop_propagation();
                cx.notify();
            }));
        let field = line_edit::mouse(field, cx, |this| &mut this.friend_code);
        let valid = horizon_friends::invite::parse(self.friend_code.text()).is_some();
        let mut join =
            action("join-friend", Action::Raised, &t).child(label(tr("Join").to_string()));
        if can_play && valid {
            join = join.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.join_friend();
                cx.notify();
            }));
        } else {
            join = disabled(join);
        }
        div()
            .relative()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(field)
            .child(join)
    }
}

impl Launcher {
    /// How long the download under way has left, from its pace since it was first seen: none
    /// for its first seconds, or without one.
    pub(super) fn download_left(&mut self) -> Option<Duration> {
        if getting_ready_preview() {
            return Some(Duration::from_secs(70));
        }
        let Some((done, total)) = horizon_launcher::mojang::progress() else {
            self.download_seen = None;
            return None;
        };
        let (since, from) = match self.download_seen {
            // A new download (Java's after the game's) starts counting again.
            Some(seen) if seen.1 <= done => seen,
            _ => {
                self.download_seen = Some((Instant::now(), done));
                return None;
            }
        };
        time_left(since.elapsed(), from, done, total)
    }

    /// Under Play while it prepares: a bar, what is happening and the time left, and the steps,
    /// the one under way lit, those done ticked. `version`: the Minecraft it downloads.
    pub(super) fn steps(&self, version: &str, left: Option<Duration>) -> Div {
        let t = self.theme;
        let n = t.night;
        let now = backend::prepare_step();
        let download = download_progress();
        let doing = match (download, now) {
            (Some((done, total)), backend::PrepareStep::Files) => trf(
                "Downloading Minecraft {version} from Mojang · {done} of {total} files",
                &[
                    ("version", version),
                    ("done", &thousands(done as u64)),
                    ("total", &thousands(total as u64)),
                ],
            ),
            (Some((done, total)), _) => trf(
                "Downloading from Mojang · {done} of {total} files",
                &[
                    ("done", &thousands(done as u64)),
                    ("total", &thousands(total as u64)),
                ],
            ),
            (None, _) => self.prepare_log.back().cloned().unwrap_or_default(),
        };
        let left = left.map(|d| match d.as_secs().div_ceil(60) {
            0 | 1 => tr("about a minute left").to_string(),
            m => trf("about {n} min left", &[("n", &m.to_string())]),
        });
        // Play is the last step when it was pressed, not Prepare.
        let play = self.play_when_prepared.is_some();
        let pill = |name: &str, state: std::cmp::Ordering| {
            let (mark, color) = match state {
                std::cmp::Ordering::Less => (
                    Some(blocks::icon("check", 12., c(n.success))),
                    c(n.ink_soft),
                ),
                std::cmp::Ordering::Equal => (None, c(n.ink)),
                std::cmp::Ordering::Greater => (None, c(n.muted)),
            };
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(match mark {
                    Some(icon) => icon.into_any_element(),
                    None => div()
                        .size(px(6.))
                        .bg(if state == std::cmp::Ordering::Equal {
                            c(n.accent_hi)
                        } else {
                            alpha(n.muted, 0.5).into()
                        })
                        .into_any_element(),
                })
                .child(
                    shell::text(night::CAPTION, color)
                        .font_weight(FontWeight::MEDIUM)
                        .child(tr(name).to_string()),
                )
        };
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .max_w(px(620.))
            .child(div().flex().child(progress_bar(
                download.map(|(done, total)| {
                    let f = done as f32 / total as f32;
                    (f, f)
                }),
                &t,
            )))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap(px(16.))
                    .child(
                        shell::text(night::CAPTION, c(n.ink_soft))
                            .min_w_0()
                            .truncate()
                            .child(doing),
                    )
                    .children(
                        left.map(|l| shell::text(night::CAPTION, c(n.muted)).flex_none().child(l)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(16.))
                    .children(
                        backend::PrepareStep::ALL
                            .iter()
                            .map(|s| pill(s.name(), s.cmp(&now))),
                    )
                    .children(play.then(|| pill("Play", std::cmp::Ordering::Greater))),
            )
    }
}

/// `HORIZON_LAUNCHER_PAGE=getting-ready` opens Home as if Play had been pressed on a version that
/// downloads from Mojang, a third of the way through: for screenshots, nothing runs.
pub(super) fn getting_ready_preview() -> bool {
    static PREVIEW: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *PREVIEW
        .get_or_init(|| std::env::var("HORIZON_LAUNCHER_PAGE").as_deref() == Ok("getting-ready"))
}

/// The download under way, files done of how many.
fn download_progress() -> Option<(usize, usize)> {
    horizon_launcher::mojang::progress().or_else(|| getting_ready_preview().then_some((1754, 5160)))
}

/// The time `total - done` files take at the pace of `done - from` in `spent`: none for the
/// first 5 s, or before a file is done.
fn time_left(spent: Duration, from: usize, done: usize, total: usize) -> Option<Duration> {
    (spent >= Duration::from_secs(5) && done > from)
        .then(|| spent.mul_f64(total.saturating_sub(done) as f64 / (done - from) as f64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_left_follows_the_pace_since_first_seen() {
        let s = Duration::from_secs;
        assert_eq!(time_left(s(10), 100, 600, 3100), Some(s(50)));
        assert_eq!(time_left(s(4), 100, 600, 3100), None, "too early");
        assert_eq!(time_left(s(10), 600, 600, 3100), None, "nothing done yet");
        assert_eq!(time_left(s(10), 0, 50, 50), Some(s(0)));
    }

    #[test]
    fn steps_run_in_their_order() {
        use backend::PrepareStep::*;
        assert_eq!(backend::PrepareStep::ALL, [Files, Java, Sounds, Prepare]);
        for s in backend::PrepareStep::ALL {
            assert_eq!(backend::PrepareStep::ALL[s as usize], s);
        }
    }
}
