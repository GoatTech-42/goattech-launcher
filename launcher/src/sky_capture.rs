//! SCRATCH ONLY: real Mono sky renderer in an isolated, input-free fixture.
use super::*;

pub(super) fn wanted() -> bool {
    std::env::var("HORIZON_MONO_SKY_CAPTURE").as_deref() == Ok("mono")
}

impl Launcher {
    pub(super) fn mono_sky_capture(&mut self, window: &Window) -> Div {
        let n = theme::night::NIGHT_MONO;
        let levels = self.sky.capture_frozen_levels();
        if let Ok(cap) = std::env::var("CAP") {
            let path = std::path::PathBuf::from(cap).join("brightness-state.txt");
            if !path.exists() {
                let _ = std::fs::write(
                    path,
                    format!("star-clock=0; motion=off; actual-levels={levels:?}\n"),
                );
            }
        }
        let (sky, _) = self.sky.render(
            &n,
            (self.viewport_w, self.viewport_h),
            window.scale_factor(),
            false,
            Instant::now(),
        );
        div()
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(shell::c(n.page))
            .child(sky)
            .child(
                div()
                    .absolute()
                    .bottom(px(16.))
                    .left(px(24.))
                    .font_family(theme::FONT)
                    .text_color(shell::c(n.ink))
                    .child(
                        "SYNTHETIC MONO SKY P41. Real sky renderer; no controls, game or account.",
                    ),
            )
    }
}
