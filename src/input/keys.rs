//! Keyboard shortcuts for workspaces, navigation, and view options.

use std::path::Path;

use crossterm::event::{self, KeyCode, KeyModifiers};

use super::{load_selected, spawn_files};
use crate::app::{App, Msg, Workspace};
use crate::ui::primitives::window::{REVIEW, SIDEBAR};

pub fn handle_key(
    key: event::KeyEvent,
    app: &mut App,
    tx: &tokio::sync::mpsc::Sender<Msg>,
    root: &Path,
) -> bool {
    if key.kind == event::KeyEventKind::Release {
        return false;
    }
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return true;
    }
    if app.show_help {
        match key.code {
            KeyCode::Char('?') | KeyCode::Esc => app.show_help = false,
            KeyCode::Char('q') => return true,
            KeyCode::Down | KeyCode::Char('j') => {
                app.help_scroll = app.help_scroll.saturating_add(1)
            }
            KeyCode::Up | KeyCode::Char('k') => app.help_scroll = app.help_scroll.saturating_sub(1),
            KeyCode::PageDown | KeyCode::Char(' ') => {
                app.help_scroll = app.help_scroll.saturating_add(10)
            }
            KeyCode::PageUp => app.help_scroll = app.help_scroll.saturating_sub(10),
            _ => {}
        }
        return false;
    }
    if app.searching {
        match key.code {
            KeyCode::Esc => {
                app.searching = false;
                app.query.clear();
                app.refresh_filter();
                app.selected = app
                    .files
                    .iter()
                    .position(|f| f.path == app.diff_title)
                    .unwrap_or(0);
            }
            KeyCode::Enter | KeyCode::Tab => {
                if !app.visible().is_empty() {
                    app.searching = false;
                    load_selected(tx, root, app);
                }
            }
            KeyCode::Up => {
                app.move_file(-1);
            }
            KeyCode::Down => {
                app.move_file(1);
            }
            KeyCode::Backspace => {
                app.query.pop();
                app.refresh_filter();
                app.selected = 0;
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.query.push(c);
                app.refresh_filter();
                app.selected = 0;
            }
            _ => {}
        }
        return false;
    }
    match key.code {
        KeyCode::Char('q') => return true,
        KeyCode::Char(number @ '1'..='4') => {
            app.open_workspace(Workspace::ALL[number as usize - '1' as usize]);
        }
        KeyCode::Char('?') => {
            app.show_help = true;
            app.help_scroll = 0;
        }
        _ if app.workspace != Workspace::Files => return false,
        KeyCode::Char(direction @ ('h' | 'j' | 'k' | 'l'))
            if key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            app.zoom.focus_direction(direction);
        }
        KeyCode::Char('+') => {
            app.zoom.maximize();
        }
        KeyCode::Char('-') => {
            app.zoom.restore();
        }
        KeyCode::F(6) => {
            if let Some(index) = app
                .zoom
                .regions
                .iter()
                .position(|region| region.id == app.zoom.focused)
            {
                app.zoom.focused = app.zoom.regions[(index + 1) % app.zoom.regions.len()].id;
            }
        }
        KeyCode::Down | KeyCode::Up | KeyCode::Char('j' | 'k') => {
            let delta = if matches!(key.code, KeyCode::Down | KeyCode::Char('j')) {
                1
            } else {
                -1
            };
            match app.zoom.focused {
                SIDEBAR => {
                    if app.move_file(delta) {
                        load_selected(tx, root, app);
                    }
                }
                REVIEW => app.move_cursor(delta),
                _ => {}
            }
        }
        KeyCode::Char(']') => app.move_hunk(1),
        KeyCode::Char('[') => app.move_hunk(-1),
        KeyCode::Char('g') | KeyCode::Home => {
            app.move_cursor(isize::MIN);
            app.diff_scroll = 0;
        }
        KeyCode::Char('G') | KeyCode::End => {
            app.move_cursor(isize::MAX);
            app.diff_scroll = app
                .display
                .lines
                .len()
                .saturating_sub(app.layout.diff.height as usize)
        }
        KeyCode::Char('v') => {
            app.side_by_side = !app.side_by_side;
            app.auto_layout = false;
        }
        KeyCode::Char('<' | '>' | '0') => {
            app.side_by_side = key.code != KeyCode::Char('<');
            app.auto_layout = key.code == KeyCode::Char('0');
        }
        KeyCode::Char('s') => {
            app.zoom.reset();
            app.show_sidebar = !app.show_sidebar;
        }
        KeyCode::Char('L') if app.zoom.focused == REVIEW => app.line_numbers = !app.line_numbers,
        KeyCode::Char('w') => {
            app.wrap_lines = !app.wrap_lines;
            app.scroll_x = 0;
        }
        KeyCode::Char('m') => app.hunk_headers = !app.hunk_headers,
        KeyCode::Char('z') => {
            if let Some(id) = app.selected_gap() {
                app.toggle_gap(id);
            }
        }
        KeyCode::Char('/' | 'o') | KeyCode::Tab => {
            app.searching = true;
            app.query.clear();
            app.refresh_filter();
            app.selected = app
                .files
                .iter()
                .position(|f| f.path == app.diff_title)
                .unwrap_or(0);
        }
        KeyCode::Char('r') => {
            app.gen_files += 1;
            app.scanning = app.files.is_empty();
            spawn_files(tx.clone(), root.to_path_buf(), app.gen_files);
            load_selected(tx, root, app);
        }
        KeyCode::PageDown | KeyCode::Char(' ' | 'f') => {
            let page = app.layout.diff.height.saturating_sub(2).max(1) as usize;
            app.diff_scroll = app.diff_scroll.saturating_add(page);
            app.move_cursor(page as isize);
        }
        KeyCode::PageUp | KeyCode::Char('b') => {
            let page = app.layout.diff.height.saturating_sub(2).max(1) as usize;
            app.diff_scroll = app.diff_scroll.saturating_sub(page);
            app.move_cursor(-(page as isize));
        }
        KeyCode::Char('d') => {
            let page = app.layout.diff.height.saturating_sub(2).max(1) as usize;
            app.diff_scroll = app.diff_scroll.saturating_add((page / 2).max(1));
            app.move_cursor((page / 2).max(1) as isize);
        }
        KeyCode::Char('u') => {
            let page = app.layout.diff.height.saturating_sub(2).max(1) as usize;
            app.diff_scroll = app.diff_scroll.saturating_sub((page / 2).max(1));
            app.move_cursor(-((page / 2).max(1) as isize));
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Char('h' | 'l') if app.zoom.focused == REVIEW => {
            app.wrap_lines = false;
            app.scroll_x = if matches!(key.code, KeyCode::Right | KeyCode::Char('l')) {
                app.scroll_x.saturating_add(4)
            } else {
                app.scroll_x.saturating_sub(4)
            };
        }
        KeyCode::Esc if !app.query.is_empty() => {
            app.query.clear();
            app.refresh_filter();
            app.selected = app
                .files
                .iter()
                .position(|f| f.path == app.diff_title)
                .unwrap_or(0);
        }
        KeyCode::Enter => load_selected(tx, root, app),
        _ => {}
    }
    false
}
