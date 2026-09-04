//! freezy — blazing-fast multi-repo diff TUI.
//! Window paints instantly; all git runs in-process on background threads.

mod app;
mod core;
mod git;
mod input;
mod ui;
mod utils;

#[cfg(test)]
mod tests;

use std::{
    io,
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::Result;
use app::{App, Msg};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, EventStream};
use crossterm::execute;
use futures::StreamExt;
use git::scan_all_files;
use input::{handle_input, load_selected, spawn_files};
use ui::render;

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
    app.scanning = true;
    spawn_files(tx.clone(), root.clone(), app.gen_files);

    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;

    // Main loop: drain results → maybe refresh → ease → render → poll input.
    // Event-driven: idle blocks up to 200ms (~0% CPU); animation/loading
    // renders at ~60fps. Input is async so tokio workers are never blocked.
    let result: Result<()> = async {
        let mut dirty = true;
        let mut reader = EventStream::new();
        loop {
            app.frame += 1;
            while let Ok(msg) = rx.try_recv() {
                dirty = true;
                match msg {
                    Msg::Files(gen, files) => {
                        if gen != app.gen_files {
                            continue;
                        }
                        let prev_entry = app.selected_file();
                        let prev_sel = prev_entry.as_ref().map(|f| f.path.clone());
                        app.set_files(gen, files);
                        let cur = app.selected_file();
                        // Reload when selection moved, when open file's stat
                        // changed under refresh, or when no diff yet. Comparing
                        // full entry avoids reload loops; `diff_source` dedupes `r`.
                        let need = match (&cur, prev_sel, prev_entry) {
                            (Some(c), Some(p), _) if c.path != p => true,
                            (Some(c), _, _) if Some(c) != app.diff_source.as_ref() => true,
                            (Some(_), None, _) => true,
                            (Some(_), Some(_), _) => app.diff_lines.is_empty() && !app.loading_diff,
                            _ => false,
                        };
                        if need || cur.is_none() {
                            load_selected(&tx, &root, &mut app);
                        }
                    }
                    Msg::Diff(gen, title, lines, hunks, syntax, rel, kind) => {
                        app.set_diff(gen, title, lines, hunks, syntax, rel, kind);
                    }
                }
            }

            // Periodic background refresh: skip tick while a scan is
            // in-flight (`scanning`). Stale-safe via generations.
            if app.last_refresh.elapsed() >= Duration::from_millis(2000)
                && !app.searching
                && !app.scanning
            {
                app.gen_files += 1;
                app.scanning = true;
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

            // Input: async stream, never blocks pool.
            let deadline = tokio::time::sleep(Duration::from_millis(if busy { 16 } else { 200 }));
            tokio::pin!(deadline);
            tokio::select! {
                biased;
                maybe = reader.next() => {
                    match maybe {
                        Some(Ok(ev)) => {
                            dirty = true;
                            if handle_input(ev, &mut app, &tx, &root) {
                                break;
                            }
                        }
                        Some(Err(e)) => return Err(e.into()),
                        None => break,
                    }
                }
                _ = &mut deadline => {}
            }
        }
        Ok(())
    }
    .await;

    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}
