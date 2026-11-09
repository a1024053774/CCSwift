//! CCSwift’s design tokens, serif fonts, orange glass surfaces, and runtime Claude mark.

use gpui_kit::component::{ActiveTheme as _, Theme, ThemeMode, ThemeSet};
use gpui_kit::{Anchor, App, BoxShadow, Hsla, Window, point, px};
use std::rc::Rc;

const THEMES: &str = include_str!("../assets/theme.json");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub const ALL: [ThemeChoice; 3] = [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark];

    pub fn label(self) -> String {
        crate::i18n::t(match self {
            ThemeChoice::System => "theme.system",
            ThemeChoice::Light => "theme.light",
            ThemeChoice::Dark => "theme.dark",
        })
    }

    /// The `appearance` value in `config.json`.
    pub fn from_config(value: &str) -> ThemeChoice {
        match value {
            "light" => ThemeChoice::Light,
            "dark" => ThemeChoice::Dark,
            _ => ThemeChoice::System,
        }
    }

    pub fn config_value(self) -> &'static str {
        match self {
            ThemeChoice::System => "system",
            ThemeChoice::Light => "light",
            ThemeChoice::Dark => "dark",
        }
    }

    fn mode(self, window: &Window) -> ThemeMode {
        match self {
            ThemeChoice::System => window.appearance().into(),
            ThemeChoice::Light => ThemeMode::Light,
            ThemeChoice::Dark => ThemeMode::Dark,
        }
    }
}

/// Registers the CC Same themes as GPUI Kit's light and dark themes and loads the one that
/// matches the system.
/// Height of the window's title row; the traffic lights and sheets are placed against it.
pub const TITLE_ROW: gpui_kit::Pixels = px(52.);

pub fn install(cx: &mut App) {
    let set: ThemeSet = serde_json::from_str(THEMES).expect("the bundled theme is valid");
    let pick = |dark: bool| {
        let config = set.themes.iter().find(|t| t.mode.is_dark() == dark).cloned();
        Rc::new(config.expect("the bundled theme has a light and a dark variant"))
    };
    let (light, dark) = (pick(false), pick(true));
    Theme::update(cx, |theme| {
        theme.light_theme = light;
        theme.dark_theme = dark;
        theme.notification.placement = Anchor::BottomCenter;
        theme.notification.width = px(340.);
        // Sheets open below our title row, not the kit's 34pt default.
        theme.sheet.margin_top = TITLE_ROW;
    });
    let mode: ThemeMode = cx.window_appearance().into();
    Theme::change(mode, None, cx);
}

/// Follows the chosen appearance; called on every render, so a system switch is picked up.
pub fn follow(choice: ThemeChoice, window: &mut Window, cx: &mut App) {
    let mode = choice.mode(window);
    if cx.theme().mode != mode {
        Theme::change(mode, Some(window), cx);
    }
    if cx.theme().font_family.as_ref() != font() {
        Theme::global_mut(cx).font_family = font().into();
    }
}

/// The soft lift under a card: a contact shadow and a wide, faint one. Dark surfaces get
/// their separation from the border alone.
pub fn card_shadow(cx: &App) -> Vec<BoxShadow> {
    let theme = cx.theme();
    if theme.is_dark() {
        return Vec::new();
    }
    let ink = theme.foreground;
    vec![
        BoxShadow {
            color: ink.opacity(0.035),
            offset: point(px(0.), px(1.)),
            blur_radius: px(2.),
            spread_radius: px(0.),
            inset: false,
        },
        BoxShadow {
            color: ink.opacity(0.04),
            offset: point(px(0.), px(6.)),
            blur_radius: px(18.),
            spread_radius: px(-4.),
            inset: false,
        },
    ]
}

/// Every dialog's title: one size and the design's medium weight (the kit's default is body-size semibold).
pub fn dialog_title(text: impl Into<gpui_kit::SharedString>) -> gpui_kit::Div {
    use gpui_kit::{ParentElement as _, Styled as _};
    gpui_kit::div().font_weight(gpui_kit::FontWeight::MEDIUM).text_size(px(18.)).child(text.into())
}

pub fn font() -> &'static str {
    if crate::i18n::current() == "zh-CN" { "Songti SC" } else { "Times New Roman" }
}
pub fn accent_text(cx: &App) -> Hsla {
    gpui_kit::rgb(if cx.theme().is_dark() { 0xE8956F } else { 0xB5532F }).into()
}
pub fn empty(cx: &App) -> Hsla {
    gpui_kit::rgb(if cx.theme().is_dark() { 0x2A2724 } else { 0xE7E4DF }).into()
}
pub fn bar(cx: &App) -> Hsla {
    gpui_kit::rgb(if cx.theme().is_dark() { 0xBDB6AC } else { 0x3A3632 }).into()
}
pub fn jelly() -> gpui_kit::Background {
    gpui_kit::linear_gradient(
        180.,
        gpui_kit::linear_color_stop(gpui_kit::rgb(0xDE7854).opacity(0.86), 0.),
        gpui_kit::linear_color_stop(gpui_kit::rgb(0xC45C38).opacity(0.94), 1.),
    )
}
pub fn jelly_shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: gpui_kit::white().opacity(0.55),
            offset: point(px(0.), px(1.)),
            blur_radius: px(0.),
            spread_radius: px(0.),
            inset: true,
        },
        BoxShadow {
            color: gpui_kit::rgb(0xD97757).opacity(0.3).into(),
            offset: point(px(0.), px(10.)),
            blur_radius: px(26.),
            spread_radius: px(0.),
            inset: false,
        },
    ]
}

/// Runtime-only asset: preserve the installed mark's alpha and tint its pixels orange (BGRA).
/// Missing or unreadable assets omit the optional mark; no copy is written to disk.
pub(crate) fn load_mark(resources: &std::path::Path) -> Option<std::sync::Arc<gpui_kit::RenderImage>> {
    ["TrayIconTemplate@2x.png", "TrayIconTemplate@3x.png"].iter().find_map(|name| {
        let mut pixels = image::open(resources.join(name)).ok()?.to_rgba8();
        for pixel in pixels.pixels_mut() {
            pixel.0 = [0x57, 0x77, 0xD9, pixel.0[3]];
        }
        Some(std::sync::Arc::new(gpui_kit::RenderImage::new(vec![image::Frame::new(pixels)])))
    })
}

#[cfg(test)]
mod mark_tests {
    #[test]
    fn missing_claude_resources_omit_mark() {
        let dir = tempfile::tempdir().unwrap();
        assert!(super::load_mark(&dir.path().join("missing-Claude.app/Contents/Resources")).is_none());
    }
}
