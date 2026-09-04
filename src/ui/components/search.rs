//! Fuzzy file finder popup.

use ratatui::{layout::Rect, style::Style, text::Line, widgets::Paragraph};
use unicode_width::UnicodeWidthStr;

use super::modal;

use crate::{
    app::App,
    utils::theme::{clipped, BLUE, CYAN, MUTED, PANEL, PANEL_ALT, SEL_BG, SEL_FG, TEXT},
};

pub fn render_search_popup(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let body = modal(frame, area, 74, 20, "Open changed file", CYAN);
    if body.height == 0 {
        app.layout.search_results = Rect::default();
        return;
    }
    frame.render_widget(
        Paragraph::new(format!(" / {}", app.query)).style(Style::default().fg(CYAN).bg(PANEL_ALT)),
        Rect::new(body.x, body.y, body.width, 1),
    );
    // body.y holds query, bottom-1 holds footer; results get the rest.
    let top = body.y.saturating_add(2.min(body.height));
    let height = body.height.saturating_sub(3);
    let results = Rect::new(body.x, top, body.width, height);
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
                    .fg(if selected { SEL_FG } else { TEXT })
                    .bg(if selected { SEL_BG } else { PANEL }),
            )
        })
        .collect();
    frame.render_widget(
        // Empty only when no matches, not when popup too short to show any.
        Paragraph::new(if visible.is_empty() {
            vec![Line::styled(
                " No matching files",
                Style::default().fg(MUTED),
            )]
        } else {
            lines
        }),
        results,
    );
    if body.height < 2 {
        // No room for a footer — don't paint it over the query line.
        return;
    }
    frame.render_widget(
        Paragraph::new(format!(
            " {} files  ·  ↑/↓ select  ·  Enter open",
            visible.len()
        ))
        .style(Style::default().fg(BLUE)),
        Rect::new(body.x, body.bottom().saturating_sub(1), body.width, 1),
    );
    let cursor_x = body.x + 3 + app.query.width() as u16;
    if cursor_x < body.right() {
        frame.set_cursor_position((cursor_x, body.y));
    }
}
