//! The existing Source Control list in a terminal-native window frame.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Borders, List, ListItem, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::ui::primitives::window::{Window, SIDEBAR};

use crate::{
    app::App,
    core::model::{kind_color, stats_label},
    utils::theme::{clipped, ACCENT, MUTED, PANEL, SEL_BG, SEL_FG, TEXT},
};

pub fn render_sidebar(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let focused = app.zoom.focused == SIDEBAR && !app.searching && !app.show_help;
    let body = Window::default()
        .borders(Borders::ALL)
        .focused(focused)
        .render(frame, area);
    frame.render_widget(
        Paragraph::new(clipped(
            &format!(" SOURCE CONTROL  {}", app.visible().len()),
            body.width
                .saturating_sub(if area.width >= 8 { 6 } else { 0 }) as usize,
        ))
        .style(
            Style::default()
                .fg(if focused { ACCENT } else { TEXT })
                .bg(PANEL)
                .add_modifier(Modifier::BOLD),
        ),
        Rect::new(body.x, area.y, body.width, 1.min(area.height)),
    );
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
                    .fg(if selected && focused {
                        SEL_FG
                    } else {
                        kind_color(file.kind)
                    })
                    .bg(if selected && focused { SEL_BG } else { PANEL })
                    .add_modifier(if selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
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
    Window::controls(frame, &mut app.zoom, SIDEBAR, SIDEBAR, area);
}
