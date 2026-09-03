//! Scrollable controls-help overlay.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::modal;

use crate::{
    app::App,
    utils::theme::{ACCENT, BLUE, TEXT},
};

pub fn render_help_overlay(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let body = modal(frame, area, 74, 37, "Controls help", ACCENT);
    let rows = [
        ("Navigation", ""),
        ("1 / 2 / 3 / 4", "Files / Commits / Branch / Stash"),
        ("Up / Down · j / k", "move active file / code selection"),
        ("PageDown / Space / f", "page down"),
        ("PageUp / b", "page up"),
        ("d / u", "half page down / up"),
        ("[ / ]", "previous / next hunk"),
        ("Left / Right · h / l", "scroll active viewer sideways"),
        ("g / Home", "jump to start"),
        ("G / End", "jump to end"),
        ("", ""),
        ("Mouse", ""),
        ("Wheel", "scroll vertically"),
        ("Shift+Wheel", "scroll code horizontally"),
        ("Click", "select workspaces, files, or changes"),
        ("", ""),
        ("View", ""),
        ("+ / -", "maximize / restore one level"),
        ("Click / F6", "choose window to maximize"),
        ("Ctrl-h/j/k/l", "focus window left/down/up/right"),
        ("< / > / 0", "stack / split / auto"),
        ("s", "toggle sidebar"),
        ("z / click gap", "expand / collapse unchanged lines"),
        ("Shift-L / w / m", "lines / wrap / metadata"),
        ("v", "toggle split / stack"),
        ("", ""),
        ("Review", ""),
        ("/ / o / Tab", "open changed file / filter"),
        ("r", "reload the review"),
        ("q / Ctrl-C", "quit"),
    ];
    let lines = rows
        .into_iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled(
                    format!(" {key:<25}"),
                    Style::default()
                        .fg(if desc.is_empty() { ACCENT } else { BLUE })
                        .add_modifier(if desc.is_empty() {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::styled(desc, Style::default().fg(TEXT)),
            ])
        })
        .collect::<Vec<_>>();
    app.help_scroll = app
        .help_scroll
        .min((lines.len() as u16).saturating_sub(body.height));
    frame.render_widget(Paragraph::new(lines).scroll((app.help_scroll, 0)), body);
}
