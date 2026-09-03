//! freezy — blazing-fast multi-repo diff TUI.
//! Window paints instantly; all git runs in-process on background threads.

mod app;
mod git;
mod model;
mod theme;
mod ui;

#[cfg(test)]
mod tests;

use std::{
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::Result;
use app::{App, Msg, Workspace};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton,
    MouseEventKind,
};
use crossterm::execute;
use git::{load_diff_text, scan_all_files};
use ui::render;

fn spawn_files(tx: tokio::sync::mpsc::Sender<Msg>, root: PathBuf, gen: u64) {
    tokio::spawn(async move {
        let files = tokio::task::spawn_blocking(move || scan_all_files(&root))
            .await
            .unwrap_or_default();
        let _ = tx.send(Msg::Files(gen, files)).await;
    });
}

fn spawn_diff(tx: tokio::sync::mpsc::Sender<Msg>, root: PathBuf, f: model::FileEntry, gen: u64) {
    tokio::spawn(async move {
        let path = f.path.clone();
        let res = tokio::task::spawn_blocking(move || {
            let (lines, hunks) = load_diff_text(&root, &f);
            let syntax = ui::syntax::highlight(&lines, &f.rel);
            (lines, hunks, syntax)
        })
        .await;
        let (lines, hunks, syntax) = res.unwrap_or_default();
        let _ = tx.send(Msg::Diff(gen, path, lines, hunks, syntax)).await;
    });
}

fn load_selected(tx: &tokio::sync::mpsc::Sender<Msg>, root: &Path, app: &mut App) {
    app.gen_diff += 1;
    app.diff_scroll = 0;
    app.anim_scroll = 0.0;
    app.scroll_x = 0;
    app.display = Default::default();
    app.expanded_gaps.clear();
    app.gap_anchor = None;
    if let Some(f) = app.selected_file() {
        app.loading_diff = true;
        app.diff_title = f.path.clone();
        spawn_diff(tx.clone(), root.to_path_buf(), f, app.gen_diff);
    } else {
        app.loading_diff = false;
        app.diff_lines.clear();
        app.syntax.clear();
        app.hunks.clear();
        app.side_cache = Default::default();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let scan_only = raw_args.iter().any(|a| a == "--scan");
    let root_arg = raw_args.into_iter().find(|a| a != "--scan");
    let root = root_arg
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let root = if root.is_absolute() {
        root
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(root)
    };

    // Headless check: `freezy --scan [dir]` prints changed files.
    if scan_only {
        let t = Instant::now();
        let files = scan_all_files(&root);
        for f in &files {
            println!("{} {} +{} -{}", f.kind, f.path, f.additions, f.deletions);
        }
        eprintln!("{} files in {}ms", files.len(), t.elapsed().as_millis());
        return Ok(());
    }

    let mut app = App::new();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Msg>(64);

    // First load — async so the window paints in ~1 frame.
    app.gen_files += 1;
    spawn_files(tx.clone(), root.clone(), app.gen_files);

    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;

    // Main loop: drain results → maybe refresh → ease → render → poll input.
    // Event-driven: idle blocks up to 200ms (~0% CPU); animation/loading
    // renders at ~60fps. No timer ever stalls input.
    let result: Result<()> = async {
        let mut dirty = true;
        loop {
            app.frame += 1;
            while let Ok(msg) = rx.try_recv() {
                dirty = true;
                match msg {
                    Msg::Files(gen, files) => {
                        if gen != app.gen_files {
                            continue;
                        }
                        let prev_sel = app.selected_file().map(|f| f.path);
                        app.set_files(gen, files);
                        let cur = app.selected_file();
                        let need = match (&cur, prev_sel) {
                            (Some(c), Some(p)) => c.path != p || app.diff_lines.is_empty(),
                            (Some(_), None) => true,
                            _ => false,
                        };
                        if need || cur.is_none() {
                            load_selected(&tx, &root, &mut app);
                        }
                    }
                    Msg::Diff(gen, title, lines, hunks, syntax) => {
                        app.set_diff(gen, title, lines, hunks, syntax);
                    }
                }
            }

            // Periodic background refresh (stale-safe via generations).
            if app.last_refresh.elapsed() >= Duration::from_millis(2000) && !app.searching {
                app.gen_files += 1;
                app.last_refresh = Instant::now();
                spawn_files(tx.clone(), root.clone(), app.gen_files);
                dirty = true;
            }

            let settled = app.ease_scroll();
            let busy = !settled || app.loading_diff || app.scanning;
            if dirty || busy {
                terminal.draw(|frame| render(frame, &mut app))?;
                dirty = false;
            }

            // Input: short poll while animating, long idle poll otherwise.
            if event::poll(Duration::from_millis(if busy { 16 } else { 200 }))? {
                dirty = true;
                if handle_input(event::read()?, &mut app, &tx, &root) {
                    break;
                }
            }
        }
        Ok(())
    }
    .await;

    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

fn in_rect(r: &ratatui::layout::Rect, x: u16, y: u16) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

/// Workspace tabs, the picker, and keyboard shortcuts share this handler.
fn handle_input(
    event: Event,
    app: &mut App,
    tx: &tokio::sync::mpsc::Sender<Msg>,
    root: &Path,
) -> bool {
    match event {
        Event::Key(key) => handle_key(key, app, tx, root),
        Event::Mouse(mouse) => {
            if !app.show_help && !app.searching {
                if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                    if let Some((id, maximize)) = app.zoom.control_at(mouse.column, mouse.row) {
                        app.zoom.focused = id;
                        if maximize {
                            app.zoom.maximize();
                        } else {
                            app.zoom.restore();
                        }
                        return false;
                    }
                }
                app.zoom.focus_at(mouse.column, mouse.row);
            }
            if app.show_help {
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        app.help_scroll = app.help_scroll.saturating_add(3)
                    }
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
                        let start = app.selected.saturating_sub(
                            app.layout.search_results.height.saturating_sub(1) as usize,
                        );
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
                        (mouse.row - app.layout.ruler.y) as f64
                            / app.layout.ruler.height.max(1) as f64,
                    );
                } else if in_rect(&app.layout.diff, mouse.column, mouse.row) {
                    let row = app.rendered_scroll() + (mouse.row - app.layout.diff.y) as usize;
                    if let Some((id, _)) =
                        app.display.gaps.iter().find(|(_, rows)| rows.start == row)
                    {
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
                    app.move_file(if down { 3 } else { -3 });
                    load_selected(tx, root, app);
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
        _ => false,
    }
}

fn handle_key(
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
            KeyCode::Up => app.move_file(-1),
            KeyCode::Down => app.move_file(1),
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
        KeyCode::Char('n' | '.') => {
            app.move_file(1);
            load_selected(tx, root, app);
        }
        KeyCode::Char('p' | ',') => {
            app.move_file(-1);
            load_selected(tx, root, app);
        }
        KeyCode::Down | KeyCode::Char('j') => app.move_cursor(1),
        KeyCode::Up | KeyCode::Char('k') => app.move_cursor(-1),
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
        KeyCode::Char('l') => app.line_numbers = !app.line_numbers,
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
        KeyCode::Left | KeyCode::Right => {
            app.wrap_lines = false;
            app.scroll_x = if key.code == KeyCode::Right {
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
