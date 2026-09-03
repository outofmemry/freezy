//! Workspace tabs, overview, and modal frames.

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Borders, Padding, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use super::Window;

use crate::{
    app::{App, Workspace},
    theme::{clipped, ACCENT, GREEN, MUTED, PANEL, PANEL_ALT, SEL_BG, TEXT},
};

pub fn render_workspace_tabs(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let body = Window::default()
        .borders(Borders::BOTTOM)
        .render(frame, area);
    let tabs = Layout::horizontal([Constraint::Ratio(1, 4); 4]).split(body);
    for (index, (&workspace, &rect)) in Workspace::ALL.iter().zip(tabs.iter()).enumerate() {
        app.layout
            .workspace_tabs
            .push(Rect::new(rect.x, area.y, rect.width, area.height));
        let active = app.workspace == workspace;
        frame.render_widget(
            Paragraph::new(format!("{}. {}", index + 1, workspace.label()))
                .alignment(Alignment::Center)
                .style(
                    Style::default()
                        .fg(if active { ACCENT } else { MUTED })
                        .bg(PANEL)
                        .add_modifier(if active {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
            rect,
        );
        if active && area.height > 0 {
            let y = area.bottom().saturating_sub(1);
            for x in rect.x..rect.right() {
                frame.buffer_mut()[(x, y)]
                    .set_symbol("━")
                    .set_fg(ACCENT)
                    .set_bg(PANEL);
            }
        }
    }
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
        ("1 / 2 / 3 / 4", "Files / Commits / Branch / Stash"),
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
        ("Click", "select workspaces, files, or changes"),
        ("", ""),
        ("View", ""),
        ("+ / -", "maximize / restore one level"),
        ("Hover / click / F6", "choose window to maximize"),
        ("< / > / 0", "stack / split / auto"),
        ("s", "toggle sidebar"),
        ("z / click gap", "expand / collapse unchanged lines"),
        ("l / w / m", "lines / wrap / metadata"),
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
