//! SCRATCH ONLY. Synthetic font routes, no input handlers or network-backed page loads.
use super::*;
use std::sync::OnceLock;

pub(super) fn mode() -> Option<&'static str> {
    static MODE: OnceLock<Option<String>> = OnceLock::new();
    MODE.get_or_init(|| {
        std::env::var("HORIZON_FONT_CAPTURE")
            .ok()
            .filter(|s| matches!(s.as_str(), "mods-en" | "labels-fr" | "flynn"))
    })
    .as_deref()
}

impl Launcher {
    pub(super) fn font_capture_view(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let mode = mode()?;
        let t = self.theme;
        i18n::set(if mode == "labels-fr" {
            i18n::Language::French
        } else {
            i18n::Language::English
        });
        let mut body = div().flex().flex_col().gap(px(16.)).p(px(24.)).child(
            text(theme::LABEL)
                .child("SYNTHETIC FONT CAPTURE, NOT GAMEPLAY. Build fixture-font-37."),
        );
        if mode == "mods-en" || mode == "labels-fr" {
            body = body
                .child(page_header(
                    "mods",
                    "synthetic offline filters, no source requests",
                    &t,
                ))
                .child(self.mods_filters(cx));
        }
        if mode == "labels-fr" {
            // Same translated pixel button path used by real controls, inert in this fixture.
            for (i, label) in ["show in folder", "hard", "slim arms"]
                .into_iter()
                .enumerate()
            {
                body = body.child(button(
                    format!("font-label-{i}"),
                    label,
                    Kind::Secondary,
                    &t,
                ));
            }
        }
        if mode == "flynn" {
            self.accounts.accounts.clear();
            self.accounts.offline = vec!["flynn".into()];
            self.accounts.offline_selected = Some("flynn".into());
            self.offline_name = Some("flynn".into());
            body = body.child(text(theme::TITLE).font_family(theme::PIXEL).child("flynn"));
            for value in ["Horizon", "Henry", "GOATTECH HOME"] {
                body = body.child(text(theme::TITLE).font_family(theme::PIXEL).child(value));
            }
            body = body.child(self.head("00000000-0000-4000-8000-000000000037", 40.));
            body = body.child(self.home_stage(cx));
        }
        Some(
            div()
                .id("font-capture-root")
                .size_full()
                .bg(t.page)
                .text_color(t.ink)
                .overflow_y_scroll()
                .child(body),
        )
    }
}
