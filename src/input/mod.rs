//! Input entry-point plus the background git loaders the UI awaits.
//!
//! Keyboard and mouse handling live in `keys` / `mouse`; this module only
//! dispatches events and owns the spawn/load helpers shared with `main`.

pub mod keys;
pub mod mouse;

use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use crossterm::event::Event;
use std::sync::OnceLock;
use tokio::sync::Semaphore;

use crate::app::{App, Msg};
use crate::core::model::FileEntry;
use crate::git::{load_diff_text, scan_all_files};
use crate::ui::syntax;

fn diff_sema() -> &'static Arc<Semaphore> {
    static SEMA: OnceLock<Arc<Semaphore>> = OnceLock::new();
    SEMA.get_or_init(|| Arc::new(Semaphore::new(2)))
}

fn latest_gen_store() -> &'static Arc<AtomicU64> {
    static LATEST: OnceLock<Arc<AtomicU64>> = OnceLock::new();
    LATEST.get_or_init(|| Arc::new(AtomicU64::new(0)))
}

pub fn spawn_files(tx: tokio::sync::mpsc::Sender<Msg>, root: PathBuf, gen: u64) {
    tokio::spawn(async move {
        let files = tokio::task::spawn_blocking(move || scan_all_files(&root))
            .await
            .unwrap_or_default();
        // Backpressure: if channel full a newer scan is queued; drop stale instead of blocking.
        let _ = tx.try_send(Msg::Files(gen, files));
    });
}

fn spawn_diff(
    tx: tokio::sync::mpsc::Sender<Msg>,
    root: PathBuf,
    f: FileEntry,
    gen: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let sema = Arc::clone(diff_sema());
        let latest = Arc::clone(latest_gen_store());
        let Ok(_permit) = sema.acquire_owned().await else {
            return;
        };
        let path = f.path.clone();
        let rel = f.rel.clone();
        let kind = f.kind;
        let res = tokio::task::spawn_blocking(move || {
            let (lines, hunks, first_line) = load_diff_text(&root, &f);
            // Coalesce: skip highlight if a newer gen was requested.
            if latest.load(Ordering::Acquire) != gen {
                return (lines, hunks, Vec::new(), first_line, true);
            }
            let syntax = syntax::highlight(&lines, &f.rel, first_line.as_deref());
            (lines, hunks, syntax, first_line, false)
        })
        .await;
        let Ok((lines, hunks, syntax, _first, stale)) = res else {
            return;
        };
        if stale {
            return;
        }
        let msg = Msg::Diff(gen, path, lines, hunks, syntax, rel, kind);
        if tx.try_send(msg).is_err() {
            // Queue flooded; render loop will pick newest next tick — drop.
        }
    })
}

pub fn load_selected(tx: &tokio::sync::mpsc::Sender<Msg>, root: &Path, app: &mut App) {
    app.gen_diff += 1;
    latest_gen_store().store(app.gen_diff, Ordering::Release);
    // Coalesce rapid j/k: abort previous diff task.
    if let Some(h) = app.diff_task.take() {
        h.abort();
    }
    app.diff_scroll = 0;
    app.anim_scroll = 0.0;
    app.scroll_x = 0;
    app.display = Default::default();
    app.expanded_gaps.clear();
    app.gap_anchor = None;
    app.diff_source = app.selected_file();
    if let Some(f) = app.selected_file() {
        app.loading_diff = true;
        app.diff_title = f.path.clone();
        app.diff_task = Some(spawn_diff(tx.clone(), root.to_path_buf(), f, app.gen_diff));
    } else {
        app.loading_diff = false;
        app.diff_source = None;
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
