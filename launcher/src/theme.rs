//! The launcher's view of Horizon's design tokens (`crates/horizon-theme`, described in
//! `docs/design-system.md`): the night look (`horizon_theme::night`, a night sky or a dawn with
//! Horizon's violet) in Bricolage Grotesque, with Geist Mono for paths and codes (all bundled, SIL
//! Open Font License). Screens not rebuilt yet draw the Blocks parts in the night's colours.
use gpui::{rgb, Hsla, WindowAppearance};
use horizon_launcher::backend::Appearance;
use horizon_launcher::themes::Accent;
use horizon_theme as tokens;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

pub use tokens::night::{self, Night};
pub use tokens::{Step, CAPTION, DISPLAY, HEADING, LABEL, MICRO, TITLE, UI, WORDMARK};
pub const FONT: &str = night::FONT;
pub const DISPLAY_FONT: &str = night::DISPLAY_FONT;
pub const MONO: &str = tokens::MONO;
/// The pixel face for headings, navigation and buttons (the concept's look): Pixelify Sans,
/// bundled and registered in `main`.
pub const PIXEL: &str = tokens::PIXEL;

pub use tokens::{BLOCK_BEVEL as BEVEL, BLOCK_LIP as LIP};
/// Shapes and controls, shared with the game's menus and the website.
pub use tokens::{CONTROL_LARGE, DISABLED, FIELD, PRESSED, RISE, ROW, SEGMENT};

/// Blocks have square corners: these stay for the places that ask for a radius.
pub const RADIUS: f32 = 0.;
pub const RADIUS_PLATE: f32 = 0.;
/// Buttons are a little taller than the quiet look's, for their lip.
pub const CONTROL: f32 = 34.;
/// The top bar, and the sidebar of instances and servers.
pub const TOPBAR: f32 = 56.;
pub const SIDEBAR: f32 = 76.;
pub const PAGE_GUTTER: f32 = 32.;

/// One continuous UI scale from the window size: on a big monitor the fixed 14 px steps read
/// small, so type and chrome grow together. The driver is the smaller of the width and height
/// ratios against 1028x780, so a wide-but-short or tall-but-narrow window does not grow.
/// Exactly 1.0 at and below 1028x780 - every window the capture harness can reach stays
/// pixel-identical - rising linearly to 1.2 at 2560x1440 and clamped there. Continuous
/// everywhere; the only slope changes are at the clamps, like `shell::compact_v` and the
/// home stage's texel. Never scales below 1.0: small windows keep their exact layout.
pub fn ui_scale(viewport_w: f32, viewport_h: f32) -> f32 {
    /// The window the scale reaches 1.2 at.
    const MAX_W: f32 = 2560.;
    const MAX_H: f32 = 1440.;
    let d = (viewport_w / 1028.).min(viewport_h / 780.);
    let d_max = (MAX_W / 1028.).min(MAX_H / 780.);
    (1. + 0.2 * (d - 1.) / (d_max - 1.)).clamp(1., 1.2)
}

/// The scale for the frame being drawn, set once per render from the window size
/// (`set_ui_scale`). Ambient like the current language (`i18n`): text runs and chrome read
/// it through the accessors below instead of threading it through hundreds of call sites.
/// 1.0 until the first render sets it, so tests and any pre-render build see the base sizes.
static UI_SCALE: AtomicU32 = AtomicU32::new(1.0f32.to_bits());

/// Records the window's scale for the frame about to be drawn.
pub fn set_ui_scale(viewport_w: f32, viewport_h: f32) {
    UI_SCALE.store(
        ui_scale(viewport_w, viewport_h).to_bits(),
        Ordering::Relaxed,
    );
}

/// The current frame's scale: 1.0 at and below 1028x780, up to 1.2 at 2560x1440.
pub fn ui_scale_current() -> f32 {
    f32::from_bits(UI_SCALE.load(Ordering::Relaxed))
}

/// A type step's size for the current frame: its own size at and below 1028x780.
pub fn step_px(step: Step) -> f32 {
    step.px * ui_scale_current()
}

/// A type step's line height for the current frame: `(size * line).round()` at the scaled
/// size, identical to `Step::line_px` at scale 1.0.
pub fn step_line_px(step: Step) -> f32 {
    (step.px * ui_scale_current() * step.line).round()
}

/// Motion: short and eased out, never bouncy.
pub const QUICK: Duration = tokens::QUICK.duration();
pub const MOVE: Duration = tokens::MOVE.duration();
pub const ENTER: Duration = tokens::ENTER.duration();
pub const STAGGER: Duration = tokens::STAGGER;

#[derive(Clone, Copy)]
pub struct Theme {
    /// The page behind everything the sky doesn't cover.
    pub page: Hsla,
    /// Hover plates, fields and the console: the raised panel colour.
    pub plate_soft: Hsla,
    /// Pressed plates, a switch that is off: the button colour.
    pub plate: Hsla,
    /// Dialogs and menus above the page: the panel colour.
    pub surface: Hsla,
    pub rule: Hsla,
    pub ink: Hsla,
    pub ink_soft: Hsla,
    pub muted: Hsla,
    pub accent: Hsla,
    pub on_accent: Hsla,
    pub danger: Hsla,
    pub success: Hsla,
    pub warn: Hsla,
    /// The blocky parts (`horizon_theme::Blocks`).
    pub side: Hsla,
    pub panel: Hsla,
    pub raised: Hsla,
    pub button: Hsla,
    pub button_bevel: Hsla,
    pub well: Hsla,
    pub bevel: Hsla,
    pub lip: Hsla,
    pub outline: Hsla,
    pub face: Hsla,
    pub face_bevel: Hsla,
    pub face_lip: Hsla,
    pub on_face: Hsla,
    /// The night look's own tokens (the sky, plates with their opacity), for rebuilt screens.
    pub night: Night,
    pub dark: bool,
}

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// Whether the launcher is pinned dark (`Some(true)`) or light, or follows the system (`None`):
/// `HORIZON_LAUNCHER_APPEARANCE=light` or `dark` (for screenshots and tests), else the player's
/// choice in Settings.
pub fn pinned_dark(choice: Appearance) -> Option<bool> {
    match std::env::var("HORIZON_LAUNCHER_APPEARANCE").as_deref() {
        Ok("light") => Some(false),
        Ok("dark") => Some(true),
        _ => match choice {
            Appearance::Dark => Some(true),
            Appearance::Light => Some(false),
            Appearance::System => None,
        },
    }
}

impl Theme {
    /// The pinned look (`pinned_dark`: dark unless the player picked another), the system's when
    /// they asked to follow it, in the player's accent.
    pub fn for_appearance(choice: Appearance, accent: Accent, system: WindowAppearance) -> Self {
        let system_dark = !matches!(
            system,
            WindowAppearance::Light | WindowAppearance::VibrantLight
        );
        let dark = pinned_dark(choice).unwrap_or(system_dark);
        // GoatTech: the Mono accent takes the true-neutral night with it.
        let night = if dark {
            if matches!(accent, Accent::MONO) {
                night::NIGHT_MONO
            } else {
                night::NIGHT
            }
        } else {
            night::DAWN
        };
        Self::from(accent.apply(night.blocks()), night)
    }

    fn from(p: tokens::Blocks, night: Night) -> Self {
        Self {
            page: c(p.page),
            plate_soft: c(p.raised),
            plate: c(p.button),
            surface: c(p.panel),
            rule: c(p.rule),
            ink: c(p.ink),
            ink_soft: c(p.ink_soft),
            muted: c(p.muted),
            accent: c(p.accent),
            on_accent: c(p.on_face),
            danger: c(p.danger),
            success: c(p.success),
            warn: c(p.warn),
            side: c(p.side),
            panel: c(p.panel),
            raised: c(p.raised),
            button: c(p.button),
            button_bevel: c(p.button_bevel),
            well: c(p.well),
            bevel: c(p.bevel),
            lip: c(p.lip),
            outline: c(p.outline),
            face: c(p.face),
            face_bevel: c(p.face_bevel),
            face_lip: c(p.face_lip),
            on_face: c(p.on_face),
            night,
            dark: p.dark,
        }
    }
}

/// How strongly a selected suggestion in a text field is tinted with the accent.
pub const SELECTION: f32 = 0.22;

#[cfg(test)]
mod tests {
    use super::{tokens, SELECTION};

    /// `top` at `alpha` over `under`, as the GPU blends them.
    fn blend(top: u32, under: u32, alpha: f32) -> u32 {
        let channel = |shift: u32| {
            let (a, b) = ((top >> shift & 0xff) as f32, (under >> shift & 0xff) as f32);
            ((a * alpha + b * (1. - alpha)).round() as u32) << shift
        };
        channel(16) | channel(8) | channel(0)
    }

    /// What the Servers page draws besides text (whose colours horizon-theme's own test holds to
    /// WCAG AA on every background): the keyboard focus ring, the borders of a focused or refused
    /// field (WCAG 1.4.11 asks 3:1 for those), and text over a selected suggestion.
    #[test]
    fn marks_and_selections_read_clearly() {
        for p in [tokens::night::DAWN.blocks(), tokens::night::NIGHT.blocks()] {
            let theme = if p.dark { "dark" } else { "light" };
            for (name, bg) in [
                ("page", p.page),
                ("raised", p.raised),
                ("button", p.button),
                ("panel", p.panel),
                ("well", p.well),
            ] {
                let ring = tokens::contrast(p.accent, bg);
                assert!(ring >= 3.0, "{theme} focus ring on {name}: {ring:.2}");
                let refused = tokens::contrast(p.danger, bg);
                assert!(
                    refused >= 3.0,
                    "{theme} refused field on {name}: {refused:.2}"
                );
            }
            let selection = blend(p.accent, p.panel, SELECTION);
            let ratio = tokens::contrast(p.ink, selection);
            assert!(ratio >= 4.5, "{theme} text on a selection: {ratio:.2}");
        }
    }
}

#[cfg(test)]
mod scale_tests {
    use super::*;

    /// The boundary table the design was reviewed against.
    #[test]
    fn ui_scale_boundary_table() {
        let cases = [
            (960., 600., 1.0),  // the minimum window: unchanged
            (1028., 780., 1.0), // the largest capturable window: unchanged
            (1027., 780., 1.0),
            (1028., 779., 1.0),
            (1000., 1000., 1.0),
            (1440., 900., 1.0363636),
            (1920., 1080., 1.0909091),
            (2560., 1440., 1.2),      // the design's top end
            (3840., 2160., 1.2),      // clamped
            (2560., 800., 1.0060606), // wide but short: the height ratio binds
            (1028., 1440., 1.0),      // tall but narrow: the width ratio binds
            (2560., 780., 1.0),       // wide but short, height exactly at the base
        ];
        for (w, h, want) in cases {
            let got = ui_scale(w, h);
            assert!((got - want).abs() < 1e-4, "{w}x{h}: got {got}, want {want}");
        }
    }

    /// f32-exact 1.0 at and below 1028x780: every window the capture harness can reach is
    /// pixel-identical (multiplication by exactly 1.0 is the identity).
    #[test]
    fn ui_scale_is_exactly_one_at_and_below_the_capture_sizes() {
        for w in [640., 800., 960., 1000., 1028.] {
            for h in [480., 600., 720., 779., 780.] {
                assert_eq!(ui_scale(w, h), 1.0, "{w}x{h}");
            }
        }
        assert_eq!(ui_scale(1028., 780.), 1.0);
    }

    /// Continuous and monotone on a square-window sweep through both clamp points: each 1 px
    /// step moves the scale by no more than the linear slope allows, and it never decreases.
    #[test]
    fn ui_scale_is_continuous_and_monotone() {
        let slope = 0.2 / ((2560f32 / 1028.).min(1440. / 780.) - 1.);
        let mut prev = ui_scale(600., 600.);
        for i in 1..=1600 {
            let x = 600. + i as f32; // 601..=2200, sweeping 1.0 -> linear -> 1.2
            let s = ui_scale(x, x);
            assert!(s >= prev, "decrease at {x}: {prev} -> {s}");
            assert!(
                s - prev <= slope / 780. + 1e-6,
                "jump at {x}: {prev} -> {s}"
            );
            prev = s;
        }
        assert_eq!(prev, 1.2);
    }
}
