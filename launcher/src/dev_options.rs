//! Developer options: a Settings section nothing in the window points to. Seven clicks on the
//! sidebar's "Launcher x.y.z" reveal it for this run; it stays shown while one of its options is
//! on. Its one option lets offline names play without an account that owns the game
//! (`backend::Settings::dev_offline`). The game still comes from the player's own install: the
//! launcher downloads Mojang's files only for an owner (`backend::download_for_owner`).
use super::*;

/// Clicks on the launcher's version that reveal the section.
const CLICKS: u8 = 7;

impl Launcher {
    /// A click on the sidebar's launcher version; the last one opens Settings on the section.
    pub(super) fn version_clicked(&mut self) {
        if self.dev_options {
            return;
        }
        self.dev_clicks += 1;
        if self.dev_clicks >= CLICKS {
            self.dev_options = true;
            self.go(Page::Settings);
        }
    }

    /// The section, when revealed or when one of its options is on.
    pub(super) fn dev_options_section(&self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.dev_options && !self.settings.dev_offline {
            return None;
        }
        let t = self.theme;
        let toggle = scaled_switch("dev-offline", self.settings.dev_offline, &t).on_click(
            cx.listener(|this, _: &ClickEvent, _, cx| {
                let previous = this.settings.dev_offline;
                this.settings.dev_offline = !previous;
                this.dev_offline_save_error = this.settings.save().err().map(|e| e.to_string());
                if this.dev_offline_save_error.is_some() {
                    this.settings.dev_offline = previous;
                }
                cx.notify();
            }),
        );
        Some(
            div()
                .flex()
                .flex_col()
                .child(
                    section_title("developer options", "terminal", &t)
                        .mt(px(theme::chrome(20.)))
                        .mb(px(theme::chrome(12.))),
                )
                .child(index_row(
                    0,
                    tr("offline names without an account"),
                    Some(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(theme::chrome(6.)))
                            .child(tr(
                                "offline names play without a microsoft account that owns the game. minecraft still comes from your own install.",
                            ))
                            .children(self.dev_offline_save_error.clone().map(|e| div().text_color(t.danger).child(e)))
                            .into_any_element(),
                    ),
                    Some(toggle.into_any_element()),
                    &t,
                )),
        )
    }
}
