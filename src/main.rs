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
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
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
