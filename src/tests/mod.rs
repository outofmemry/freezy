//! App-level tests, grouped by area. Shared fixtures live here.

use crate::app::App;
use crate::core::model::{DKind, DLine, FileEntry};
use crate::input::keys::handle_key;
use crate::ui::{render, syntax};
use crossterm::event::{self, KeyCode, KeyModifiers};
use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};
use std::path::Path;

pub mod git;
pub mod navigation;
pub mod render;
pub mod windows;

fn file(repo: &str, rel: &str) -> FileEntry {
    FileEntry {
        path: format!("{repo}/{rel}"),
        repo: repo.into(),
        rel: rel.into(),
        kind: 'M',
        additions: 2,
        deletions: 2,
    }
}

fn fixture() -> App {
    let mut app = App::new();
    app.set_files(
        0,
        vec![
            file("freezy", "src/main.rs"),
            file("freezy", "README.md"),
            file("other", "界.rs"),
        ],
    );
    let lines = vec![
        DLine::plain("diff --git a/src/main.rs b/src/main.rs".into(), DKind::File),
        DLine::plain("@@ -1,2 +1,2 @@".into(), DKind::Hunk),
        DLine {
            text: " fn main() {".into(),
            kind: DKind::Ctx,
            old_no: Some(1),
            new_no: Some(1),
        },
        DLine {
            text: "-    let value = 1;".into(),
            kind: DKind::Del,
            old_no: Some(2),
            new_no: None,
        },
        DLine {
            text: "+    let value = 2;".into(),
            kind: DKind::Add,
            old_no: None,
            new_no: Some(2),
        },
        DLine::plain("@@ -20,1 +20,1 @@".into(), DKind::Hunk),
        DLine {
            text: "-    old();".into(),
            kind: DKind::Del,
            old_no: Some(20),
            new_no: None,
        },
        DLine {
            text: format!("+\tprintln!(\"{}\");", "界e\u{301}".repeat(24)),
            kind: DKind::Add,
            old_no: None,
            new_no: Some(20),
        },
    ];
    let syntax = syntax::highlight(&lines, "main.rs");
    app.set_diff(0, "freezy/src/main.rs".into(), lines, vec![1, 5], syntax);
    app
}

fn draw(app: &mut App, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render(frame, app)).unwrap();
    terminal.backend().buffer().clone()
}

fn screen(buffer: &Buffer) -> String {
    buffer
        .content
        .chunks(buffer.area.width.max(1) as usize)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn press(app: &mut App, code: KeyCode) {
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    assert!(!handle_key(
        event::KeyEvent::new(code, KeyModifiers::NONE),
        app,
        &tx,
        Path::new(".")
    ));
}
