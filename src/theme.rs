//! Catppuccin Mocha, using the surface colors from the installed Hunk theme.

use ratatui::style::Color;
use unicode_width::UnicodeWidthChar;

pub const BG: Color = Color::Rgb(30, 30, 46);
pub const PANEL: Color = Color::Rgb(44, 45, 62);
pub const PANEL_ALT: Color = Color::Rgb(51, 52, 70);
pub const BORDER: Color = Color::Rgb(62, 63, 82);
pub const MUTED: Color = Color::Rgb(177, 178, 185);
pub const FAINT: Color = Color::Rgb(128, 133, 157);
pub const TEXT: Color = Color::Rgb(205, 214, 244);
pub const ACCENT: Color = Color::Rgb(249, 226, 175);
pub const GREEN: Color = Color::Rgb(166, 227, 161);
pub const RED: Color = Color::Rgb(243, 139, 168);
pub const SEL_BG: Color = Color::Rgb(85, 79, 78);
pub const ADD_BG: Color = Color::Rgb(57, 69, 69);
pub const DEL_BG: Color = Color::Rgb(73, 52, 70);
pub const ADD_EMPH: Color = Color::Rgb(68, 85, 78);
pub const DEL_EMPH: Color = Color::Rgb(90, 61, 80);

/// Clip terminal cells, not bytes or characters. Never split a wide character.
pub fn clipped(text: &str, width: usize) -> String {
    let clean: String = text
        .chars()
        .map(|c| if c.is_control() { '�' } else { c })
        .collect();
    let text = clean.as_str();
    if unicode_width::UnicodeWidthStr::width(text) <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut used = 0;
    text.chars()
        .take_while(|c| {
            used += c.width().unwrap_or(0);
            used < width
        })
        .chain(std::iter::once('…'))
        .collect()
}

pub fn spinner(frame: u64) -> &'static str {
    const FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
    FRAMES[(frame as usize) % FRAMES.len()]
}
