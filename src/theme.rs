//! LazyGit-inspired dark palette, independent of the terminal's ANSI theme.

use ratatui::style::Color;
use unicode_width::UnicodeWidthChar;

pub const BG: Color = Color::Rgb(0, 0, 0);
pub const PANEL: Color = BG;
pub const PANEL_ALT: Color = BG;
pub const BORDER: Color = Color::Rgb(192, 192, 192);
pub const MUTED: Color = Color::Rgb(175, 175, 175);
pub const FAINT: Color = Color::Rgb(128, 128, 128);
pub const TEXT: Color = Color::Rgb(215, 215, 215);
pub const ACCENT: Color = GREEN;
pub const GREEN: Color = Color::Rgb(0, 215, 0);
pub const RED: Color = Color::Rgb(255, 95, 95);
pub const YELLOW: Color = Color::Rgb(255, 255, 0);
pub const BLUE: Color = Color::Rgb(95, 175, 255);
pub const CYAN: Color = Color::Rgb(0, 215, 255);
pub const MAGENTA: Color = Color::Rgb(215, 95, 255);
pub const SEL_BG: Color = Color::Rgb(0, 95, 175);
pub const SEL_FG: Color = Color::Rgb(255, 255, 255);
pub const ADD_BG: Color = Color::Rgb(12, 35, 18);
pub const DEL_BG: Color = Color::Rgb(46, 16, 18);
pub const ADD_EMPH: Color = Color::Rgb(22, 66, 32);
pub const DEL_EMPH: Color = Color::Rgb(82, 26, 30);

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
