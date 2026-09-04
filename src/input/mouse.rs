//! Mouse support: window focus/zoom, clicks, and wheel scrolling.

use std::path::Path;

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use super::load_selected;
use crate::app::{App, Msg, Workspace};

fn in_rect(r: &ratatui::layout::Rect, x: u16, y: u16) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

pub fn handle_mouse(
    mouse: MouseEvent,
    app: &mut App,
    tx: &tokio::sync::mpsc::Sender<Msg>,
    root: &Path,
) -> bool {
    if !app.show_help && !app.searching && mouse.kind == MouseEventKind::Down(MouseButton::Left) {
        if let Some((id, maximize)) = app.zoom.control_at(mouse.column, mouse.row) {
            app.zoom.focused = id;
            if maximize {
                app.zoom.maximize();
            } else {
                app.zoom.restore();
            }
            return false;
        }
        app.zoom.focus_at(mouse.column, mouse.row);
    }
    if app.show_help {
        match mouse.kind {
            MouseEventKind::ScrollDown => app.help_scroll = app.help_scroll.saturating_add(3),
            MouseEventKind::ScrollUp => app.help_scroll = app.help_scroll.saturating_sub(3),
            _ => {}
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            app.show_help = false;
        }
        return false;
    }
    if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
        if app.searching {
            if in_rect(&app.layout.search_results, mouse.column, mouse.row) {
                let start = app
                    .selected
                    .saturating_sub(app.layout.search_results.height.saturating_sub(1) as usize);
                let index = start + (mouse.row - app.layout.search_results.y) as usize;
                if index < app.visible().len() {
                    app.selected = index;
                    app.searching = false;
                    load_selected(tx, root, app);
                }
            }
            return false;
        }
        if let Some(index) = app
            .layout
            .workspace_tabs
            .iter()
            .position(|r| in_rect(r, mouse.column, mouse.row))
        {
            app.open_workspace(Workspace::ALL[index]);
            return false;
        }
        if app.workspace != Workspace::Files {
            return false;
        }
        if in_rect(&app.layout.side, mouse.column, mouse.row) {
            let y = (mouse.row - app.layout.side.y) as usize + app.layout.side_win;
            if let Some(index) = app.sidebar_hit(y) {
                app.selected = index;
                load_selected(tx, root, app);
            }
        } else if in_rect(&app.layout.ruler, mouse.column, mouse.row) {
            app.ruler_jump(
                (mouse.row - app.layout.ruler.y) as f64 / app.layout.ruler.height.max(1) as f64,
            );
        } else if in_rect(&app.layout.diff, mouse.column, mouse.row) {
            let base = app.clamped_rendered_scroll(app.layout.diff.height as usize);
            let row = base + (mouse.row - app.layout.diff.y) as usize;
            if let Some((id, _)) = app.display.gaps.iter().find(|(_, rows)| rows.start == row) {
                app.toggle_gap(*id);
            } else if app.display.code_rows.contains(&row) {
                app.cursor_row = row;
                app.move_cursor(0);
            }
        }
    } else if matches!(
        mouse.kind,
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
    ) {
        let down = mouse.kind == MouseEventKind::ScrollDown;
        if app.workspace != Workspace::Files {
            return false;
        }
        if app.searching {
            app.move_file(if down { 1 } else { -1 });
        } else if mouse.modifiers.contains(KeyModifiers::SHIFT) {
            app.wrap_lines = false;
            app.scroll_x = if down {
                app.scroll_x.saturating_add(4)
            } else {
                app.scroll_x.saturating_sub(4)
            };
        } else if in_rect(&app.layout.side, mouse.column, mouse.row) {
            if app.move_file(if down { 3 } else { -3 }) {
                load_selected(tx, root, app);
            }
        } else {
            app.diff_scroll = if down {
                app.diff_scroll.saturating_add(3)
            } else {
                app.diff_scroll.saturating_sub(3)
            };
        }
    }
    false
}
