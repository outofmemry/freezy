//! Workspace tabs, file panes, review stream, and overview ruler.

pub mod components;
pub mod primitives;
pub mod syntax;

use components::{diff, help, ruler, search, sidebar, tabs};
use primitives::window::{Window, REVIEW, SIDEBAR};

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::Borders;

use crate::{
    app::{App, Workspace},
    utils::theme::BG,
};

pub fn render(frame: &mut ratatui::Frame, app: &mut App) {
    let area = frame.area();
    Window::default().background(BG).render(frame, area);
    app.layout = Default::default();
    app.zoom.begin_frame();
    let shell = Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).split(Rect::new(
        area.x + u16::from(area.width > 0),
        area.y,
        area.width.saturating_sub(2),
        area.height,
    ));
    tabs::render_workspace_tabs(frame, app, shell[0]);
    let workspace = Window::default().render(frame, shell[1]);
    if app.workspace == Workspace::Files {
        let review_visible = !app.zoom.is_hidden(REVIEW);
        let side_width = if app.show_sidebar && !app.zoom.is_hidden(SIDEBAR) {
            if !review_visible {
                workspace.width
            } else if area.width >= 60 {
                36.min(area.width / 3)
            } else {
                0
            }
        } else {
            0
        };
        let columns = Layout::horizontal([Constraint::Length(side_width), Constraint::Min(0)])
            .split(workspace);
        if side_width > 0 {
            sidebar::render_sidebar(frame, app, columns[0]);
        }
        if review_visible {
            diff::render_diff(frame, app, columns[1]);
            ruler::render_ruler(frame, app, app.layout.ruler);
        }
    } else {
        Window::default()
            .borders(Borders::ALL)
            .focused(true)
            .render(frame, workspace);
    }
    app.zoom.ensure_focus();
    if app.searching {
        search::render_search_popup(frame, app, area);
    }
    if app.show_help {
        help::render_help_overlay(frame, app, area);
    }
}
