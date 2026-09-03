//! Hunk workspace extension's compact Source Control pane.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{List, ListItem, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use super::{window::SIDEBAR, Window};

use crate::{
    app::App,
    model::{kind_color, stats_label},
    theme::{clipped, MUTED, PANEL, PANEL_ALT, SEL_BG, TEXT},
};

pub fn render_sidebar(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let area = Window::default().render(frame, area);
    let sections = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(clipped(
            &format!(" SOURCE CONTROL  {}", app.visible().len()),
            area.width
                .saturating_sub(if area.width >= 6 { 6 } else { 0 }) as usize,
        ))
        .style(Style::default().fg(MUTED).bg(PANEL_ALT)),
        sections[0],
    );
    let body = sections[1];
    app.layout.side = body;
    let visible = app.visible();
    let mut items = Vec::with_capacity(visible.len() + 8);
    let mut last_repo = "";
    let width = body.width as usize;
    for (vi, &index) in visible.iter().enumerate() {
        let file = &app.files[index];
        if file.repo != last_repo {
            if !last_repo.is_empty() {
                items.push(ListItem::new(""));
            }
            last_repo = &file.repo;
            items.push(ListItem::new(Line::styled(
                clipped(&format!(" ▾ {}", file.repo), width),
                Style::default().fg(MUTED),
            )));
        }
        let selected = app.selected == vi;
        let stats = stats_label(file);
        let label_width = width.saturating_sub(stats.width() + 5);
        let label = clipped(&file.rel, label_width);
        let padding = " ".repeat(label_width.saturating_sub(label.width()));
        items.push(
            ListItem::new(format!(
                "{} {} {label}{padding} {stats}",
                if selected { "▌" } else { " " },
                file.kind,
            ))
            .style(
                Style::default()
                    .fg(if selected {
                        TEXT
                    } else {
                        kind_color(file.kind)
                    })
                    .bg(if selected { SEL_BG } else { PANEL }),
            ),
        );
    }
    if items.is_empty() {
        items.push(
            ListItem::new(if app.scanning {
                " Scanning repositories…"
            } else if !app.query.is_empty() {
                " No matching files"
            } else {
                " No local changes"
            })
            .style(Style::default().fg(MUTED)),
        );
    }
    let height = body.height as usize;
    let window = app
        .selected_row()
        .unwrap_or(0)
        .saturating_sub(height / 2)
        .min(items.len().saturating_sub(height));
    app.layout.side_win = window;
    frame.render_widget(
        List::new(
            items
                .into_iter()
                .skip(window)
                .take(height)
                .collect::<Vec<_>>(),
        )
        .style(Style::default().bg(PANEL)),
        body,
    );
    frame.render_widget(
        Paragraph::new(if app.query.is_empty() {
            " n/p file  [/] change".into()
        } else {
            format!(" / {}  · Esc clear", app.query)
        })
        .style(Style::default().fg(MUTED).bg(PANEL_ALT)),
        sections[2],
    );
    Window::controls(frame, &mut app.zoom, SIDEBAR, SIDEBAR, area);
}
