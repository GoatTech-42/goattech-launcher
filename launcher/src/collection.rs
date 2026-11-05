//! Data collection in the window: the question asked once (and again when what is collected
//! changes), and its section in Settings. Everything goes through `horizon_launcher::telemetry`.
use super::*;
use horizon_launcher::telemetry;
use horizon_telemetry::{Choices, Consent, Kind as DataKind};

/// What the window keeps about data collection.
/// Direct-child indices in the consent body's scroll view, for keyboard scroll-follow.
/// The five choice rows are direct children in this order; details sits after NEVER.
pub(crate) const ASK_CHILD_BASIC: usize = 3;
pub(crate) const ASK_CHILD_DETAILS: usize = 10;
/// The example reports, right after the details row, once they show.
pub(crate) const ASK_CHILD_JSON: usize = 11;

pub(crate) struct Collection {
    pub consent: Consent,
    body_scroll: Scrollbar,
    json_scroll: Scrollbar,
    /// The question's switches, until the player answers.
    pub draft: Choices,
    /// "View what would be sent": a line on what it is, and the JSON.
    pub view: Option<(String, String)>,
    /// What "Delete my data" did, or why it could not.
    pub note: Option<String>,
    pub confirm_delete: bool,
}

impl Collection {
    /// Keep child `ix` of the question's body visible (deferred to the next layout, so
    /// it measures the content as it will be then).
    pub(crate) fn body_reveal(&self, ix: usize) {
        self.body_scroll.handle.scroll_to_item(ix);
    }

    /// Scroll the question's body so child `ix` is its first visible row. The request pends
    /// until the child has bounds, so a region that is still being laid out is landed on once
    /// it measures.
    pub(crate) fn body_to_item_top(&self, ix: usize) {
        self.body_scroll.handle.scroll_to_top_of_item(ix);
    }

    /// Scroll the question's body back to its first row.
    pub(crate) fn body_to_top(&self) {
        self.body_scroll
            .handle
            .set_offset(gpui::point(px(0.), px(0.)));
    }

    /// The open example reports, read top to bottom under the keyboard: Down steps the
    /// question's body a few lines at a time until its foot - where the JSON plate
    /// sits - is on screen, then steps the plate's own lines; Up walks back the same
    /// way, always stepping the region that is on screen: the body whenever it is away
    /// from its foot (the plate is off screen then), the plate only while it shows at
    /// the foot, then the body again. Every step derives from the two scroll handles'
    /// actual offsets, so after a Tab away or a pointer scroll the next key moves the
    /// visible region; the plate keeps its own place while it is off screen and a
    /// reopened view starts it at its first line. The offsets are clamped by hand
    /// because ScrollHandle::set_offset takes the offset as given (offsets grow
    /// negative downward).
    pub(crate) fn json_key_scroll(&self, down: bool) {
        if self.view.is_none() {
            return;
        }
        let body = &self.body_scroll.handle;
        let plate = &self.json_scroll.handle;
        let body_max = body.max_offset().y;
        let at_foot = body.offset().y <= -body_max;
        if down {
            if !at_foot {
                let mut off = body.offset();
                off.y = (off.y - px(60.)).clamp(-body_max, px(0.));
                body.set_offset(off);
            } else {
                let max = plate.max_offset().y;
                let mut off = plate.offset();
                off.y = (off.y - px(60.)).clamp(-max, px(0.));
                plate.set_offset(off);
            }
        } else if !at_foot {
            let mut off = body.offset();
            off.y = (off.y + px(60.)).clamp(-body_max, px(0.));
            body.set_offset(off);
        } else if plate.offset().y < px(0.) {
            let max = plate.max_offset().y;
            let mut off = plate.offset();
            off.y = (off.y + px(60.)).clamp(-max, px(0.));
            plate.set_offset(off);
        } else {
            let mut off = body.offset();
            off.y = (off.y + px(60.)).clamp(-body_max, px(0.));
            body.set_offset(off);
        }
    }

    pub fn load() -> Self {
        Self {
            consent: telemetry::consent(),
            body_scroll: Scrollbar::default(),
            json_scroll: Scrollbar::default(),
            draft: Choices::NONE,
            view: None,
            note: None,
            confirm_delete: false,
        }
    }
}

fn kind_id(prefix: &str, kind: DataKind) -> &'static str {
    match (prefix, kind) {
        ("ask", DataKind::Perf) => "ask-perf",
        ("ask", DataKind::Crash) => "ask-crash",
        ("ask", DataKind::Bug) => "ask-bug",
        ("ask", DataKind::Benchmark) => "ask-benchmark",
        (_, DataKind::Perf) => "data-perf",
        (_, DataKind::Crash) => "data-crash",
        (_, DataKind::Bug) => "data-bug",
        (_, DataKind::Benchmark) => "data-benchmark",
    }
}

/// Where a switch would be: the ping has none.
/// The ping's state chip: "On while playing" - the ping goes to GoatTech's collector by default
/// (`horizon_telemetry::live_endpoint`), unless `HORIZON_TELEMETRY_URL=off`.
fn ping_state(t: &Theme) -> Div {
    let label = if horizon_telemetry::live_endpoint().is_some() {
        tr("On while playing")
    } else {
        tr("Off by default")
    };
    caps(label).flex_none().text_color(t.muted)
}

/// The "I'm playing" ping on the first-run question, laid out as the switches' rows.
fn ping_row(t: &Theme) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(16.))
        .py(px(10.))
        .border_b_1()
        .border_color(t.rule)
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .child(
                    text(theme::UI)
                        .text_color(t.ink)
                        .child(tr(telemetry::PING_NAME)),
                )
                .child(
                    text(theme::CAPTION)
                        .text_color(t.muted)
                        .child(tr(telemetry::ping_short())),
                ),
        )
        .child(ping_state(t))
}

/// The full wording for every kind, then the exact JSON, in mono, scrolling inside a plate.
fn json_view(view: &(String, String), t: &Theme, scroll: &Scrollbar) -> Div {
    let detail = |name: &'static str, what: &'static str| {
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(text(theme::UI).text_color(t.ink).child(tr(name)))
            .child(text(theme::UI).text_color(t.ink_soft).child(tr(what)))
    };
    let kinds = telemetry::SWITCHES
        .iter()
        .map(|&(_, name, what)| detail(name, what).into_any_element());
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            // How to read on, said where the reading starts.
            text(theme::CAPTION)
                .text_color(t.accent)
                .child(tr("Down and Up read through the examples.")),
        )
        .child(text(theme::UI).text_color(t.ink_soft).child(view.0.clone()))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(detail(telemetry::BASIC_NAME, telemetry::BASIC_WHAT).into_any_element())
                .children(kinds)
                .child(detail(telemetry::PING_NAME, telemetry::ping_text()).into_any_element()),
        )
        .child(
            scroll.wrap(
                div()
                    .id("data-json")
                    .max_h(px(260.))
                    .overflow_y_scroll()
                    .p(px(16.))
                    .rounded(px(theme::RADIUS_PLATE))
                    .bg(t.plate)
                    .font_family(theme::MONO)
                    .text_size(px(theme::CAPTION.px))
                    .line_height(px(18.))
                    .text_color(t.ink_soft)
                    .children(view.1.lines().map(|l| div().child(l.to_string()))),
                t,
            ),
        )
}

impl Launcher {
    /// Basic summaries off, for real. Cancellation is immediate and does not wait on the
    /// disk: recording the choice vetoes every collection gate in this process at once
    /// (before its save is even queued), the launcher's session is dropped (nothing can
    /// be sent, and a canceled sample can never be resurrected by the switch coming back
    /// on), and the explicit choice is saved off the window's thread, in click order -
    /// the row says so if that save fails, and off still holds for the rest of this
    /// session. Only then is the running game told, on a detached thread so a stuck pipe
    /// never freezes the UI; a failed control line only logs (the game is never asked to
    /// acknowledge), and nothing is sent either way because the launcher's session is
    /// gone.
    fn basic_off(&mut self) {
        self.settings.basic_summaries = Some(false);
        self.basic_session = None;
        horizon_launcher::basic_stats::record_choice_async(self.settings.clone());
        if let Some(client) = &self.client {
            let stdin = client.stdin_shared();
            std::thread::spawn(move || {
                if !crate::backend::Running::send_shared(
                    &stdin,
                    horizon_launcher::basic_stats::CONTROL_OFF,
                ) {
                    eprintln!(
                        "Horizon: the running game could not be told basic summaries are off"
                    );
                }
            });
        }
    }

    /// The basic-summaries row's own switch, on the first-run question and in Settings: the
    /// recorded setting, never the ping's flag and never a consent kind. Turning it off stops
    /// collection for a game already running and drops anything gathered.
    fn basic_switch(
        &self,
        id: &'static str,
        focus: Option<&FocusHandle>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let on = self.settings.basic_summaries == Some(true);
        let sw = switch(id, on, &self.theme);
        let sw = match focus {
            Some(f) => sw.track_focus(f),
            None => sw,
        };
        sw.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            if on {
                this.basic_off();
            } else {
                // Saved off the window's thread like the off path; collection starts
                // only once the ON is actually on disk (the veto lifts then), so a failed
                // save can never turn anything on - and the row says it failed.
                this.settings.basic_summaries = Some(true);
                horizon_launcher::basic_stats::record_choice_async(this.settings.clone());
            }
            cx.notify();
        }))
    }

    /// The basic row, laid out as the switches' rows, with its real switch.
    fn basic_row(&self, id: &'static str, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let failed = horizon_launcher::basic_stats::choice_save_failed();
        let row = horizon_launcher::basic_stats::row_state(self.settings.basic_summaries);
        div()
            .flex()
            .items_start()
            .gap(px(16.))
            .py(px(10.))
            .border_t_1()
            .border_color(t.rule)
            .border_b_1()
            .border_color(t.rule)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        text(theme::UI)
                            .text_color(t.ink)
                            .child(tr(telemetry::BASIC_NAME)),
                    )
                    .child(
                        text(theme::CAPTION)
                            .text_color(t.muted)
                            .child(tr(telemetry::BASIC_SHORT)),
                    )
                    // Only what is actually known (row_state): a save in flight is
                    // "turning", a failed choice with the session veto held is off for
                    // this session (the disk record is unknown), and anything else quotes
                    // the persisted record read from disk NOW - never this window's own
                    // snapshot, which a failed save or another process can leave stale.
                    .child(
                        text(theme::CAPTION)
                            .text_color(t.muted)
                            .child(match row {
                                horizon_launcher::basic_stats::RowState::Saving(true) => {
                                    tr("Turning on...")
                                }
                                horizon_launcher::basic_stats::RowState::Saving(false) => {
                                    tr("Turning off...")
                                }
                                horizon_launcher::basic_stats::RowState::OffThisSession => {
                                    tr("Off for this session.")
                                }
                                horizon_launcher::basic_stats::RowState::OnForInstall => {
                                    tr("On for this install.")
                                }
                                horizon_launcher::basic_stats::RowState::OffForInstall => {
                                    tr("Off for this install.")
                                }
                            }),
                    )
                    // The newest choice's save failed: what the player chose still holds
                    // this session (off) or not at all (on), and the row says so until a
                    // choice actually saves.
                    .when(failed.is_some(), |d| {
                        let (on, error) = failed.as_ref().unwrap();
                        d.child(text(theme::CAPTION).text_color(t.warn).child(if *on {
                            trf(
                                "The choice could not be saved ({error}): basic summaries stay as they were.",
                                &[("error", error)],
                            )
                        } else {
                            trf(
                                "Basic summaries are off for the rest of this session, but the choice could not be saved ({error}): they may be on again the next time the launcher starts.",
                                &[("error", error)],
                            )
                        }))
                    }),
            )
            .child(self.basic_switch(id, Some(&self.consent_keys.basic), cx))
    }

    /// Whether the question covers the window: not answered for this consent version, and the
    /// opening is over.
    pub(crate) fn asking_consent(&self) -> bool {
        self.opening.is_none() && self.collection.consent.needs_asking()
    }

    fn answer_consent(&mut self, choices: Choices) {
        match telemetry::answer(choices) {
            Ok(consent) => {
                self.collection.consent = consent;
                self.collection.view = None;
                self.collection.note = None;
            }
            Err(e) => {
                // The consent file is not written and the question stays exactly as it
                // was - open details included; only its foot gains the reason.
                self.collection.note =
                    Some(trf("Could not save your choice: {error}", &[("error", &e)]))
            }
        }
        // The question and Settings say what the "I'm playing" ping holds, beside the switches.
        if !self.settings.ping_notice_seen {
            self.settings.ping_notice_seen = true;
            let _ = self.settings.save();
        }
    }

    fn toggle_view(&mut self) {
        self.collection.view = match self.collection.view {
            Some(_) => None,
            None => Some(telemetry::preview()),
        };
        // A fresh preview reads from its first line, never an earlier visit's depth.
        if self.collection.view.is_some() {
            self.collection
                .json_scroll
                .handle
                .set_offset(gpui::point(px(0.), px(0.)));
        }
    }

    /// The question, centred over the launcher. Everything optional starts off; "Keep optional data off" is one
    /// click, as easy as saying yes.
    pub(crate) fn consent_screen(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let t = self.theme;
        let draft = self.collection.draft;
        // Built first: the lazy `rows` below holds `cx` until `children(rows)` runs.
        let basic = self.basic_row("ask-basic", cx);
        let rows = telemetry::SWITCHES.iter().map(|&(kind, name, _)| {
            let on = draft.allows(kind);
            div()
                .flex()
                .items_start()
                .gap(px(16.))
                .py(px(10.))
                .border_b_1()
                .border_color(t.rule)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.))
                        .child(text(theme::UI).text_color(t.ink).child(tr(name)))
                        .child(
                            text(theme::CAPTION)
                                .text_color(t.muted)
                                .child(tr(telemetry::short_what(kind))),
                        ),
                )
                .child(
                    switch(kind_id("ask", kind), on, &t)
                        .track_focus(self.consent_keys.for_kind(kind))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.collection.draft.set(kind, !on);
                            cx.notify();
                        })),
                )
        });
        let viewing = self.collection.view.is_some();
        // The body scrolls inside the card, so the answers stay at its foot however short the
        // window and however long what would be sent.
        let body = div()
            .id("ask-body")
            .flex()
            .flex_col()
            .min_h_0()
            .gap(px(12.))
            .px(px(28.))
            .pt(px(28.))
            .pb(px(24.))
            .overflow_y_scroll()
            .child(caps(tr("Data collection")).text_color(t.muted))
            .child(
                text(theme::HEADING)
                    .text_color(t.ink)
                    .child(tr("Help make GoatTech faster and steadier?")),
            )
            .child(text(theme::UI).text_color(t.muted).child(tr(
                if horizon_telemetry::live_endpoint().is_some() {
                    "The short version: on a new install, basic performance summaries start on; the four report kinds start off. \"Keep optional data off\" turns all five off in one click - the anonymous player-count ping runs while you play either way. Settings can change any choice later."
                } else {
                    "The short version: on a new install, basic performance summaries start on; the four report kinds start off. \"Keep optional data off\" turns all five off in one click - the player-count ping is off in this configuration. Settings can change any choice later."
                },
            )))
            .child(basic)
            .children(rows)
            .child(ping_row(&t))
            .child(text(theme::CAPTION).text_color(t.muted).child(tr(telemetry::NEVER)))
            .child(
                // Same keyboard reach as the switches: the full wording stays one Tab away.
                keyboard(
                    div()
                        .id("ask-view")
                        .text_size(px(theme::UI.px))
                        .text_color(t.accent)
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.8))
                        .child(tr(if viewing {
                            "Hide details and example reports"
                        } else {
                            "View details and example reports"
                        })),
                    &t,
                )
                .track_focus(&self.consent_keys.details)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.toggle_view();
                    if this.collection.view.is_some() {
                        // Opening grows the body far past the fold. The pending
                        // scroll_to_top_of_item request is what lays the grown region out
                        // again, and it lands the reader on the details' first line rather
                        // than the JSON plate at the foot; Down reads on from there.
                        this.collection.body_to_item_top(ASK_CHILD_JSON);
                    } else {
                        // Closing shrinks the body back; keep the row itself in view.
                        this.collection.body_reveal(ASK_CHILD_DETAILS);
                    }
                    cx.notify();
                })),
            )
            .children(self.collection.view.as_ref().map(|v| json_view(v, &t, &self.collection.json_scroll)));
        let answers = div()
            .flex()
            .flex_none()
            .justify_end()
            .gap(px(12.))
            .px(px(28.))
            .pt(px(16.))
            .pb(px(28.))
            .border_t_1()
            .border_color(t.rule)
            .children(horizon_telemetry::live_endpoint().is_some().then(|| {
                div().flex_1().min_w_0().flex().items_center().child(
                    text(theme::CAPTION)
                        .text_color(t.muted)
                        .child(tr("The anonymous player-count ping stays on.")),
                )
            }))
            .child(
                button("ask-none", "Keep optional data off", Kind::Secondary, &t)
                    .track_focus(&self.consent_keys.keep_off)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.answer_consent(Choices::NONE);
                        // Everything optional means everything: basic summaries too. The
                        // "I'm playing" ping is not optional data and is untouched.
                        this.basic_off();
                        cx.notify();
                    })),
            )
            .child(
                button("ask-save", "Save choices", Kind::Primary, &t)
                    .track_focus(&self.consent_keys.save)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.answer_consent(this.collection.draft);
                        cx.notify();
                    })),
            );
        let card = plate(&t)
            .flex()
            .flex_col()
            .w_full()
            .max_w(px(680.))
            .max_h_full()
            .bg(t.page)
            .border_1()
            .border_color(t.rule)
            .child(self.collection.body_scroll.wrap(body, &t))
            // A failed save says so here, in the always-visible foot of the question.
            .children(self.collection.note.clone().map(|n| {
                div()
                    .px(px(16.))
                    .pb(px(4.))
                    .child(text(theme::CAPTION).text_color(t.warn).child(n))
            }))
            .child(answers);
        div()
            .id("consent")
            // Focused while the question asks; the window's Tab handling cycles inside it.
            .track_focus(&self.consent_keys.container)
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .px(px(32.))
            .py(px(24.))
            .bg(t.page.opacity(0.92))
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .occlude()
            .child(card)
    }

    /// Settings' data collection: a switch per kind, then the player's data.
    pub(crate) fn collection_section(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let consent = self.collection.consent.clone();
        let mut list = div().flex().flex_col().border_t_1().border_color(t.rule);
        let basic = index_row(
            0,
            tr(telemetry::BASIC_NAME),
            Some(div().child(tr(telemetry::BASIC_WHAT)).into_any_element()),
            Some(self.basic_switch("data-basic", None, cx).into_any_element()),
            &t,
        );
        list = list.child(basic);
        for (i, &(kind, name, what)) in telemetry::SWITCHES.iter().enumerate() {
            let on = !consent.needs_asking() && consent.choices.allows(kind);
            let toggle = switch(kind_id("data", kind), on, &t).on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    let mut choices = if this.collection.consent.needs_asking() {
                        Choices::NONE
                    } else {
                        this.collection.consent.choices
                    };
                    choices.set(kind, !on);
                    this.answer_consent(choices);
                    cx.notify();
                },
            ));
            list = list.child(index_row(
                i + 1,
                tr(name),
                Some(div().child(tr(what)).into_any_element()),
                Some(toggle.into_any_element()),
                &t,
            ));
        }
        list = list.child(index_row(
            telemetry::SWITCHES.len() + 1,
            tr(telemetry::PING_NAME),
            Some(div().child(tr(telemetry::ping_text())).into_any_element()),
            Some(ping_state(&t).into_any_element()),
            &t,
        ));
        let waiting = horizon_telemetry::outbox::pending(&telemetry::home()).len();
        let id = match consent.install_id.as_str() {
            "" => tr("No optional-data install ID is recorded").to_string(),
            id => trf(
                "Install ID {prefix}…{suffix}",
                &[("prefix", &id[..4]), ("suffix", &id[id.len() - 4..])],
            ),
        };
        let server = if horizon_telemetry::endpoint().is_some() {
            trf(
                "{count} waiting to be sent",
                &[("count", &waiting.to_string())],
            )
        } else {
            tr("Optional uploads are unavailable: no upload server is configured. The player-count ping is separate.").into()
        };
        let confirm = self.collection.confirm_delete;
        let actions = div()
            .flex()
            .gap(px(8.))
            .child(
                button(
                    "data-view",
                    if self.collection.view.is_some() {
                        "Hide"
                    } else {
                        "View details and example reports"
                    },
                    Kind::Ghost,
                    &t,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.toggle_view();
                    cx.notify();
                })),
            )
            .child(
                button(
                    "data-delete",
                    if confirm {
                        "Delete everything?"
                    } else {
                        "Delete my data"
                    },
                    Kind::Secondary,
                    &t,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if this.collection.confirm_delete {
                        this.collection.note = Some(match telemetry::delete_my_data() {
                            Ok(done) => done,
                            Err(e) => e,
                        });
                        this.collection.consent = telemetry::consent();
                        this.collection.view = None;
                    }
                    this.collection.confirm_delete = !this.collection.confirm_delete;
                    cx.notify();
                })),
            );
        list = list.child(index_row(
            telemetry::SWITCHES.len() + 2,
            tr("Your data"),
            Some(
                div()
                    .font_family(theme::MONO)
                    .child(format!("{id}  ·  {server}"))
                    .into_any_element(),
            ),
            Some(actions.into_any_element()),
            &t,
        ));
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                text(theme::HEADING)
                    .text_color(t.ink)
                    .child(tr("Data collection")),
            )
            .child(
                text(theme::UI)
                    .max_w(px(600.))
                    .text_color(t.muted)
                    .child(tr(
                        "Basic performance summaries are on by default on a new install; every other switch is off unless you turn it on. What each sends is below it; the privacy page on playhorizon.dev says how long it is kept.",
                    )),
            )
            .child(list)
            .child(
                text(theme::CAPTION)
                    .max_w(px(600.))
                    .text_color(t.muted)
                    .child(tr(telemetry::NEVER)),
            )
            .children(
                self.collection
                    .note
                    .clone()
                    .map(|n| text(theme::UI).text_color(t.ink).child(n)),
            )
            .children(
                self.collection
                    .view
                    .as_ref()
                    .map(|v| json_view(v, &t, &self.collection.json_scroll)),
            )
    }
}
