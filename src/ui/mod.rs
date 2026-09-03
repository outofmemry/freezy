//! Hunk's padded menu bar, workspace pane, review stream, and overview ruler.

pub mod chrome;
pub mod diff;
pub mod sidebar;
pub mod syntax;

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::{Block, Borders},
};

use crate::{
    app::App,
    theme::{BG, BORDER, PANEL},
};

pub fn render(frame: &mut ratatui::Frame, app: &mut App) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(BG)), area);
    app.layout = Default::default();
    let shell = Layout::vertical([
        Constraint::Length(u16::from(app.show_menu_bar)),
        Constraint::Min(0),
    ])
    .split(Rect::new(
        area.x + u16::from(area.width > 0),
        area.y,
        area.width.saturating_sub(2),
        area.height,
    ));
    if app.show_menu_bar {
        chrome::render_titlebar(frame, app, shell[0]);
    }
    let side_width = if app.show_sidebar && area.width >= 60 {
        36.min(area.width / 3)
    } else {
        0
    };
    let columns = Layout::horizontal([
        Constraint::Length(side_width),
        Constraint::Min(0),
        Constraint::Length(if area.width >= 20 { 2 } else { 0 }),
    ])
    .split(shell[1]);
    if side_width > 0 {
        sidebar::render_sidebar(frame, app, columns[0]);
    }
    let block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(PANEL));
    let review = block.inner(columns[1]);
    frame.render_widget(block, columns[1]);
    app.layout.diff = review;
    app.layout.ruler = columns[2];
    diff::render_diff(frame, app, review);
    chrome::render_ruler(frame, app, columns[2]);
    if app.searching {
        chrome::render_search_popup(frame, app, area);
    }
    if app.show_help {
        chrome::render_help_overlay(frame, app, area);
    }
    if app.menu.is_some() {
        chrome::render_menu(frame, app, area);
    }
}
