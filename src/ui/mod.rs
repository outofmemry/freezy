//! Hunk's padded menu bar, workspace pane, review stream, and overview ruler.

pub mod chrome;
pub mod diff;
pub mod sidebar;
pub mod syntax;
pub mod window;

pub use window::Window;

use ratatui::layout::{Constraint, Layout, Rect};

use crate::{app::App, theme::BG};

pub fn render(frame: &mut ratatui::Frame, app: &mut App) {
    let area = frame.area();
    Window::default().background(BG).render(frame, area);
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
    app.layout.ruler = columns[2];
    diff::render_diff(frame, app, columns[1]);
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
