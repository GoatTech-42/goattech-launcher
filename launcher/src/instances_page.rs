//! Instances in the window (`horizon_launcher::instances`): the sidebar's list, by group, and the
//! dialog that makes one (empty, or a copy of the selected one) or changes its name, group and
//! icon.
use super::*;
use gpui::RenderImage;
use horizon_launcher::instances::{self, Icon, Instance, Kind as InstanceKind};
use horizon_launcher::{other_launchers, sources};
use servers_page::Field;
use std::collections::HashMap;
use std::sync::Arc;

/// Textures offered as icons, when the player's prepared files hold them: block faces and items
/// that read well at sidebar size.
const TEXTURES: [&str; 24] = [
    "grass_block_side",
    "oak_planks",
    "oak_log",
    "cobblestone",
    "bricks",
    "bookshelf",
    "crafting_table_front",
    "tnt_side",
    "sand",
    "snow",
    "ice",
    "obsidian",
    "netherrack",
    "end_stone",
    "glowstone",
    "diamond_ore",
    "diamond",
    "emerald",
    "apple",
    "ender_pearl",
    "iron_pickaxe",
    "diamond_sword",
    "bow",
    "book",
];

/// Colours offered as icons: Horizon's accent and a few that sit beside it in both themes.
const COLOURS: [&str; 8] = [
    "#5b3fd6", "#3f7fd6", "#2f9e8f", "#4f9e2f", "#c9a227", "#d9732f", "#c4405a", "#7d7591",
];

/// The dialog: a new instance (`editing` None) or one being changed. Its name and group are the
/// text fields `Field::InstanceName` and `Field::InstanceGroup`.
pub(super) struct InstanceDialog {
    editing: Option<String>,
    /// A new instance copies the selected one's worlds, options and packs.
    copy: bool,
    icon: Option<Icon>,
    /// A picture chosen since it opened (None in it: the instance's is removed on Save).
    pub(super) picture: Option<Option<instance_icons::Chosen>>,
    /// A banner chosen since it opened; same shape as `picture`.
    pub(super) banner: Option<Option<instance_icons::Chosen>>,
    pub(super) error: Option<String>,
    busy: bool,
    /// Horizon, or the Java game it plays.
    pub(super) java: new_instance::JavaChoice,
}

/// The instances and their icons as images, kept between frames.
#[derive(Default)]
pub(super) struct Instances {
    pub(super) list: Vec<Instance>,
    pub(super) dialog: Option<InstanceDialog>,
    /// The instance whose Delete is waiting for its confirmation, and why the last one failed.
    confirm_delete: Option<String>,
    delete_error: Option<String>,
    /// Textures, pictures and marks as images, by name and size in device pixels.
    pub(super) icons: HashMap<(String, u32, bool), Option<Arc<RenderImage>>>,
    /// Mojang's versions, for a new Java instance.
    pub(super) versions: new_instance::VersionList,
    /// Other launchers' instances from the last Find here: each with why its mods can't come
    /// along when they can't (`other_launchers::mods_blocked`). None until Find runs.
    others: Option<Vec<(other_launchers::Found, Option<String>)>>,
    /// A scan or import for that section is running.
    others_busy: bool,
    /// Why the last import from another launcher failed.
    others_error: Option<String>,
}

impl Launcher {
    pub(super) fn reload_instances(&mut self) {
        self.instances.list = instances::list();
        // A picture or banner may have come or gone with the change.
        self.instances
            .icons
            .retain(|(key, _, _), _| {
                !key.starts_with("picture:") && !key.starts_with("banner:")
            });
    }

    /// The instance Play starts.
    pub(super) fn selected_instance(&self) -> Option<&Instance> {
        let id = self
            .settings
            .instance
            .as_deref()
            .unwrap_or(instances::DEFAULT);
        let list = &self.instances.list;
        list.iter().find(|i| i.id == id).or(list.first())
    }

    /// The installed version `instance` plays: its own, else the launcher's.
    pub(super) fn version_of(&self, instance: &Instance) -> Option<&String> {
        match &instance.version {
            Some(own) => self.installed.iter().find(|t| *t == own),
            None => self.launcher_version(),
        }
    }

    /// Keeps the selected instance on `tag`, or lets it follow the launcher's version.
    pub(super) fn pin_version(&mut self, tag: Option<String>) {
        let Some(id) = self.selected_instance().map(|i| i.id.clone()) else {
            return;
        };
        match instances::edit(&id, |m| m.version = tag) {
            Ok(_) => self.reload_instances(),
            Err(e) => self.client_error = Some(e),
        }
    }

    pub(super) fn select_instance(&mut self, instance: &Instance) {
        self.settings.instance =
            (instance.kind != InstanceKind::Default).then(|| instance.id.clone());
        self.settings.save();
    }

    /// A texture from the selected version's prepared files, its first frame, drawn as pixels.
    fn texture_image(&mut self, name: &str, size: f32) -> Option<Arc<RenderImage>> {
        let tag = self.selected().filter(|t| backend::is_prepared(t))?.clone();
        let scale = self.scale;
        self.instances
            .icons
            .entry((name.to_string(), (size * scale) as u32, false))
            .or_insert_with(|| {
                let path = Icon::Texture(name.into()).texture(&backend::resources(&tag))?;
                let png = image::open(path).ok()?.to_rgba8();
                let w = png.width() as usize;
                // An animated texture is its frames one under another: the first is square.
                let texels: Vec<[u8; 4]> = png.pixels().take(w * w).map(|p| p.0).collect();
                Some(art::bake(&texels, w, w, size / w as f32, scale))
            })
            .clone()
    }

    /// An instance's icon, `size` points square (`instance_icons.rs` says which).
    pub(super) fn instance_icon(&mut self, instance: &Instance, size: f32) -> AnyElement {
        let t = self.theme;
        if let Some(picture) = self.instance_picture(instance, size) {
            return instance_icons::centred(picture, size, self.scale).into_any_element();
        }
        let framed = |d: Div| {
            d.flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(size))
                .border_2()
                .border_color(t.outline)
        };
        match &instance.icon {
            Some(Icon::Colour(hex)) => {
                let rgb = u32::from_str_radix(&hex[1..], 16).unwrap_or(0);
                return framed(div().bg(gpui::rgb(rgb)))
                    .child(
                        blocks::pixel((size * 0.42).round(), FontWeight::BOLD)
                            .text_color(gpui::white())
                            .child(initial(&instance.name)),
                    )
                    .into_any_element();
            }
            Some(Icon::Texture(name)) => {
                if let Some(image) = self.texture_image(name, size - 8.) {
                    return framed(div().bg(t.well))
                        .child(gpui::img(image).size(px(size - 8.)))
                        .into_any_element();
                }
            }
            None => {
                if let Some(mark) = self.instance_mark(instance, size) {
                    return instance_icons::centred(mark, size, self.scale).into_any_element();
                }
            }
        }
        blocks::slot(None, &instance.name, size, &t).into_any_element()
    }

    /// The Instances page (concept): Rust and Java side by side, each instance a card - its
    /// icon, what it plays, when it was played, Open and its own Play. New instance opens the
    /// dialog the sidebar's `+` did. Card banner art joins with per-instance banners (#8).
    /// Finds the instances other launchers on this computer already have
    /// (`other_launchers::scan`: their usual folders, nothing written), with why each one's
    /// mods can't come along. On a background thread.
    fn scan_other_launchers(&mut self, cx: &mut Context<Self>) {
        if self.instances.others_busy {
            return;
        }
        self.instances.others_busy = true;
        self.instances.others_error = None;
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            match sources::Folders::current() {
                Some(folders) => other_launchers::scan(sources::Os::current(), &folders)
                    .into_iter()
                    .map(|f| {
                        let blocked = other_launchers::mods_blocked(&f);
                        (f, blocked)
                    })
                    .collect(),
                None => Vec::new(),
            }
        });
        cx.spawn(async move |this, cx| {
            let found = task.await;
            let _ = this.update(cx, |this, cx| {
                this.instances.others = Some(found);
                this.instances.others_busy = false;
                cx.notify();
            });
        })
        .detach();
    }

    /// Imports one of them on a background thread: a copy under its name (never overwrites - a
    /// name already taken fails and says so), its loader when Horizon installs it, and its mods
    /// exactly as the row said when it was clicked: `blocked` is the scan's reason its mods
    /// can't come along, the same decision the button's label showed. The folder may have
    /// changed since the scan: `other_launchers::import` checks again then and fails visibly,
    /// never silently turning Import into Import without mods.
    fn import_other_launcher(
        &mut self,
        found: other_launchers::Found,
        blocked: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.instances.others_busy {
            return;
        }
        self.instances.others_busy = true;
        self.instances.others_error = None;
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            let with_mods = blocked.is_none();
            let name = found.name.clone();
            let imported = other_launchers::import(&found, &name, None, with_mods)?;
            Ok::<_, String>((imported, found.game))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.instances.others_busy = false;
                match result {
                    Ok((instance, game)) => {
                        if let Some(found) = &mut this.instances.others {
                            found.retain(|(f, _)| f.game != game);
                        }
                        this.reload_instances();
                        this.select_instance(&instance);
                        this.go(Page::Play);
                    }
                    Err(e) => this.instances.others_error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The "From other launchers" section (#7): Find scans once, on demand; each row says what
    /// the instance plays and imports a copy of it. Mods come along only when Horizon installs
    /// their loader - the row shows why not and offers "Import without mods" otherwise, and a
    /// version the launcher doesn't record leaves the row without an import to run.
    fn other_launchers_section(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let n = t.night;
        let busy = self.instances.others_busy;
        let mut rows: Vec<AnyElement> = Vec::new();
        match &self.instances.others {
            None => rows.push(
                shell::text(theme::night::CAPTION, shell::c(n.muted))
                    .child(
                        "Find the instances other launchers on this computer already have."
                            .to_string(),
                    )
                    .into_any_element(),
            ),
            Some(found) if found.is_empty() => rows.push(
                shell::text(theme::night::CAPTION, shell::c(n.muted))
                    .child("No other launchers' instances found.".to_string())
                    .into_any_element(),
            ),
            Some(found) => {
                for (i, (f, blocked)) in found.iter().enumerate() {
                    let plays = match (&f.minecraft, &f.loader) {
                        (None, _) => {
                            "its launcher doesn't record its Minecraft version".to_string()
                        }
                        (Some(v), Some(l)) => {
                            format!("Minecraft {v} with {} {}", l.name, l.version)
                        }
                        (Some(v), None) => format!("Minecraft {v}"),
                    };
                    let mut text_col = div()
                        .flex()
                        .flex_1()
                        .min_w_0()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            shell::text(theme::night::CAPTION, shell::c(n.ink))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!("{} - {}", f.name, f.launcher)),
                        )
                        .child(
                            shell::text(theme::night::CAPTION, shell::c(n.muted)).child(plays),
                        );
                    if let Some(why) = blocked {
                        text_col = text_col.child(
                            shell::text(theme::night::CAPTION, shell::c(n.muted))
                                .child(why.clone()),
                        );
                    }
                    let label = if blocked.is_some() {
                        "Import without mods"
                    } else {
                        "Import"
                    };
                    let mut b = button(format!("import-other-{i}"), label, Kind::Secondary, &t);
                    if !busy && f.minecraft.is_some() {
                        let f = f.clone();
                        let blocked = blocked.clone();
                        b = b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.import_other_launcher(f.clone(), blocked.clone(), cx);
                        }));
                    } else {
                        b = b.opacity(0.5);
                    }
                    rows.push(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(16.))
                            .p(px(12.))
                            .border_2()
                            .border_color(t.outline)
                            .child(text_col)
                            .child(b.flex_none())
                            .into_any_element(),
                    );
                }
            }
        }
        let mut find = button("find-other-launchers", "Find", Kind::Secondary, &t);
        if busy {
            find = find.opacity(0.5);
        } else {
            find = find.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.scan_other_launchers(cx);
            }));
        }
        let mut footer = div().flex().items_center().gap(px(12.)).mt(px(10.)).child(find);
        if let Some(e) = &self.instances.others_error {
            footer = footer
                .child(shell::text(theme::night::CAPTION, shell::c(n.muted)).child(e.clone()));
        }
        div()
            .flex()
            .flex_col()
            .mt(px(26.))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .child(
                        shell::text(theme::night::CAPTION, shell::c(n.muted))
                            .child("From other launchers".to_string()),
                    )
                    .child(
                        shell::text(theme::night::CAPTION, shell::c(n.muted))
                            .font_family(theme::MONO)
                            .text_size(px(11.))
                            .child(
                                "import a copy - the other launcher's instance stays as it is"
                                    .to_string(),
                            ),
                    ),
            )
            .child(div().flex().flex_col().gap(px(10.)).mt(px(10.)).children(rows))
            .child(footer)
    }

    pub(super) fn instances_page(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let n = t.night;
        let (mut rust, mut java): (Vec<AnyElement>, Vec<AnyElement>) = (Vec::new(), Vec::new());
        for i in self.instances.list.clone() {
            if !matches!(i.kind, InstanceKind::Default | InstanceKind::Own) {
                continue;
            }
            let card = self.instance_card(&i, cx);
            if i.java.is_some() {
                java.push(card);
            } else {
                rust.push(card);
            }
        }
        let section = |title: &str, note: Option<&'static str>, cards: Vec<AnyElement>| {
            div()
                .flex()
                .flex_col()
                .mt(px(26.))
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .justify_between()
                        .child(
                            shell::text(theme::night::CAPTION, shell::c(n.muted))
                                .child(tr(title).to_string()),
                        )
                        .children(note.map(|s| {
                            shell::text(theme::night::CAPTION, shell::c(n.muted))
                                .font_family(theme::MONO)
                                .text_size(px(11.))
                                .child(tr(s).to_string())
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(16.))
                        .mt(px(10.))
                        .children(cards),
                )
        };
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_end()
                    .justify_between()
                    .child(page_header(
                        "Instances",
                        "Rust and Java live side by side. Each instance keeps its own version, runtime, mods, configs and saves.",
                        &t,
                    ))
                    .child(
                        button("new-instance", "New instance", Kind::Primary, &t).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.open_instance_dialog(None);
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .child(section("Rust", None, rust))
            .child(section(
                "Java",
                Some("Software is chosen inside each Java instance - vanilla, Fabric or Quilt."),
                java,
            ))
            .child(self.other_launchers_section(cx))
    }

    /// One instance's card on the Instances page.
    fn instance_card(&mut self, i: &Instance, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let n = t.night;
        let icon = self.instance_icon(i, 40.);
        let banner = self.instance_banner(i, 296.);
        let mut chips: Vec<Div> = Vec::new();
        match &i.java {
            Some(j) => {
                chips.push(shell::chip_tag("Java", &t));
                chips.push(shell::chip_tag(
                    match &j.loader {
                        Some(l) => l.kind.name().to_string(),
                        None => tr("Vanilla").to_string(),
                    },
                    &t,
                ));
                chips.push(shell::chip_tag(j.minecraft.clone(), &t));
            }
            None => {
                chips.push(shell::chip_tag("Horizon engine", &t));
                if let Some(tag) = self.version_of(i) {
                    chips.push(shell::chip_tag(
                        horizon_launcher::nightly::label(tag)
                            .unwrap_or_else(|| tag.trim_start_matches('v').to_string()),
                        &t,
                    ));
                }
            }
        }
        let ready = i.java.is_some()
            || self
                .version_of(i)
                .map(String::as_str)
                .is_some_and(backend::is_prepared);
        let open = i.clone();
        let play = i.clone();
        let card = i.clone();
        let id = i.id.clone();
        crate::plate::plate(&n)
            .id(SharedString::from(format!("instance-card-{id}")))
            .flex()
            .flex_col()
            .w(px(296.))
            .overflow_hidden()
            .cursor_pointer()
            // The banner's place: the instance's own banner.png when it has a readable one
            // (cover-cropped to the strip); the raised strip carries the icon either way.
            .child(
                div()
                    .relative()
                    .h(px(64.))
                    .overflow_hidden()
                    .bg(plate::alpha(n.card_hi, n.card_hi_alpha))
                    .when_some(banner, |strip, image| {
                        strip.child(
                            gpui::img(image)
                                .w(px(296.))
                                .h(px(64.))
                                .object_fit(gpui::ObjectFit::Cover),
                        )
                    })
                    .child(
                        div()
                            .absolute()
                            .left(px(12.))
                            .bottom(px(-16.))
                            .child(div().relative().child(icon)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .pt(px(24.))
                    .px(px(16.))
                    .pb(px(16.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                shell::text(theme::night::UI, shell::c(n.ink))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(i.name.clone()),
                            )
                            .child(
                                shell::action(
                                    SharedString::from(format!("card-open-{id}")),
                                    shell::Action::Quiet,
                                    &t,
                                )
                                .h(px(30.))
                                .child(shell::label(tr("Open").to_string()))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.select_instance(&open);
                                    this.go(Page::Play);
                                    cx.notify();
                                })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .children(chips),
                    )
                    .child(
                        shell::text(theme::night::CAPTION, shell::c(n.muted))
                            .font_family(theme::MONO)
                            .text_size(px(11.))
                            .child(played_line(i)),
                    )
                    .child(
                        shell::action(
                            SharedString::from(format!("card-play-{id}")),
                            shell::Action::Raised,
                            &t,
                        )
                        .w_full()
                        .child(
                            div()
                                .relative()
                                .child(blocks::icon("play", 14., shell::c(n.ink))),
                        )
                        .child(shell::label(tr("Play").to_string()))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.select_instance(&play);
                            match play_press(ready, this.owner, false) {
                                PlayPress::Nothing => {}
                                PlayPress::Play => this.play(),
                                PlayPress::PrepareThenPlay => {
                                    this.play_when_prepared = this.selected().cloned();
                                    this.prepare(backend::MinecraftFrom::Install);
                                }
                            }
                            this.go(Page::Play);
                            cx.notify();
                        })),
                    ),
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.select_instance(&card);
                this.go(Page::Play);
                cx.notify();
            }))
            .into_any_element()
    }

    /// Opens the dialog for a new instance, or for changing `editing`.
    pub(super) fn open_instance_dialog(&mut self, editing: Option<Instance>) {
        let (name, group) = editing
            .as_ref()
            .map(|i| (i.name.clone(), i.group.clone().unwrap_or_default()))
            .unwrap_or_default();
        let memory = editing
            .as_ref()
            .and_then(|i| instances::memory_mb(&i.id))
            .map_or(String::new(), |m| m.to_string());
        let java_home = editing
            .as_ref()
            .and_then(|i| i.java.as_ref())
            .and_then(|j| j.home.as_ref())
            .map_or(String::new(), |h| h.display().to_string());
        let jvm_args = editing
            .as_ref()
            .and_then(|i| i.java.as_ref())
            .map_or(String::new(), |j| j.jvm_args.clone());
        self.servers.text(Field::InstanceName).set(name);
        self.servers.text(Field::InstanceGroup).set(group);
        self.servers.text(Field::InstanceMemory).set(memory);
        self.servers.text(Field::InstanceJavaHome).set(java_home);
        self.servers.text(Field::InstanceJvmArgs).set(jvm_args);
        self.servers.focus_soon = Some(Field::InstanceName);
        self.instances.dialog = Some(InstanceDialog {
            icon: editing.as_ref().and_then(|i| i.icon.clone()),
            picture: None,
            banner: None,
            editing: editing.map(|i| i.id),
            copy: false,
            error: None,
            busy: false,
            java: Default::default(),
        });
        self.servers.text(Field::InstanceVersion).set(String::new());
    }

    /// Escape closes the dialog. Whether it did.
    pub(super) fn instance_escape(&mut self) -> bool {
        if self.instances.dialog.as_ref().is_some_and(|d| !d.busy) {
            self.instances.dialog = None;
            return true;
        }
        false
    }

    /// Creates or changes the instance as the dialog says. A copy can be gigabytes of worlds, so
    /// the work runs in the background.
    pub(super) fn save_instance(&mut self, cx: &mut Context<Self>) {
        let selected = self.selected_instance().map(|i| i.id.clone());
        let name = self.servers.value(Field::InstanceName).trim().to_string();
        let group = Some(self.servers.value(Field::InstanceGroup).to_string());
        let memory_raw = self.servers.value(Field::InstanceMemory).trim().to_string();
        let java_home_raw = self
            .servers
            .value(Field::InstanceJavaHome)
            .trim()
            .to_string();
        let jvm_args_raw = self.servers.value(Field::InstanceJvmArgs).trim().to_string();
        let Some(d) = self.instances.dialog.as_mut().filter(|d| !d.busy) else {
            return;
        };
        let icon = d.icon.clone();
        let picture = d
            .picture
            .as_ref()
            .map(|p| p.as_ref().map(|c| c.png.clone()));
        let banner = d
            .banner
            .as_ref()
            .map(|b| b.as_ref().map(|c| c.png.clone()));
        let editing = d.editing.clone();
        let copy = selected.filter(|_| d.copy);
        // A new Java instance: what it plays, and its name when none is typed.
        let java = (editing.is_none() && d.java.kind != new_instance::NewKind::Horizon)
            .then(|| d.java.clone());
        let minecraft = java.as_ref().map(|j| {
            j.minecraft(match &self.instances.versions {
                new_instance::VersionList::Ready(v) => Some(v),
                _ => None,
            })
        });
        let Some(d) = self.instances.dialog.as_mut() else {
            return;
        };
        let (java, name) = match (java, minecraft) {
            (Some(_), Some(None)) => {
                d.error = Some(tr("Mojang's version list isn't here yet.").into());
                return;
            }
            (Some(j), Some(Some(mc))) => {
                let name = if name.is_empty() {
                    j.default_name(&mc)
                } else {
                    name
                };
                (Some((j, mc)), name)
            }
            _ => (None, name),
        };
        // Memory is an edit-dialog row: empty keeps the default; otherwise a sane -Xmx range.
        let memory = if editing.is_some() && !memory_raw.is_empty() {
            match memory_raw.parse::<u32>() {
                Ok(m) if (512..=65536).contains(&m) => Some(m),
                _ => {
                    d.error = Some(format!(
                        "Memory is 512 to 65536 MiB, or empty for the default ({} MiB).",
                        instances::DEFAULT_MEMORY_MB
                    ));
                    return;
                }
            }
        } else {
            None
        };
        // A Java home, when typed, must be a folder that is there; empty: the automatic pick.
        let java_home = if editing.is_some() && !java_home_raw.is_empty() {
            let path = std::path::PathBuf::from(&java_home_raw);
            if path.is_dir() {
                Some(Some(path))
            } else {
                d.error = Some(format!("{java_home_raw} is not a folder that is there."));
                return;
            }
        } else if editing.is_some() {
            Some(None)
        } else {
            None
        };
        // Arguments apply to the instance being changed; a new one starts without its own.
        let editing_needs_args = editing.is_some();
        d.busy = true;
        d.error = None;
        let task = cx.background_executor().spawn(async move {
            let id = match (editing, java) {
                (Some(id), _) => id,
                (None, Some((j, mc))) => new_instance::create_java(&j, &name, &mc)?.id,
                (None, None) => instances::create(&name, copy.as_deref())?.id,
            };
            let instance = instances::edit(&id, |m| {
                m.name = Some(name);
                m.group = group;
                m.icon = icon;
                if let Some(j) = &mut m.java {
                    j.memory_mb = memory;
                    if let Some(home) = &java_home {
                        j.home = home.clone();
                    }
                    if editing_needs_args {
                        j.jvm_args = jvm_args_raw;
                    }
                }
            })?;
            if let Some(png) = picture {
                instances::set_picture(
                    &instance.home,
                    png.as_deref(),
                    &horizon_launcher::desktop_seams::to_trash,
                )?;
            }
            if let Some(png) = banner {
                instances::set_banner(
                    &instance.home,
                    png.as_deref(),
                    &horizon_launcher::desktop_seams::to_trash,
                )?;
            }
            Ok(instance)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(instance) => {
                        this.instances.dialog = None;
                        this.select_instance(&instance);
                        this.reload_instances();
                        if matches!(instance.kind, InstanceKind::Default | InstanceKind::Own) {
                            this.go(Page::Play);
                        }
                    }
                    Err(e) => {
                        if let Some(d) = &mut this.instances.dialog {
                            d.busy = false;
                            d.error = Some(e);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn instance_dialog(&mut self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let t = self.theme;
        let d = self.instances.dialog.as_ref()?;
        let editing_id = d.editing.clone();
        let chosen = d
            .picture
            .as_ref()
            .map(|p| p.as_ref().map(|c| c.preview.clone()));
        let chosen_banner = d
            .banner
            .as_ref()
            .map(|b| b.as_ref().map(|c| c.preview.clone()));
        let (editing, copy, icon, error, busy, kind) = (
            d.editing.is_some(),
            d.copy,
            d.icon.clone(),
            d.error.clone(),
            d.busy,
            d.java.kind,
        );
        let java = !editing && kind != new_instance::NewKind::Horizon;
        let editing_game = editing_id
            .as_ref()
            .and_then(|id| self.instances.list.iter().find(|i| &i.id == id))
            .and_then(|i| i.java.clone());
        let editing_java = editing_game.is_some();
        let n = t.night;
        let from = self
            .selected_instance()
            .map_or_else(|| "Horizon".into(), |i| i.name.clone());
        let row = |label: &'static str, control: AnyElement| {
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    shell::text(theme::night::CAPTION, shell::c(n.muted))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tr(label).to_string()),
                )
                .child(control)
        };
        let mut body = div()
            .relative()
            .flex()
            .flex_col()
            .gap(px(20.))
            .p(px(28.))
            .child(
                shell::text(theme::night::TITLE, shell::c(n.ink)).child(
                    tr(if editing {
                        "Edit instance"
                    } else {
                        "New instance"
                    })
                    .to_string(),
                ),
            );
        if !editing {
            body = body.child(self.kind_tabs(cx));
        }
        if java {
            body = body.child(self.java_choice_rows(cx));
        }
        body = body.child(row(
            "Name",
            self.field(Field::InstanceName, "My instance", 400., cx)
                .into_any_element(),
        ));
        if !editing && !java {
            let weak = cx.entity().downgrade();
            body = body.child(row(
                "Start from",
                segments(
                    "instance-from",
                    &[(false, "Empty"), (true, "A copy of the selected one")],
                    copy,
                    &t,
                    move |copy, _, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            if let Some(d) = &mut this.instances.dialog {
                                d.copy = copy;
                            }
                            cx.notify();
                        });
                    },
                )
                .into_any_element(),
            ));
            body = body.child(text(theme::CAPTION).text_color(t.muted).child(if copy {
                format!("{from}'s worlds, options and packs are copied; {from} stays as it is.")
            } else {
                "Its own worlds and packs; your options and player name come along.".to_string()
            }));
        }
        body = body.child(row(
            "Group",
            self.field(Field::InstanceGroup, "No group", 400., cx)
                .into_any_element(),
        ));
        if editing_java {
            body = body.child(row(
                "Memory (MiB)",
                self.field(
                    Field::InstanceMemory,
                    instances::DEFAULT_MEMORY_MB.to_string(),
                    400.,
                    cx,
                )
                .into_any_element(),
            ));
            body = body.child(text(theme::CAPTION).text_color(t.muted).child(format!(
                "The most the game may use (-Xmx). Empty uses the default ({} MiB).",
                instances::DEFAULT_MEMORY_MB
            )));
            body = body.child(row(
                "Java home",
                self.field(Field::InstanceJavaHome, "Automatic pick", 400., cx)
                    .into_any_element(),
            ));
            body = body.child(text(theme::CAPTION).text_color(t.muted).child(
                "The Java that runs it. Empty: the launcher picks one for its Minecraft.".to_string(),
            ));
            body = body.child(row(
                "JVM arguments",
                self.field(Field::InstanceJvmArgs, "None", 400., cx)
                    .into_any_element(),
            ));
        }
        if let Some(game) = &editing_game {
            // What it plays, read-only here: changing them is a new instance, not an edit.
            body = body.child(row(
                "Minecraft",
                text(theme::CAPTION)
                    .text_color(t.muted)
                    .child(game.minecraft.clone())
                    .into_any_element(),
            ));
            body = body.child(row(
                "Loader",
                text(theme::CAPTION)
                    .text_color(t.muted)
                    .child(
                        game.loader
                            .as_ref()
                            .map_or("Vanilla".to_string(), |l| l.kind.name().to_string()),
                    )
                    .into_any_element(),
            ));
            // Carried notes, read-only: where it came from wrote them, editing them here is not
            // part of the import-carry scope.
            let notes = editing_id
                .as_ref()
                .and_then(|id| self.instances.list.iter().find(|i| &i.id == id))
                .and_then(|i| i.notes.clone());
            if let Some(notes) = notes {
                body = body.child(row(
                    "Notes",
                    text(theme::CAPTION)
                        .text_color(t.muted)
                        .child(notes)
                        .into_any_element(),
                ));
            }
        }
        let mut tiles: Vec<AnyElement> = Vec::new();
        let tile = |id: String, chosen: bool| {
            div()
                .id(SharedString::from(id))
                .flex()
                .items_center()
                .justify_center()
                .size(px(40.))
                .border_2()
                .border_color(if chosen { t.accent } else { t.rule })
                .cursor_pointer()
                .hover(move |s| s.border_color(t.outline))
        };
        let instance =
            editing_id.and_then(|id| self.instances.list.iter().find(|i| i.id == id).cloned());
        // The picture it will show: one just chosen, else its own unless that is being removed.
        let picture = match chosen {
            Some(chosen) => chosen,
            None => instance
                .as_ref()
                .and_then(|i| self.instance_picture(i, 28.)),
        };
        let pictured = picture.is_some();
        if let Some(image) = picture {
            tiles.push(
                tile("instance-icon-picture".into(), true)
                    .child(instance_icons::centred(image, 28., self.scale))
                    .into_any_element(),
            );
        }
        // Nothing chosen: the instance's mark (its loader's, Horizon's), when it has one.
        let mark = instance.as_ref().and_then(|i| self.instance_mark(i, 28.));
        tiles.push(
            tile("instance-icon-none".into(), icon.is_none() && !pictured)
                .child(match mark {
                    Some(mark) => instance_icons::centred(mark, 28., self.scale).into_any_element(),
                    None => text(theme::MICRO)
                        .text_color(t.muted)
                        .child("NONE")
                        .into_any_element(),
                })
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.instances.dialog {
                        d.icon = None;
                        d.picture = Some(None);
                    }
                    cx.notify();
                }))
                .into_any_element(),
        );
        for name in TEXTURES {
            let Some(image) = self.texture_image(name, 28.) else {
                continue;
            };
            let chosen = !pictured && icon == Some(Icon::Texture(name.into()));
            tiles.push(
                tile(format!("instance-icon-{name}"), chosen)
                    .bg(t.well)
                    .child(gpui::img(image).size(px(28.)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(d) = &mut this.instances.dialog {
                            d.icon = Some(Icon::Texture(name.into()));
                            d.picture = Some(None);
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        for hex in COLOURS {
            let chosen = !pictured && icon == Some(Icon::Colour(hex.into()));
            let rgb = u32::from_str_radix(&hex[1..], 16).unwrap_or(0);
            tiles.push(
                tile(format!("instance-icon-{hex}"), chosen)
                    .child(div().size(px(28.)).bg(gpui::rgb(rgb)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(d) = &mut this.instances.dialog {
                            d.icon = Some(Icon::Colour(hex.into()));
                            d.picture = Some(None);
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        let choose = shell::action("instance-picture", shell::Action::Quiet, &t)
            .child(shell::label(tr("Choose image…").to_string()))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.choose_instance_picture(cx);
            }));
        // Rows of eleven: a wrapping row is measured as one line, and the plate would end above
        // the buttons.
        let (mut tiles, mut rows) = (tiles.into_iter().peekable(), Vec::new());
        while tiles.peek().is_some() {
            rows.push(div().flex().gap(px(6.)).children(tiles.by_ref().take(11)));
        }
        body = body.child(row(
            "Icon",
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .children(rows)
                .child(div().flex().child(choose))
                .into_any_element(),
        ));
        // The banner it will show: one just chosen, else its own unless that is being removed.
        let banner_now = match &chosen_banner {
            Some(chosen) => chosen.clone(),
            None => instance
                .as_ref()
                .and_then(|i| self.instance_banner(i, 160.)),
        };
        let choose_banner = shell::action("instance-banner", shell::Action::Quiet, &t)
            .child(shell::label(tr("Choose image…").to_string()))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.choose_instance_banner(cx);
            }));
        let remove_banner = banner_now.is_some().then(|| {
            shell::action("instance-banner-remove", shell::Action::Quiet, &t)
                .child(shell::label(tr("Remove").to_string()))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.instances.dialog {
                        d.banner = Some(None);
                    }
                    cx.notify();
                }))
        });
        body = body.child(row(
            "Banner",
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    div()
                        .id("instance-banner-preview")
                        .w(px(160.))
                        .h(px(40.))
                        .overflow_hidden()
                        .bg(plate::alpha(n.card_hi, n.card_hi_alpha))
                        .when_some(banner_now, |strip, image| {
                            strip.child(
                                gpui::img(image)
                                    .w(px(160.))
                                    .h(px(40.))
                                    .object_fit(gpui::ObjectFit::Cover),
                            )
                        }),
                )
                .child(choose_banner)
                .children(remove_banner)
                .into_any_element(),
        ));
        if self.selected().is_none_or(|t| !backend::is_prepared(t)) {
            body = body.child(
                text(theme::CAPTION)
                    .text_color(t.muted)
                    .child("Block and item icons come from your prepared game files: prepare a version to see them."),
            );
        }
        if let Some(e) = error {
            body = body.child(text(theme::CAPTION).text_color(t.danger).child(e));
        }
        let cancel = shell::action("instance-cancel", shell::Action::Quiet, &t)
            .child(shell::label(tr("Cancel").to_string()));
        let label = match (editing, busy, copy) {
            (_, true, true) if !java => "Copying…",
            (true, false, _) => "Save",
            _ => "Create",
        };
        let save = shell::action("instance-save", shell::Action::Main, &t)
            .h(px(theme::CONTROL + 6.))
            .text_size(px(theme::night::UI.px))
            .child(shell::label(tr(label).to_string()));
        // A modpack becomes an instance by being imported, on the Mods page.
        let import = (!editing && !busy).then(|| {
            shell::action("instance-import", shell::Action::Quiet, &t)
                .child(shell::label(tr("Import a modpack…").to_string()))
                .mr_auto()
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.instances.dialog = None;
                    this.show_modpack(None);
                    cx.notify();
                }))
        });
        let foot = div()
            .relative()
            .flex()
            .justify_end()
            .gap(px(8.))
            .px(px(28.))
            .py(px(16.))
            .border_t_1()
            .border_color(plate::alpha(n.stroke, n.rule_alpha))
            .items_center()
            .children(import);
        let foot = if busy {
            foot.child(disabled(cancel)).child(disabled(save))
        } else {
            foot.child(cancel.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.instances.dialog = None;
                cx.notify();
            })))
            .child(save.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.save_instance(cx);
                cx.notify();
            })))
        };
        // Opaque, as the sign-in sheet: it sits over the page's content.
        let card = plate::cut(
            theme::night::PIXEL_CORNER,
            2,
            Some(plate::alpha(n.card, 0.98)),
            Some((plate::alpha(n.stroke, 2. * n.stroke_alpha), 1.)),
        )
        .flex()
        .flex_col()
        .flex_none()
        .w_full()
        .max_w(px(600.))
        .child(body)
        .child(foot);
        Some(
            div()
                .id("instance-dialog")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .px(px(32.))
                .py(px(24.))
                // A Java kind's list can make it taller than the window.
                .overflow_y_scroll()
                .bg(plate::alpha(n.page, 0.8))
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .occlude()
                .child(card),
        )
    }

    /// The Play page's row for the selected instance: its folder, and changing, copying or
    /// exporting it.
    pub(super) fn instance_row(&mut self, n: usize, cx: &mut Context<Self>) -> Option<Div> {
        let t = self.theme;
        let instance = self.selected_instance()?.clone();
        let (home, edit, name) = (
            instance.home.clone(),
            instance.clone(),
            instance.name.clone(),
        );
        // Under the name, wrapping: six actions don't fit beside it at the window's narrowest.
        let actions = div()
            .flex()
            .flex_wrap()
            .gap(px(8.))
            .mt(px(6.))
            .ml(px(-14.))
            .child(
                button("instance-edit", "Edit…", Kind::Ghost, &t).on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.open_instance_dialog(Some(edit.clone()));
                        cx.notify();
                    },
                )),
            )
            .child(
                button("instance-copy", "Copy…", Kind::Ghost, &t).on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.open_instance_dialog(None);
                        this.servers
                            .text(Field::InstanceName)
                            .set(format!("{name} copy"));
                        if let Some(d) = &mut this.instances.dialog {
                            d.copy = true;
                        }
                        cx.notify();
                    },
                )),
            )
            .child(
                button(
                    "instance-export",
                    if self.export.is_some() {
                        "Close export"
                    } else {
                        "Export…"
                    },
                    Kind::Ghost,
                    &t,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.toggle_export();
                    cx.notify();
                })),
            )
            .child(
                button(
                    "instance-launch",
                    if self.launch.is_some() {
                        "Close launch"
                    } else {
                        "Launch…"
                    },
                    Kind::Ghost,
                    &t,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.toggle_launch();
                    cx.notify();
                })),
            )
            .child(self.shortcut_button(&instance, cx))
            .child(
                button("instance-folder", "Open folder", Kind::Ghost, &t).on_click(
                    move |_, _, cx| {
                        let _ = std::fs::create_dir_all(&home);
                        cx.open_with_system(&home);
                    },
                ),
            )
            .children(self.delete_button(&instance, cx));
        Some(index_row(
            n,
            tr("Instance"),
            Some(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(10.))
                            .child(instance.name.clone())
                            .child(
                                text(theme::CAPTION)
                                    .text_color(t.muted)
                                    .child(played_line(&instance)),
                            ),
                    )
                    .children(self.shortcut_note(&instance.id))
                    .child(actions)
                    .children(self.delete_confirmation(&instance, cx))
                    .into_any_element(),
            ),
            None,
            &t,
        ))
    }
}

impl Launcher {
    /// Delete…, for every instance but Horizon's own (the game's home); not while a game runs.
    fn delete_button(&self, instance: &Instance, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        if instance.kind == InstanceKind::Default {
            return None;
        }
        let t = self.theme;
        let delete = button("instance-delete", "Delete…", Kind::Ghost, &t);
        if self.client.is_some() || self.starting_client.is_some() {
            return Some(disabled(delete));
        }
        let id = instance.id.clone();
        Some(
            delete.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.instances.confirm_delete = Some(id.clone());
                this.instances.delete_error = None;
                cx.notify();
            })),
        )
    }

    /// "Move it to the Trash?" under the actions, once Delete… is pressed.
    fn delete_confirmation(&self, instance: &Instance, cx: &mut Context<Self>) -> Option<Div> {
        if self.instances.confirm_delete.as_ref() != Some(&instance.id) {
            return None;
        }
        let t = self.theme;
        let id = instance.id.clone();
        let line = match &self.instances.delete_error {
            Some(e) => e.clone(),
            None => trf(
                "Move {name} to the Trash, with its worlds and settings? You can get it back from there.",
                &[("name", &instance.name)],
            ),
        };
        Some(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(8.))
                .mt(px(8.))
                .child(text(theme::CAPTION).text_color(t.danger).child(line))
                .child(
                    button("instance-keep", "Keep", Kind::Ghost, &t).on_click(cx.listener(
                        |this, _: &ClickEvent, _, cx| {
                            this.instances.confirm_delete = None;
                            this.instances.delete_error = None;
                            cx.notify();
                        },
                    )),
                )
                .child(
                    button("instance-trash", "Move to Trash", Kind::Secondary, &t).on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.delete_instance(&id);
                            cx.notify();
                        }),
                    ),
                ),
        )
    }

    fn delete_instance(&mut self, id: &str) {
        if self.client.is_some() || self.starting_client.is_some() {
            return;
        }
        match instances::delete(id) {
            Ok(_) => {
                self.instances.confirm_delete = None;
                // Play starts Horizon again; the window's copy of the settings says so too.
                if self.settings.instance.as_deref() == Some(id) {
                    self.settings.instance = None;
                    self.settings.save();
                }
                self.reload_instances();
            }
            Err(e) => self.instances.delete_error = Some(e),
        }
    }
}

/// "3 h 05 min in total · last played today", or "Not played yet".
pub(super) fn played_line(instance: &Instance) -> String {
    match instance.last_played {
        Some(when) => trf(
            "{time} in total · last played {when}",
            &[
                ("time", &instances::play_time(instance.played)),
                ("when", &worlds_page::ago(when)),
            ],
        ),
        None => tr("Not played yet").into(),
    }
}

/// The first letter or digit of a name, for an icon without a picture.
fn initial(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .unwrap_or('?')
        .to_uppercase()
        .to_string()
}
