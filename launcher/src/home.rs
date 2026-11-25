//! Home in the night look (docs/design-system.md, "Night"): the selected instance's name over the
//! sky with what it plays and whether it is ready, Play and the actions beside it, and a card with
//! what's new and joining a friend.
use super::*;
use crate::plate::alpha;
use crate::shell::{action, c, label, Action};
use theme::night;

/// Device pixels per model texel (at scale 1) on the Home stage: the figure is drawn this
/// big, crisp at display resolution. The stage grows with the window: the 9 px it was fixed
/// at in the default window (and any narrow one), up to 14 px on a maximized 27" monitor.
/// Height sets the size, width keeps a narrow window from overflowing. Below 720 px tall the
/// model shrinks proportionally with height, from 9 px/texel (396 pt tall) at 720 down to
/// 6 px/texel (264 pt tall, an integer texel so the skin stays crisp) at the 960x600 minimum
/// window, so Play and the helper line under it both stay above the fold; heights windows
/// already laid out well at (720 and up) never change.
fn stage_texel(viewport_w: f32, viewport_h: f32) -> f32 {
    if viewport_h < 720. {
        // 1/40 slope: exactly 6.0 at 600 pt, approaching 9.0 as h nears 720, where the
        // normal rule yields 9 at every width (its floor clamps the width cap back up),
        // so the curve meets it without a jump at any window width. No width cap here:
        // the value stays <= 9, the minimum the normal rule already allows anywhere.
        return (9. - (720. - viewport_h) / 40.).clamp(6., 9.);
    }
    (viewport_h / 96.).min(viewport_w / 140.).clamp(9., 14.)
}

const NEXT_NEW_INSTANCE: &str = "next-new-instance";

/// Separate the card namespace from the create tile: "new-instance" is a valid instance id.
fn next_instance_control_id(id: &str) -> SharedString {
    format!("next-instance-{id}").into()
}

/// The one-line state under Play. It names the real blocker: a bare "needs preparing"
/// left a first run stuck when preparation really needs an account that owns the game
/// (the resources row below is out of sight). The two states that were a dead end do
/// what that row does, from the line the player actually sees - text and handler only,
/// the line keeps its exact style and place.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HeroStatus {
    /// Prepared and signed (or the developer offline pass): Play starts the game.
    Ready,
    /// Not prepared, owner known: Play itself prepares, then plays.
    NeedsPreparing,
    /// Not prepared, known not to own the game: signing in comes first.
    SignInToPrepare,
    /// Not prepared, ownership unread: the resources row offers prepare from an install.
    PrepareFromRow,
    /// Prepared, the sign-in state not read yet.
    CheckingSignIn,
    /// Prepared, known not to own the game and no developer offline pass.
    SignInToLaunch,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HeroRecovery {
    SignIn,
    Prepare,
}

fn hero_recovery(status: HeroStatus, preparing: bool) -> Option<HeroRecovery> {
    if preparing {
        return None;
    }
    match status {
        HeroStatus::SignInToPrepare | HeroStatus::SignInToLaunch => Some(HeroRecovery::SignIn),
        HeroStatus::PrepareFromRow => Some(HeroRecovery::Prepare),
        _ => None,
    }
}

fn hero_status(prepared: bool, signed: bool, owner: Option<bool>) -> HeroStatus {
    if prepared {
        if signed {
            HeroStatus::Ready
        } else if owner.is_none() {
            HeroStatus::CheckingSignIn
        } else {
            HeroStatus::SignInToLaunch
        }
    } else {
        match owner {
            Some(false) => HeroStatus::SignInToPrepare,
            None => HeroStatus::PrepareFromRow,
            Some(true) => HeroStatus::NeedsPreparing,
        }
    }
}

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
                if tag.is_empty() {
                    "minecraft"
                } else {
                    "horizon"
                },
                |i| i.name.as_str(),
            )
            .to_string();
        let java = instance.as_ref().and_then(|i| i.java.as_ref()).cloned();
        // The subtitle's facts: kind, loader, Minecraft, mods. Real values only - a fact is
        // absent, never a placeholder, while it is unread.
        let mut facts: Vec<String> = Vec::new();
        match &java {
            Some(j) => {
                facts.push("java".to_string());
                if let Some(l) = &j.loader {
                    facts.push(format!("{} {}", l.kind.name(), l.version));
                }
                facts.push(format!("minecraft {}", j.minecraft));
            }
            None if !tag.is_empty() => {
                facts.push(format!(
                    "horizon {}",
                    horizon_launcher::nightly::label(tag)
                        .or_else(|| horizon_launcher::build_import::label(tag))
                        .unwrap_or_else(|| tag.trim_start_matches('v').to_string())
                ));
                facts.push(format!("minecraft {game}"));
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
        // Below 720 pt tall the column tightens the way the stage's floor already does
        // (stage_texel), so the helper line under Play stays inside the 960x600 minimum
        // window instead of ending up below the fold; 720 and up is untouched.
        let compact = self.viewport_h < 720.;
        let mut hero = div()
            .relative()
            .flex()
            .flex_none()
            .flex_col()
            .items_center()
            .gap(px(if compact { 4. } else { 8. }))
            .w_full()
            .child(
                shell::text(night::DISPLAY, c(n.ink))
                    .font_family(theme::PIXEL)
                    .text_size(px(26.))
                    .line_height(px(30.))
                    .child(name.clone()),
            )
            .child(self.home_stage(cx))
            .child(self.play_action(ready, &facts.join("  ·  "), cx))
            .child({
                let status = hero_status(prepared, signed, self.owner);
                let line = || {
                    shell::text(
                        night::CAPTION,
                        if prepared && signed { t.success } else { t.warn },
                    )
                    .font_family(theme::PIXEL)
                    // Supporting text scales; the player stage and Launch geometry stay pinned.
                    .text_size(px(theme::chrome(12.)))
                    .line_height(px(theme::chrome(17.)))
                    .child(
                        tr(match status {
                            HeroStatus::Ready => "ready to launch",
                            HeroStatus::NeedsPreparing | HeroStatus::PrepareFromRow => {
                                "needs preparing"
                            }
                            HeroStatus::SignInToPrepare => "sign in to prepare",
                            HeroStatus::CheckingSignIn => "checking sign-in…",
                            HeroStatus::SignInToLaunch => "sign in to launch",
                        })
                        .to_string(),
                    )
                };
                // While preparation runs its busy Play owns the moment: no second start.
                match hero_recovery(status, self.preparing) {
                    Some(HeroRecovery::SignIn) => keyboard(
                        div()
                            .id("hero-status")
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.8))
                            .child(line()),
                        &t,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.sign_in_to_play();
                        cx.notify();
                    }))
                    .into_any_element(),
                    Some(HeroRecovery::Prepare) => keyboard(
                        div()
                            .id("hero-status")
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.8))
                            .child(line()),
                        &t,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.prepare(backend::MinecraftFrom::Install);
                        cx.notify();
                    }))
                    .into_any_element(),
                    _ => line().into_any_element(),
                }
            })
            .children(self.playing.map(|playing| {
                shell::text(night::CAPTION, c(n.muted))
                    .relative()
                    .text_size(px(theme::chrome(12.)))
                    .line_height(px(theme::chrome(17.)))
                    .child(trf("{n} playing horizon now", &[("n", &thousands(playing))]))
            }))
            // Words sit on a plate where the sky would cross them (docs/design-system.md).
            .child(
                shell::text(night::CAPTION, c(n.muted))
                    .bg(alpha(n.card, 0.6))
                    .text_size(px(theme::chrome(12.)))
                    .line_height(px(theme::chrome(17.)))
                    .px(px(10.))
                    .py(px(if compact { 2. } else { 4. }))
                    .max_w(px(theme::chrome(560.)))
                    .child(
                        tr("its own runtime, mods and saves. nothing mixes with your other instances.")
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
                .gap(px(theme::chrome(8.)))
                .child(div().size(px(theme::chrome(6.))).bg(c(n.accent_hi)))
                .child(trf(
                    "{version} available",
                    &[("version", update_name(release))],
                ));
            hero = hero.child(match installing {
                Some((_, p)) => line
                    .child(
                        div()
                            .w(px(theme::chrome(120.)))
                            .flex()
                            .child(progress_bar(Some(self.bar), &t).h(px(theme::chrome(6.)))),
                    )
                    .child(format!("{:.0}%", p * 100.)),
                None => line.child(
                    keyboard(
                        div()
                            .id("install-update")
                            .text_color(c(n.accent_hi))
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.8))
                            .child(tr("install")),
                        &t,
                    )
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
        // The notice describes the ping, which this build sends by default; hidden only
        // when `HORIZON_TELEMETRY_URL=off` turns it off.
        if !self.settings.ping_notice_seen
            && horizon_telemetry::live_endpoint().is_some()
            && !self.collection.consent.needs_asking()
            && !self.asking_owner()
        {
            // The short first layer carries what the ping is and that it's on; the full
            // disclosure sits behind details. A press that travels is a scroll drag, not a
            // click, so neither button answers it.
            let ping_detail = self.ping_notice_detail;
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
                            .child(tr(telemetry::PING_HOME_SENT)),
                    )
                    .child(
                        shell::text(night::CAPTION, c(n.ink))
                            .relative()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr(telemetry::PING_HOME_STOP)),
                    )
                    .child(
                        shell::text(night::CAPTION, c(n.muted))
                            .relative()
                            .child(tr(telemetry::PING_HOME_DETAILS)),
                    )
                    .when(ping_detail, |d| {
                        d.children(telemetry::bullets(telemetry::PING).into_iter().map(|line| {
                            shell::text(night::CAPTION, c(n.muted))
                                .relative()
                                .child(line)
                                .into_any_element()
                        }))
                    })
                    .child(
                        div()
                            .flex()
                            .gap(px(16.))
                            .child(
                                shell::text(night::UI, c(n.accent_hi))
                                    .relative()
                                    .id("ping-notice-detail")
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.8))
                                    .child(tr(if ping_detail {
                                        "hide details"
                                    } else {
                                        "details"
                                    }))
                                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                                        if drag_not_click(event) {
                                            return;
                                        }
                                        this.ping_notice_detail = !this.ping_notice_detail;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                shell::text(night::UI, c(n.accent_hi))
                                    .relative()
                                    .id("ping-notice-ok")
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.8))
                                    .child(tr("dismiss"))
                                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                                        if drag_not_click(event) {
                                            return;
                                        }
                                        this.settings.ping_notice_seen = true;
                                        let _ = this.settings.save();
                                        cx.notify();
                                    })),
                            ),
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
        let account = keyboard(
            div()
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
                ),
            &t,
        )
        .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
            if this.closed_by_this_press(event) {
                return;
            }
            this.account_menu = !this.account_menu;
            if this.account_menu {
                this.account_menu_top = true;
                // Who can play may have changed elsewhere since the window opened.
                this.accounts = Accounts::load();
            }
            cx.notify();
        }));
        let account = div()
            .relative()
            .child(account)
            .children(self.account_menu(true, cx));
        // Safe area under the window chrome: the bar owns the full TOPBAR band and centers its
        // buttons, so neither the New instance button nor the account pill can touch the title
        // bar's bottom border (Luke, 2026-11-17).
        div()
            .flex()
            .flex_none()
            .h(px(theme::TOPBAR))
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
                            .line_height(px(25.))
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
                    .children(self.native_log.available.then(|| {
                        shell::action("topbar-build-log", shell::Action::Quiet, &t)
                            .h(px(theme::CONTROL))
                            .child(shell::label("build log".to_string()))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.native_log_open();
                                cx.notify();
                            }))
                    }))
                    .child(
                        shell::action("topbar-new-instance", shell::Action::Raised, &t)
                            .h(px(theme::CONTROL))
                            .child(div().relative().child(blocks::icon("plus", 16., c(n.ink))))
                            .child(shell::label(tr("new instance").to_string()))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.open_instance_dialog(None, cx);
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
        let mut row = div().flex().flex_wrap().gap(px(theme::chrome(12.)));
        for instance in self.instances.list.clone() {
            let active = selected_id.as_ref() == Some(&instance.id);
            let played = instances_page::played_line(&instance);
            // A Java instance never plays a Horizon nightly: say its Minecraft and loader,
            // never the launcher's version.
            let sub = match &instance.java {
                Some(j) => match &j.loader {
                    Some(l) => format!(
                        "minecraft {} · {} {}",
                        j.minecraft,
                        l.kind.name(),
                        l.version
                    ),
                    None => format!("minecraft {}", j.minecraft),
                },
                None => self
                    .version_of(&instance)
                    .map(|tag| {
                        horizon_launcher::nightly::label(tag)
                            .or_else(|| horizon_launcher::build_import::label(tag))
                            .unwrap_or_else(|| tag.trim_start_matches('v').to_string())
                    })
                    .unwrap_or_default(),
            };
            let clicked = instance.clone();
            row = row.child(
                keyboard(
                    div()
                        .id(next_instance_control_id(&instance.id))
                        .relative()
                        .flex()
                        .flex_none()
                        .flex_col()
                        .justify_center()
                        .gap(px(theme::chrome(4.)))
                        .w(px(theme::chrome(220.)))
                        .h(px(theme::chrome(84.)))
                        .px(px(theme::chrome(14.)))
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
                                .text_size(px(theme::chrome(11.)))
                                .line_height(px(theme::chrome(17.)))
                                .w_full()
                                .truncate()
                                .child(sub),
                        )
                        .child(
                            shell::text(night::CAPTION, c(n.muted))
                                .text_size(px(theme::chrome(11.)))
                                .line_height(px(theme::chrome(17.)))
                                .w_full()
                                .truncate()
                                .child(played),
                        ),
                    &t,
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.select_instance(&clicked);
                    cx.notify();
                })),
            );
        }
        row = row.child(
            keyboard(
                div()
                    .id(NEXT_NEW_INSTANCE)
                    .relative()
                    .flex()
                    .flex_none()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(theme::chrome(6.)))
                    .w(px(theme::chrome(220.)))
                    .h(px(theme::chrome(84.)))
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
                            .text_size(px(theme::chrome(11.)))
                            .line_height(px(theme::chrome(17.)))
                            .child(tr("new instance").to_string()),
                    ),
                &t,
            )
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.open_instance_dialog(None, cx);
                cx.notify();
            })),
        );
        div()
            .flex()
            .flex_none()
            .flex_col()
            .gap(px(theme::chrome(10.)))
            .child(
                shell::text(night::CAPTION, c(n.muted))
                    .font_family(theme::PIXEL)
                    .text_size(px(theme::chrome(11.)))
                    .line_height(px(theme::chrome(17.)))
                    .child(tr("playing next").to_string()),
            )
            .child(row)
    }

    /// The home stage's center (concept Home): the player model, 3D-projected from its skin,
    /// under its pixel nametag when a player name is known. Account textures load off-thread; the labelled fixture is
    /// kept only while no supported account texture is available.
    pub(super) fn home_stage(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let n = t.night;
        let scale = self.scale;
        let profile = match self.accounts.player() {
            account::Player::Microsoft(_) => self.accounts.selected().cloned(),
            _ => None,
        };
        let texture = profile.and_then(|a| {
            let (url, model) = a.skin?;
            let url = horizon_launcher::skins::texture_url(&url)?;
            let file = self.mods_image(&url, 64)?;
            let slim = match model.as_str() {
                "classic" => false,
                "slim" => true,
                _ => return None,
            };
            Some((file, slim))
        });
        // The stage is alive: the clock from drain drives idle sway, a wave every few
        // seconds, and a look around; dragging turns the model. Rendered fresh each frame.
        // A click's wave runs on its own clock over the stage's cycle, then hands back.
        let pose = match self.stage_click_wave {
            Some(at) => match player_model::Pose::click_wave(self.stage_t, self.stage_t - at) {
                Some(p) => p,
                None => {
                    self.stage_click_wave = None;
                    player_model::Pose::stage(self.stage_t)
                }
            },
            None => player_model::Pose::stage(self.stage_t),
        };
        // Reduce motion keeps the base sway but drops the occasional foot tap and arm lift.
        let pose = if crate::reduce_motion() {
            pose.without_extras(self.stage_t)
        } else {
            pose
        };
        let (yaw, pitch) = (self.stage_yaw, self.stage_pitch);
        // Rasterize at real display resolution (texel stage_texel in device pixels): geometry edges
        // are sharp at any window scale while the skin texture stays nearest-blocky.
        let texel = stage_texel(self.viewport_w, self.viewport_h);
        let dev_w = (player_model::W as f32 * texel * scale).round().max(1.) as usize;
        let dev_h = (player_model::H as f32 * texel * scale).round().max(1.) as usize;
        let skin = texture.and_then(|(path, slim)| {
            let stamp = crate::servers_page::file_stamp(&path);
            let png = self.stage_skin.get(&path, stamp, || {
                let Some(bytes) = horizon_launcher::browser::images::read_capped(&path, 1 << 20)
                else {
                    return SkinRead::Retry;
                };
                SkinRead::Done(
                    horizon_launcher::skins::read_skin(&bytes)
                        .ok()
                        .filter(|png| png.dimensions() == (64, 64))
                        .map(std::sync::Arc::new),
                )
            })?;
            Some((png, slim))
        });
        let real = skin.is_some();
        // The same skin, pose, angle and size draw the same picture, so a render between two
        // stage ticks (a hover elsewhere repaints the window) reuses the last baked image
        // instead of rasterizing again; a new image is made only when the key differs.
        let key = StagePicture::new(skin.as_ref(), yaw, pitch, &pose, (dev_w, dev_h));
        let (baked, fresh) = self.stage_frame.get(key, || {
            let pixels = match &skin {
                Some((png, slim)) => Some(player_model::render_posed_hi(
                    png, yaw, pitch, &pose, *slim, dev_w, dev_h,
                )),
                None => player_model::fixture().map(|p| {
                    player_model::render_posed_hi(&p, yaw, pitch, &pose, false, dev_w, dev_h)
                }),
            };
            pixels.map(|p| art::bake(&p, dev_w, dev_h, 1., 1.))
        });
        if let (true, Some(i)) = (fresh, &baked) {
            // Every new image replaces the last; the one it replaces leaves the atlas next render.
            if let Some(old) = self.stage_image.replace(i.clone()) {
                self.stage_replaced.push(old);
            }
        }
        let (who, _) = self.who_plays();
        // Same short-window tightening as the hero column: drop the stage's own padding.
        let compact = self.viewport_h < 720.;
        let mut stage = div()
            .relative()
            .flex()
            .flex_none()
            .flex_col()
            .items_center()
            .gap(px(if compact { 2. } else { 4. }))
            .pt(px(if compact { 0. } else { 8. }));
        let named_player = !matches!(
            self.accounts.player(),
            horizon_launcher::account::Player::GameName if self.offline_name.is_none()
        );
        if named_player {
            stage = stage.child(
                shell::text(night::CAPTION, c(n.ink))
                    .font_family(theme::PIXEL)
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .child(who.to_lowercase()),
            );
        }
        if let Some(i) = baked {
            // The model is a keyboard stop: Enter or Space waves, Left and Right turn it.
            let model = div()
                .id("stage-model")
                .w(px(player_model::W as f32 * texel))
                .h(px(player_model::H as f32 * texel))
                .cursor(gpui::CursorStyle::PointingHand)
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                    let m = &ev.keystroke.modifiers;
                    let chord = m.control || m.alt || m.platform || m.function;
                    match stage_key(ev.keystroke.key.as_str(), ev.is_held, chord) {
                        Some(StageKey::Wave) => this.stage_click_wave = Some(this.stage_t),
                        Some(StageKey::Turn(by)) => {
                            this.stage_touched = std::time::Instant::now();
                            this.stage_yaw += by;
                        }
                        None => return,
                    }
                    cx.stop_propagation();
                    cx.notify();
                }));
            stage = stage.child(
                keyboard(model, &self.theme)
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                            this.stage_drag
                                .begin((event.position.x.into(), event.position.y.into()));
                            cx.notify();
                        }),
                    )
                    .child(gpui::img(i).w_full().h_full()),
            );
        }
        if real {
            stage
        } else {
            stage.child(
                shell::text(night::CAPTION, c(n.muted))
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .child(tr("preview player").to_string()),
            )
        }
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
                    tr("getting ready…"),
                    done as f32 * 100. / total as f32
                ),
                None => tr("getting ready…").to_string(),
            };
            return div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    action("play", Action::Main, &t)
                        .h(px(night::PLAY))
                        .text_size(px(20.))
                        .min_w(px(260.))
                        .cursor_default()
                        .child(label(busy)),
                )
                .children(download.map(|_| {
                    action("download-cancel", Action::Quiet, &t)
                        .h(px(theme::CONTROL))
                        .child(label(tr("cancel").to_string()))
                        .on_click(|_, _, _| {
                            horizon_launcher::mojang::CANCEL.store(true, Ordering::Relaxed)
                        })
                }));
        }
        // Nothing was ever picked and the Rust game was never used here: the hero's Play
        // would choose a game for the player. Ask for the first pick instead. An explicit
        // card Play, Join or quick-play stays untouched: those are the pick.
        if instances::needs_first_pick_with(
            &self.instances.list,
            self.settings.instance.as_deref(),
            backend::any_prepared,
        ) {
            return div().flex().child(
                action("play", Action::Main, &t)
                    .h(px(night::PLAY))
                    .text_size(px(20.))
                    .min_w(px(320.))
                    .child(label(tr("new instance").to_string()))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.open_instance_dialog(None, cx);
                        cx.notify();
                    })),
            );
        }
        let press = play_press(ready, self.owner, self.preparing || running);
        let live = !matches!(press, PlayPress::Nothing);
        // The concept's Play wears the instance's facts as a small second line.
        let mut play = action("play", Action::Main, &t)
            .h(px(night::PLAY))
            .text_size(px(20.))
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
                            .child(div().relative().child(blocks::icon(
                                "play",
                                20.,
                                c(t.night.on_accent),
                            )))
                            .child(label(
                                tr(if running { "running" } else { "launch" }).to_string(),
                            )),
                    )
                    .when(!subtitle.is_empty(), |d| {
                        d.child(
                            shell::text(theme::MICRO, c(t.night.on_accent))
                                .text_size(px(theme::chrome(12.)))
                                .line_height(px(theme::chrome(18.)))
                                .child(subtitle.to_lowercase()),
                        )
                    }),
            );
        match press {
            PlayPress::Nothing => play = play.opacity(0.78).cursor_default().tab_stop(false),
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
            .child(keyboard(
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
                        .child(div().relative().child(blocks::icon(
                            "chevron-down",
                            20.,
                            c(t.night.on_accent),
                        )))
                        .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                            if !this.closed_by_this_press(event) {
                                this.picker_open = !this.picker_open;
                            }
                            cx.notify();
                        })),
                    SharedString::from("play-versions"),
                ),
                &t,
            ));
        if let Some(menu) = self.picker_menu(cx) {
            split = split.child(menu);
        }
        div().flex().relative().items_center().child(split)
    }

    /// What else the instance offers, under the hero card while it is not preparing: its
    /// version (Horizon), its mods, its folder.
    // Wired up with the social row it belongs to.
    #[allow(dead_code)]
    pub(super) fn home_actions(&self, _java: bool, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let folder = self.selected_instance().map(|i| i.home.clone());
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(10.))
            .child(
                action("home-mods", Action::Quiet, &t)
                    .h(px(theme::CONTROL))
                    .child(label(tr("mods").to_string()))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.go(Page::Mods);
                        cx.notify();
                    })),
            )
            .children(folder.map(|home| {
                action("home-folder", Action::Quiet, &t)
                    .h(px(theme::CONTROL))
                    .child(label(tr("folder").to_string()))
                    .on_click(move |_, _, cx| {
                        let _ = std::fs::create_dir_all(&home);
                        cx.open_with_system(&home);
                    })
            }))
    }

    /// The concept Home's "you" card, while nobody plays as a Microsoft account: a preview
    /// stands in until sign-in. A signed-in player already has the sidebar's account card.
    // Wired up with the social row it belongs to.
    #[allow(dead_code)]
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
                        .child(tr("you").to_string()),
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
                        tr("a stand-in plays for now. sign in with your microsoft account to play as yourself.")
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
    // Wired up with the social row it belongs to.
    #[allow(dead_code)]
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
            .w(px(360.))
            .p(px(20.));
        if let Some(line) = summary {
            card = card
                .child(heading("what's new"))
                .child(
                    shell::text(night::UI, c(n.ink))
                        .relative()
                        .font_weight(FontWeight::BOLD)
                        .child(
                            horizon_launcher::nightly::label(tag)
                                .or_else(|| horizon_launcher::build_import::label(tag))
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
        card.child(heading("join a friend"))
            .child(self.friend_box(self.can_start(tag), cx))
    }

    /// The invite code field and Join.
    // Wired up with the social row it belongs to.
    #[allow(dead_code)]
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
                tr("paste an invite code"),
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
        let mut join = action("join-friend", Action::Raised, &t)
            .h(px(theme::CONTROL))
            .child(label(tr("join").to_string()));
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
                "downloading minecraft {version} from mojang · {done} of {total} files",
                &[
                    ("version", version),
                    ("done", &thousands(done as u64)),
                    ("total", &thousands(total as u64)),
                ],
            ),
            (Some((done, total)), _) => trf(
                "downloading from mojang · {done} of {total} files",
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
                    .children(play.then(|| pill("play", std::cmp::Ordering::Greater))),
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

/// Everything that decides the stage picture: the decoded skin (None for the fixture), its
/// model, the turn angles, the pose limbs and the size, each compared exactly (floats by bits,
/// the skin by identity of the one decoded image the cache holds).
pub(crate) struct StagePicture {
    skin: Option<std::sync::Arc<image::RgbaImage>>,
    slim: bool,
    angles: [u32; 2],
    pose: [u32; 5],
    size: (usize, usize),
}

impl StagePicture {
    fn new(
        skin: Option<&(std::sync::Arc<image::RgbaImage>, bool)>,
        yaw: f32,
        pitch: f32,
        pose: &player_model::Pose,
        size: (usize, usize),
    ) -> Self {
        Self {
            skin: skin.map(|(png, _)| png.clone()),
            slim: skin.is_some_and(|(_, slim)| *slim),
            angles: [yaw.to_bits(), pitch.to_bits()],
            pose: [
                pose.arm_r.to_bits(),
                pose.arm_l.to_bits(),
                pose.leg_r.to_bits(),
                pose.leg_l.to_bits(),
                pose.head_yaw.to_bits(),
            ],
            size,
        }
    }

    fn same(&self, other: &Self) -> bool {
        let skin = match (&self.skin, &other.skin) {
            (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        skin && self.slim == other.slim
            && self.angles == other.angles
            && self.pose == other.pose
            && self.size == other.size
    }
}

/// The last stage picture and the key it was made for.
pub(crate) struct StageFrame<T> {
    last: Option<(StagePicture, Option<T>)>,
}

impl<T> Default for StageFrame<T> {
    fn default() -> Self {
        Self { last: None }
    }
}

impl<T: Clone> StageFrame<T> {
    /// The picture for `key`: the kept one when the key is exactly the last (second value
    /// false), else `make()`d and kept (true).
    pub(crate) fn get(
        &mut self,
        key: StagePicture,
        make: impl FnOnce() -> Option<T>,
    ) -> (Option<T>, bool) {
        if let Some((last, picture)) = &self.last {
            if last.same(&key) {
                return (picture.clone(), false);
            }
        }
        let picture = make();
        self.last = Some((key, picture.clone()));
        (picture, true)
    }
}

/// What reading the stage's skin file gave. Only a file that was read decides the skin (`Done`,
/// None when it is not a usable 64 by 64 skin); a failed read is `Retry` and never remembered.
pub(crate) enum SkinRead {
    Done(Option<std::sync::Arc<image::RgbaImage>>),
    Retry,
}

/// The stage's decoded skin: one file (the stage shows one player), kept while its stamp is
/// unchanged. The stage repaints every 33 ms and used to read and decode the PNG each time.
#[derive(Default)]
pub(crate) struct StageSkin {
    entry: Option<(
        std::path::PathBuf,
        crate::servers_page::FileStamp,
        Option<std::sync::Arc<image::RgbaImage>>,
    )>,
}

impl StageSkin {
    /// The decoded skin at `path`, loading it only when the stamp (or path) differs from the
    /// kept one. No stamp (file not readable) or a `Retry` read gives no skin and keeps nothing,
    /// so the next frame tries again.
    pub(crate) fn get(
        &mut self,
        path: &std::path::Path,
        stamp: Option<crate::servers_page::FileStamp>,
        load: impl FnOnce() -> SkinRead,
    ) -> Option<std::sync::Arc<image::RgbaImage>> {
        let Some(stamp) = stamp else {
            self.entry = None;
            return None;
        };
        if let Some((p, s, png)) = &self.entry {
            if p == path && *s == stamp {
                return png.clone();
            }
        }
        match load() {
            SkinRead::Retry => {
                self.entry = None;
                None
            }
            SkinRead::Done(png) => {
                self.entry = Some((path.to_path_buf(), stamp, png.clone()));
                png
            }
        }
    }
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

    #[test]
    fn hero_status_names_the_real_blocker() {
        use HeroStatus::*;
        // Prepared: ownership decides between playing, waiting and signing in.
        assert_eq!(hero_status(true, true, Some(true)), Ready);
        assert_eq!(
            hero_status(true, true, Some(false)),
            Ready,
            "developer offline pass"
        );
        assert_eq!(hero_status(true, false, None), CheckingSignIn);
        assert_eq!(hero_status(true, false, Some(false)), SignInToLaunch);
        // Not prepared: the blocker is preparation itself, or the account it needs.
        assert_eq!(hero_status(false, false, Some(true)), NeedsPreparing);
        assert_eq!(hero_status(false, false, Some(false)), SignInToPrepare);
        assert_eq!(
            hero_status(false, true, Some(false)),
            SignInToPrepare,
            "the developer offline pass never exempts preparation"
        );
        assert_eq!(hero_status(false, false, None), PrepareFromRow);
        assert_eq!(hero_status(false, true, None), PrepareFromRow);
    }

    #[test]
    fn both_sign_in_blockers_offer_the_same_recovery_and_busy_offers_none() {
        for status in [HeroStatus::SignInToPrepare, HeroStatus::SignInToLaunch] {
            assert_eq!(hero_recovery(status, false), Some(HeroRecovery::SignIn));
            assert_eq!(hero_recovery(status, true), None);
        }
        assert_eq!(
            hero_recovery(HeroStatus::PrepareFromRow, false),
            Some(HeroRecovery::Prepare)
        );
        assert_eq!(hero_recovery(HeroStatus::PrepareFromRow, true), None);
        for status in [
            HeroStatus::Ready,
            HeroStatus::NeedsPreparing,
            HeroStatus::CheckingSignIn,
        ] {
            assert_eq!(hero_recovery(status, false), None);
        }
    }

    #[test]
    fn new_instance_named_instance_and_create_tile_get_distinct_focus_slots() {
        let mut slots = IdSlots::new(3);
        slots.begin(|_| false);
        for id in ["horizon", "new-instance"] {
            assert!(slots
                .slot_for(next_instance_control_id(id).into())
                .is_some());
        }
        assert!(slots.slot_for(NEXT_NEW_INSTANCE.into()).is_some());
    }

    #[test]
    fn playing_next_select_and_create_tiles_keep_the_keyboard_control_path() {
        let source = include_str!("home.rs");
        let body = source
            .split("pub(super) fn playing_next(")
            .nth(1)
            .unwrap()
            .split("/// The home stage")
            .next()
            .unwrap();
        assert_eq!(body.matches("keyboard(").count(), 2);
        assert!(body.contains("this.select_instance(&clicked)"));
        assert!(body.contains("this.open_instance_dialog(None, cx)"));
    }

    #[test]
    fn the_stage_skin_is_decoded_once_and_a_failed_read_is_retried() {
        use std::path::Path;
        use std::sync::Arc;
        let mut cache = StageSkin::default();
        let a = Path::new("/skins/a.png");
        let png = Arc::new(image::RgbaImage::new(64, 64));
        let loads = std::cell::Cell::new(0);
        let done = |png: &Arc<image::RgbaImage>| {
            let png = png.clone();
            let loads = &loads;
            move || {
                loads.set(loads.get() + 1);
                SkinRead::Done(Some(png))
            }
        };
        // A failed read is tried every frame and leaves nothing behind.
        for n in 1..=3 {
            let got = cache.get(a, Some((1, 9)), || {
                loads.set(loads.get() + 1);
                SkinRead::Retry
            });
            assert!(got.is_none());
            assert_eq!(loads.get(), n);
            assert!(cache.entry.is_none());
        }
        // Once read, 1000 frames decode it once and hand back the same image.
        for _ in 0..1000 {
            let got = cache.get(a, Some((1, 9)), done(&png)).unwrap();
            assert!(Arc::ptr_eq(&got, &png));
        }
        assert_eq!(loads.get(), 4);
        // A new stamp, or another path, loads again; no stamp drops it.
        cache.get(a, Some((2, 9)), done(&png));
        assert_eq!(loads.get(), 5);
        cache.get(Path::new("/skins/b.png"), Some((2, 9)), done(&png));
        assert_eq!(loads.get(), 6);
        assert!(cache.get(a, None, done(&png)).is_none());
        assert_eq!(loads.get(), 6);
        assert!(cache.entry.is_none());
        // A file that read but is not a usable skin is remembered as such.
        for _ in 0..10 {
            let got = cache.get(a, Some((3, 9)), || {
                loads.set(loads.get() + 1);
                SkinRead::Done(None)
            });
            assert!(got.is_none());
        }
        assert_eq!(loads.get(), 7);
    }

    #[test]
    fn the_stage_picture_is_made_again_only_when_something_that_draws_it_changes() {
        use std::sync::Arc;
        let png = Arc::new(image::RgbaImage::new(64, 64));
        let other = Arc::new(image::RgbaImage::new(64, 64));
        let pose = player_model::Pose::stage(1.0);
        let size = (320, 352);
        let made = std::cell::Cell::new(0u32);
        let mut frame = StageFrame::<u32>::default();
        let draw = |frame: &mut StageFrame<u32>, key: StagePicture| {
            frame.get(key, || {
                made.set(made.get() + 1);
                Some(made.get())
            })
        };
        let key = |png: Option<&Arc<image::RgbaImage>>,
                   slim,
                   yaw: f32,
                   pose: &player_model::Pose,
                   size| {
            StagePicture::new(
                png.map(|p| (p.clone(), slim)).as_ref(),
                yaw,
                0.1,
                pose,
                size,
            )
        };
        // 500 renders of an unchanged stage make one picture, and only the first is fresh.
        let (first, fresh) = draw(&mut frame, key(Some(&png), false, 0.4, &pose, size));
        assert_eq!((first, fresh), (Some(1), true));
        for _ in 0..500 {
            let (got, fresh) = draw(&mut frame, key(Some(&png), false, 0.4, &pose, size));
            assert_eq!((got, fresh), (Some(1), false));
        }
        assert_eq!(made.get(), 1);
        // Each input that draws the picture, changed alone, makes a new one.
        let mut later = pose;
        later.arm_r += f32::EPSILON;
        let changes: Vec<StagePicture> = vec![
            key(Some(&other), false, 0.4, &pose, size),
            key(Some(&png), true, 0.4, &pose, size),
            key(Some(&png), false, 0.5, &pose, size),
            key(Some(&png), false, 0.4, &later, size),
            key(Some(&png), false, 0.4, &pose, (321, 352)),
            key(None, false, 0.4, &pose, size),
        ];
        for (n, k) in changes.into_iter().enumerate() {
            // Back to the base picture, then one input differs.
            draw(&mut frame, key(Some(&png), false, 0.4, &pose, size));
            let before = made.get();
            let (_, fresh) = draw(&mut frame, k);
            assert!(fresh, "change {n} must make a new picture");
            assert_eq!(made.get(), before + 1, "change {n}");
        }
    }

    /// The key compares floats by their bits and every input alone, so each pose limb, the
    /// pitch, the height and a float that differs only in its bits each make a new picture,
    /// while the very same bits (a NaN included) are a hit and never a picture per frame.
    #[test]
    fn the_stage_picture_key_sees_every_limb_the_pitch_the_height_and_float_bits() {
        use std::sync::Arc;
        let png = Arc::new(image::RgbaImage::new(64, 64));
        let base_pose = player_model::Pose::stage(1.0);
        let key = |yaw: f32, pitch: f32, pose: &player_model::Pose, size: (usize, usize)| {
            StagePicture::new(Some(&(png.clone(), false)), yaw, pitch, pose, size)
        };
        let base = || key(0.4, 0.1, &base_pose, (320, 352));
        let mut differs: Vec<(&str, StagePicture)> = vec![
            (
                "pitch",
                key(0.4, 0.1 + f32::EPSILON, &base_pose, (320, 352)),
            ),
            ("height", key(0.4, 0.1, &base_pose, (320, 353))),
            ("width", key(0.4, 0.1, &base_pose, (319, 352))),
        ];
        for (n, name) in ["arm_r", "arm_l", "leg_r", "leg_l", "head_yaw"]
            .into_iter()
            .enumerate()
        {
            let mut moved = base_pose;
            let limb = match n {
                0 => &mut moved.arm_r,
                1 => &mut moved.arm_l,
                2 => &mut moved.leg_r,
                3 => &mut moved.leg_l,
                _ => &mut moved.head_yaw,
            };
            *limb += 0.001;
            differs.push((name, key(0.4, 0.1, &moved, (320, 352))));
        }
        for (name, k) in &differs {
            assert!(!base().same(k), "{name} alone must make a new picture");
            assert!(!k.same(&base()), "{name}: the comparison is symmetric");
        }
        // Equal values with different bits are different keys: 0.0 and -0.0.
        let zero = player_model::Pose {
            arm_r: 0.0,
            ..base_pose
        };
        let negative_zero = player_model::Pose {
            arm_r: -0.0,
            ..base_pose
        };
        assert!(!key(0.4, 0.1, &zero, (1, 1)).same(&key(0.4, 0.1, &negative_zero, (1, 1))));
        // The same NaN bits are the same key, so a NaN never remakes the picture each frame.
        let nan = player_model::Pose {
            arm_r: f32::NAN,
            ..base_pose
        };
        assert!(key(f32::NAN, f32::NAN, &nan, (1, 1)).same(&key(f32::NAN, f32::NAN, &nan, (1, 1))));
        // A NaN with other bits is another key.
        let other_nan = f32::from_bits(f32::NAN.to_bits() ^ 1);
        assert!(other_nan.is_nan());
        assert!(!key(f32::NAN, 0.1, &base_pose, (1, 1)).same(&key(
            other_nan,
            0.1,
            &base_pose,
            (1, 1)
        )));
        // Each key matches itself.
        for (name, k) in &differs {
            assert!(k.same(k), "{name} matches itself");
        }
    }
}
