//! Input entry-point plus the background git loaders the UI awaits.
//!
//! Keyboard and mouse handling live in `keys` / `mouse`; this module only
//! dispatches events and owns the spawn/load helpers shared with `main`.

pub mod keys;
pub mod mouse;

use std::path::{Path, PathBuf};

use crossterm::event::Event;

use crate::app::{App, Msg};
use crate::core::model::FileEntry;
use crate::git::{load_diff_text, scan_all_files};
use crate::ui::syntax;

pub fn spawn_files(tx: tokio::sync::mpsc::Sender<Msg>, root: PathBuf, gen: u64) {
    tokio::spawn(async move {
        let files = tokio::task::spawn_blocking(move || scan_all_files(&root))
            .await
            .unwrap_or_default();
        let _ = tx.send(Msg::Files(gen, files)).await;
    });
}

fn spawn_diff(tx: tokio::sync::mpsc::Sender<Msg>, root: PathBuf, f: FileEntry, gen: u64) {
    tokio::spawn(async move {
        let path = f.path.clone();
        let res = tokio::task::spawn_blocking(move || {
            let (lines, hunks) = load_diff_text(&root, &f);
            let syntax = syntax::highlight(&lines, &f.rel);
            (lines, hunks, syntax)
        })
        .await;
        let (lines, hunks, syntax) = res.unwrap_or_default();
        let _ = tx.send(Msg::Diff(gen, path, lines, hunks, syntax)).await;
    });
}

pub fn load_selected(tx: &tokio::sync::mpsc::Sender<Msg>, root: &Path, app: &mut App) {
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

/// Workspace tabs, the picker, and keyboard shortcuts share this handler.
pub fn handle_input(
    event: Event,
    app: &mut App,
    tx: &tokio::sync::mpsc::Sender<Msg>,
    root: &Path,
) -> bool {
    match event {
        Event::Key(key) => keys::handle_key(key, app, tx, root),
        Event::Mouse(mouse) => mouse::handle_mouse(mouse, app, tx, root),
        _ => false,
    }
}
