//! Matching Hunk menu bar, dropdowns, overview, and modal frames.

use crossterm::event::KeyCode;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Borders, Padding, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use super::Window;

use crate::{
    app::App,
    theme::{clipped, ACCENT, GREEN, MUTED, PANEL, PANEL_ALT, SEL_BG, TEXT},
};

pub const MENUS: [&str; 6] = ["File", "View", "Navigate", "Agent", "Extensions", "Help"];

pub struct MenuEntry {
    pub label: String,
    pub hint: &'static str,
    pub key: Option<KeyCode>,
}

pub fn menu_entries(app: &App, menu: usize) -> Vec<MenuEntry> {
    let item = |label: &str, hint, key| MenuEntry {
        label: label.into(),
        hint,
        key: Some(key),
    };
    let toggle = |label: &str, checked, hint, key| {
        item(
            &format!("[{}] {label}", if checked { "x" } else { " " }),
            hint,
            key,
        )
    };
    let note = |label: &str| MenuEntry {
        label: label.into(),
        hint: "",
        key: None,
    };
    match menu {
        0 => vec![
            item("Open changed file", "o", KeyCode::Char('o')),
            item("Focus filter", "/", KeyCode::Char('/')),
            item("Reload", "r", KeyCode::Char('r')),
            item("Quit", "q", KeyCode::Char('q')),
        ],
        1 => vec![
            toggle(
                "Split",
                app.side_by_side && !app.auto_layout,
                "1",
                KeyCode::Char('1'),
            ),
            toggle("Stack", !app.side_by_side, "2", KeyCode::Char('2')),
            toggle("Auto", app.auto_layout, "0", KeyCode::Char('0')),
            toggle("Sidebar", app.show_sidebar, "s", KeyCode::Char('s')),
            toggle("Line numbers", app.line_numbers, "l", KeyCode::Char('l')),
            toggle("Wrap lines", app.wrap_lines, "w", KeyCode::Char('w')),
            toggle("Hunk metadata", app.hunk_headers, "m", KeyCode::Char('m')),
            toggle(
                "Unchanged context",
                app.selected_gap()
                    .is_some_and(|id| app.expanded_gaps.contains(&id)),
                "z",
                KeyCode::Char('z'),
            ),
            toggle("Menu bar", app.show_menu_bar, "M", KeyCode::Char('M')),
            item("Maximize window one level", "+", KeyCode::Char('+')),
            item("Restore window one level", "-", KeyCode::Char('-')),
            note(&format!("Window zoom: level {}", app.zoom.level())),
            note("Theme: Catppuccin Mocha"),
        ],
        2 => vec![
            item("Next changed file", "n", KeyCode::Char('n')),
            item("Previous changed file", "p", KeyCode::Char('p')),
            item("Next hunk", "]", KeyCode::Char(']')),
            item("Previous hunk", "[", KeyCode::Char('[')),
            item("Jump to start", "g", KeyCode::Char('g')),
            item("Jump to end", "G", KeyCode::Char('G')),
        ],
        3 => vec![
            note("No agent notes in this review"),
            note("Freezy is a read-only Git viewer"),
        ],
        4 => vec![
            note("Built-in: workspace files"),
            note("Built-in: changes overview"),
        ],
        _ => vec![
            item("Controls help", "?", KeyCode::Char('?')),
            note("freezy · read-only Git review"),
        ],
    }
}

pub fn render_titlebar(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let area = Window::default().background(PANEL_ALT).render(frame, area);
    let mut x = area.x;
    for (i, label) in MENUS.iter().enumerate() {
        let width = (label.len() as u16 + 2).min(area.right().saturating_sub(x));
        let rect = Rect::new(x, area.y, width, area.height);
        app.layout.menus.push(rect);
        let active = app.menu == Some(i);
        frame.render_widget(
            Paragraph::new(format!(" {label} ")).style(
                Style::default()
                    .fg(if active { TEXT } else { MUTED })
                    .bg(if active { SEL_BG } else { PANEL_ALT }),
            ),
            rect,
        );
        x += width;
    }
    let selected = app.files.iter().find(|f| f.path == app.diff_title);
    let title = selected
        .map(|f| {
            if app.loading_diff {
                return format!("{}  Loading changes…", f.rel);
            }
            let (adds, dels) = app.diff_lines.iter().fold((0, 0), |(a, d), l| {
                (
                    a + usize::from(l.kind == crate::model::DKind::Add),
                    d + usize::from(l.kind == crate::model::DKind::Del),
                )
            });
            format!("{}  1 file  +{adds}  -{dels}", f.rel)
        })
        .unwrap_or_else(|| format!("{}  0 files", app.root_name));
    let title = if app.status.is_empty() {
        title
    } else {
        app.status.clone()
    };
    let remaining = Rect::new(x, area.y, area.right().saturating_sub(x), area.height);
    frame.render_widget(
        Paragraph::new(clipped(&title, remaining.width.saturating_sub(2) as usize))
            .alignment(Alignment::Center)
            .style(Style::default().fg(MUTED).bg(PANEL_ALT)),
        remaining,
    );
}

pub fn render_ruler(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let area = Window::default().render(frame, area);
    for (y, hunk) in app
        .ruler_marks(area.height as usize)
        .into_iter()
        .enumerate()
    {
        if let Some(hunk) = hunk {
            Window::default()
                .background(if hunk == app.hunk_idx { ACCENT } else { GREEN })
                .render(frame, Rect::new(area.x, area.y + y as u16, area.width, 1));
        }
    }
}

fn modal(frame: &mut ratatui::Frame, area: Rect, width: u16, height: u16, title: &str) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    let rect = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let inside = Window::default()
        .borders(Borders::ALL)
        .padding(Padding::vertical(1))
        .overlay()
        .render(frame, rect);
    let close_width = 7.min(inside.width);
    frame.render_widget(
        Paragraph::new(format!(
            " {}",
            clipped(title, inside.width.saturating_sub(close_width + 1) as usize)
        ))
        .style(Style::default().fg(TEXT)),
        Rect::new(
            inside.x,
            inside.y,
            inside.width.saturating_sub(close_width),
            1.min(inside.height),
        ),
    );
    frame.render_widget(
        Paragraph::new("[Esc] ")
            .alignment(Alignment::Right)
            .style(Style::default().fg(MUTED)),
        Rect::new(
            inside.right().saturating_sub(close_width),
            inside.y,
            close_width,
            1.min(inside.height),
        ),
    );
    Rect::new(
        inside.x,
        inside.y + 2.min(inside.height),
        inside.width,
        inside.height.saturating_sub(2),
    )
}

pub fn render_search_popup(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let body = modal(frame, area, 74, 20, "Open changed file");
    if body.height == 0 {
        return;
    }
    frame.render_widget(
        Paragraph::new(format!(" / {}", app.query)).style(Style::default().fg(TEXT).bg(PANEL_ALT)),
        Rect::new(body.x, body.y, body.width, 1),
    );
    let results = Rect::new(
        body.x,
        body.y + 2.min(body.height),
        body.width,
        body.height.saturating_sub(3),
    );
    app.layout.search_results = results;
    let visible = app.visible();
    let start = app
        .selected
        .saturating_sub(results.height.saturating_sub(1) as usize);
    let lines: Vec<_> = visible
        .iter()
        .skip(start)
        .take(results.height as usize)
        .enumerate()
        .map(|(offset, &i)| {
            let selected = start + offset == app.selected;
            Line::styled(
                format!(
                    " {} {}",
                    if selected { "›" } else { " " },
                    clipped(&app.files[i].path, results.width.saturating_sub(3) as usize)
                ),
                Style::default()
                    .fg(if selected { TEXT } else { MUTED })
                    .bg(if selected { SEL_BG } else { PANEL }),
            )
        })
        .collect();
    frame.render_widget(
        Paragraph::new(if lines.is_empty() {
            vec![Line::styled(
                " No matching files",
                Style::default().fg(MUTED),
            )]
        } else {
            lines
        }),
        results,
    );
    frame.render_widget(
        Paragraph::new(format!(
            " {} files  ·  ↑/↓ select  ·  Enter open",
            visible.len()
        ))
        .style(Style::default().fg(MUTED)),
        Rect::new(body.x, body.bottom().saturating_sub(1), body.width, 1),
    );
    let cursor_x = body.x + 3 + app.query.width() as u16;
    if cursor_x < body.right() {
        frame.set_cursor_position((cursor_x, body.y));
    }
}

pub fn render_help_overlay(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let body = modal(frame, area, 74, 37, "Controls help");
    let rows = [
        ("Navigation", ""),
        ("Up / Down · j / k", "move line-by-line"),
        ("PageDown / Space / f", "page down"),
        ("PageUp / b", "page up"),
        ("d / u", "half page down / up"),
        ("[ / ]", "previous / next hunk"),
        ("p / n · , / .", "previous / next file"),
        ("Left / Right", "scroll code sideways"),
        ("g / Home", "jump to start"),
        ("G / End", "jump to end"),
        ("", ""),
        ("Mouse", ""),
        ("Wheel", "scroll vertically"),
        ("Shift+Wheel", "scroll code horizontally"),
        ("Click", "select files, menus, or changes"),
        ("", ""),
        ("View", ""),
        ("+ / -", "maximize / restore one level"),
        ("Hover / click / F6", "choose window to maximize"),
        ("1 / 2 / 0", "split / stack / auto"),
        ("s", "toggle sidebar"),
        ("z / click gap", "expand / collapse unchanged lines"),
        ("l / w / m / M", "lines / wrap / metadata / menu"),
        ("v", "toggle split / stack"),
        ("", ""),
        ("Review", ""),
        ("/ / o / Tab", "open changed file / filter"),
        ("F10", "open menus"),
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
                        .fg(if desc.is_empty() { TEXT } else { MUTED })
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

pub fn render_menu(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let Some(index) = app.menu else {
        return;
    };
    let entries = menu_entries(app, index);
    let width = entries
        .iter()
        .map(|e| e.label.width() + e.hint.width() + 6)
        .max()
        .unwrap_or(20)
        .max(28)
        .min(area.width as usize) as u16;
    let left = app
        .layout
        .menus
        .get(index)
        .map_or(1, |r| r.x)
        .min(area.right().saturating_sub(width));
    let rect = Rect::new(
        left,
        area.y + 1.min(area.height),
        width,
        (entries.len() as u16 + 2).min(area.height.saturating_sub(1)),
    );
    app.layout.menu_items = Window::default()
        .borders(Borders::ALL)
        .overlay()
        .render(frame, rect);
    let lines = entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let label = clipped(
                &e.label,
                width.saturating_sub(e.hint.len() as u16 + 5) as usize,
            );
            let pad =
                " ".repeat((width as usize).saturating_sub(label.width() + e.hint.width() + 4));
            Line::styled(
                format!(" {label}{pad}{} ", e.hint),
                Style::default()
                    .fg(if e.key.is_some() { TEXT } else { MUTED })
                    .bg(if i == app.menu_row && e.key.is_some() {
                        SEL_BG
                    } else {
                        PANEL
                    }),
            )
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), app.layout.menu_items);
}
