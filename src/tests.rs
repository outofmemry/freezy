use super::*;
use model::{DKind, DLine, FileEntry};
use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

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
    let mut app = App::new("freezy".into());
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
    let syntax = ui::syntax::highlight(&lines, "main.rs");
    app.set_diff(0, "freezy/src/main.rs".into(), lines, vec![1, 5], syntax);
    app
}

fn draw(app: &mut App, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
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

#[test]
fn shared_window_frames_content_and_clears_only_its_overlay() {
    use ratatui::{
        layout::Rect,
        widgets::{Borders, Padding, Paragraph},
    };
    use ui::Window;

    let mut terminal = Terminal::new(TestBackend::new(20, 12)).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            frame.render_widget(Paragraph::new(vec!["x".repeat(20); 12].join("\n")), area);
            assert_eq!(Window::default().render(frame, area), area);
            let body = Window::default()
                .borders(Borders::ALL)
                .padding(Padding::vertical(1))
                .background(theme::BG)
                .overlay()
                .render(frame, Rect::new(2, 2, 12, 8));
            assert_eq!(body, Rect::new(3, 4, 10, 4));
            frame.render_widget(Paragraph::new("content"), body);

            // Clip partially off-screen windows before calculating their content area.
            let clipped = Window::default().render(frame, Rect::new(18, 10, 8, 8));
            assert_eq!(clipped, Rect::new(18, 10, 2, 2));
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(0, 0)].symbol(), "x");
    assert_eq!(buffer[(0, 0)].bg, theme::PANEL);
    assert_eq!(buffer[(2, 2)].symbol(), "┌");
    assert_eq!(buffer[(2, 2)].fg, theme::BORDER);
    assert_eq!(buffer[(3, 4)].symbol(), "c");
    assert_eq!(buffer[(3, 5)].symbol(), " ");
    assert_eq!(buffer[(3, 5)].bg, theme::BG);
    assert_eq!(buffer[(1, 5)].symbol(), "x");
}

#[test]
fn hunk_layout_controls_and_rendered_navigation() {
    let mut app = fixture();
    let buffer = draw(&mut app, 160, 46);
    let text = screen(&buffer);
    assert!(text.starts_with("  File  View  Navigate  Agent  Extensions  Help"));
    assert!(text.contains("SOURCE CONTROL  3"));
    assert!(!text.contains("ORIGINAL"));
    assert!(!text.contains("diff --git"));
    assert_eq!(app.layout.side.x, 1);
    assert_eq!(app.layout.side.width, 36);
    assert_eq!(app.layout.diff.x, 38);
    assert_eq!(buffer[(1, 3)].bg, theme::SEL_BG);
    assert_eq!(buffer[(1, 1)].bg, theme::PANEL_ALT);
    assert_eq!(buffer[(37, 2)].fg, theme::BORDER);
    assert_eq!(buffer[(120, 4)].bg, theme::PANEL_ALT);
    assert_eq!(app.sidebar_hit(0), Some(0));
    assert_eq!(app.sidebar_hit(1), Some(0));
    assert_eq!(app.sidebar_hit(3), None); // gap between repositories
    assert_eq!(app.sidebar_hit(5), Some(2));
    let first_cursor = app.cursor_row;
    press(&mut app, KeyCode::Down);
    assert!(app.cursor_row > first_cursor);
    assert_eq!(app.selected, 0);
    press(&mut app, KeyCode::Char(']'));
    assert_eq!(app.hunk_idx, 1);
    assert_eq!(app.diff_scroll, app.display.hunks[1]);
    let marked = app
        .ruler_marks(45)
        .iter()
        .position(|mark| *mark == Some(0))
        .unwrap();
    app.ruler_jump(marked as f64 / 45.0);
    assert_eq!(app.hunk_idx, 0);

    press(&mut app, KeyCode::F(10));
    press(&mut app, KeyCode::Right);
    press(&mut app, KeyCode::Down); // Stack
    assert!(screen(&draw(&mut app, 160, 46)).contains("[ ] Stack"));
    press(&mut app, KeyCode::Enter);
    assert!(!app.side_by_side);
    assert!(app.menu.is_none());
    draw(&mut app, 100, 20);
    assert!(app
        .display
        .lines
        .iter()
        .all(|line| line.width() <= app.layout.diff.width as usize));
    assert!(!app
        .display
        .lines
        .iter()
        .flat_map(|l| &l.spans)
        .any(|s| s.content.contains('…')));
    let wrapped = app.display.lines.len();
    press(&mut app, KeyCode::Char('w'));
    draw(&mut app, 100, 20);
    assert!(app.display.lines.len() < wrapped);
    press(&mut app, KeyCode::Char('m'));
    assert!(!screen(&draw(&mut app, 100, 20)).contains("@@"));
    assert_eq!(app.display.hunks.len(), 2);
    press(&mut app, KeyCode::Char('s'));
    draw(&mut app, 100, 20);
    assert_eq!(app.layout.side.width, 0);
    assert_eq!(app.layout.diff.x, 2);
    press(&mut app, KeyCode::Char('/'));
    for ch in "no-such-file".chars() {
        press(&mut app, KeyCode::Char(ch));
    }
    assert!(screen(&draw(&mut app, 100, 30)).contains("No matching files"));
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.selected_file().unwrap().path, app.diff_title);
    press(&mut app, KeyCode::Char('?'));
    assert!(screen(&draw(&mut app, 100, 40)).contains("Controls help"));
}

#[test]
fn window_zoom_follows_pointer_and_restores_layout_step_by_step() {
    use ui::window::{WindowId, REVIEW, SIDEBAR};

    let mouse = |app: &mut App, kind, x, y| {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        assert!(!handle_input(
            Event::Mouse(event::MouseEvent {
                kind,
                column: x,
                row: y,
                modifiers: KeyModifiers::NONE,
            }),
            app,
            &tx,
            Path::new("."),
        ));
    };
    let regions = |app: &App| {
        app.zoom
            .regions
            .iter()
            .map(|r| (r.id, r.area))
            .collect::<Vec<_>>()
    };
    let region = |app: &App, id: WindowId| *app.zoom.regions.iter().find(|r| r.id == id).unwrap();
    let split_content = |app: &App| {
        let rows: Vec<_> = app.display.lines.iter().map(ToString::to_string).collect();
        assert!(rows
            .iter()
            .any(|row| { row.contains("let value = 1") && row.contains("let value = 2") }));
        assert!(rows.iter().any(|row| row.contains("old();")));
        assert!(rows.iter().any(|row| row.contains("println!")));
    };

    // Both diff columns are content inside REVIEW, never independent zoom targets.
    for half in [0, 1] {
        let mut app = fixture();
        app.auto_layout = false;
        let normal = screen(&draw(&mut app, 160, 30));
        assert_eq!(normal.matches("[-][+]").count(), 2);
        let original = regions(&app);
        assert_eq!(original.len(), 2);
        assert_eq!(original[0].0, SIDEBAR);
        assert_eq!(original[1].0, REVIEW);
        let pane = region(&app, REVIEW);
        mouse(
            &mut app,
            MouseEventKind::Moved,
            pane.area.x + half * pane.area.width / 2 + 2,
            pane.area.y + 3,
        );
        assert_eq!(app.zoom.focused, REVIEW);
        app.cursor_row = *app.display.code_rows.last().unwrap();
        let source = app.display.source_at(app.cursor_row);
        press(&mut app, KeyCode::Char('+'));
        let maximized = screen(&draw(&mut app, 160, 30));
        assert!(app.zoom.is_hidden(SIDEBAR));
        assert_eq!(app.layout.side.width, 0);
        assert!(region(&app, REVIEW).area.width > pane.area.width);
        assert_eq!(app.display.source_at(app.cursor_row), source);
        assert_eq!(maximized.matches("[-][+]").count(), 1);
        assert_eq!(app.zoom.regions.len(), 1);
        assert_eq!(app.zoom.regions[0].id, REVIEW);
        split_content(&app);
        let level_one = regions(&app);
        press(&mut app, KeyCode::Char('+'));
        assert_eq!(screen(&draw(&mut app, 160, 30)), maximized);
        assert_eq!(app.zoom.level(), 1); // Another + cannot remove either diff column.
        assert_eq!(regions(&app), level_one);
        split_content(&app);
        press(&mut app, KeyCode::Char('-'));
        draw(&mut app, 160, 30);
        assert_eq!(regions(&app), original);
        split_content(&app);
        press(&mut app, KeyCode::Char('-'));
        assert_eq!(app.zoom.level(), 0);
        assert!(app.show_sidebar && app.side_by_side && !app.auto_layout);

        // Controls dispatch before content selection, without reloading Git.
        for maximize in [true, true, false] {
            let pane = region(&app, REVIEW);
            let button = if maximize {
                pane.maximize
            } else {
                pane.restore
            };
            mouse(
                &mut app,
                MouseEventKind::Down(MouseButton::Left),
                button.x + 1,
                button.y,
            );
            draw(&mut app, 160, 30);
            assert_eq!(app.zoom.level(), usize::from(maximize));
            split_content(&app);
        }
        assert_eq!(regions(&app), original);
        assert_eq!(app.gen_diff, 0);
        assert!(!app.loading_diff);

        app.zoom.focused = SIDEBAR;
        let ruler = app.layout.ruler;
        mouse(&mut app, MouseEventKind::Moved, ruler.x, ruler.y + 2);
        assert_eq!(app.zoom.focused, REVIEW);
    }

    let mut app = fixture();
    app.show_sidebar = false;
    let no_sidebar = screen(&draw(&mut app, 160, 30));
    press(&mut app, KeyCode::Char('+'));
    assert_eq!(screen(&draw(&mut app, 160, 30)), no_sidebar);
    assert_eq!(app.zoom.level(), 0);
    split_content(&app);
    press(&mut app, KeyCode::Char('-'));
    draw(&mut app, 160, 30);
    assert!(!app.show_sidebar);
    assert_eq!(app.layout.side.width, 0);

    app.show_sidebar = true;
    draw(&mut app, 160, 30);
    let original = regions(&app);
    app.zoom.focused = SIDEBAR;
    press(&mut app, KeyCode::Char('+'));
    let sidebar_only = screen(&draw(&mut app, 160, 30));
    assert!(app.zoom.is_hidden(REVIEW));
    assert_eq!(app.layout.diff.width, 0);
    assert_eq!(app.layout.side.width, 158);
    assert_eq!(app.zoom.regions.len(), 1);
    assert_eq!(app.zoom.regions[0].id, SIDEBAR);
    press(&mut app, KeyCode::Char('+'));
    assert_eq!(screen(&draw(&mut app, 160, 30)), sidebar_only);
    assert_eq!(app.zoom.level(), 1);
    for (width, height) in [(0, 0), (1, 1), (10, 4), (60, 12), (160, 30)] {
        draw(&mut app, width, height);
    }
    press(&mut app, KeyCode::Char('-'));
    draw(&mut app, 160, 30);
    assert_eq!(regions(&app), original);

    // F6 cycles only the actual sidebar and file-viewer windows.
    app.zoom.focused = REVIEW;
    press(&mut app, KeyCode::F(6));
    assert_eq!(app.zoom.focused, SIDEBAR);
    press(&mut app, KeyCode::F(6));
    assert_eq!(app.zoom.focused, REVIEW);

    // Split, stack, and responsive auto preserve all changes and view preferences.
    for mode in ['1', '2', '0'] {
        for width in [100, 160] {
            let mut app = fixture();
            press(&mut app, KeyCode::Char(mode));
            draw(&mut app, width, 30);
            let original = regions(&app);
            let options = (app.show_sidebar, app.side_by_side, app.auto_layout);
            app.cursor_row = *app.display.code_rows.last().unwrap();
            let source = app.display.source_at(app.cursor_row);
            press(&mut app, KeyCode::Char('+'));
            draw(&mut app, width, 30);
            assert_eq!(app.zoom.level(), 1);
            assert_eq!(app.zoom.focused, REVIEW);
            assert_eq!(app.display.source_at(app.cursor_row), source);
            let text = app
                .display
                .lines
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            for change in ["let value = 1", "let value = 2", "old();", "println!"] {
                assert!(text.contains(change));
            }
            if mode == '1' {
                split_content(&app);
            }
            press(&mut app, KeyCode::Char('+'));
            assert_eq!(app.zoom.level(), 1);
            for (width, height) in [(0, 0), (1, 1), (10, 4), (60, 12), (width, 30)] {
                draw(&mut app, width, height);
            }
            assert_eq!(app.zoom.focused, REVIEW);
            press(&mut app, KeyCode::Char('-'));
            draw(&mut app, width, 30);
            assert_eq!(regions(&app), original);
            assert_eq!(
                (app.show_sidebar, app.side_by_side, app.auto_layout),
                options
            );
        }
    }

    // Search text and overlays never zoom the windows underneath them.
    press(&mut app, KeyCode::Char('/'));
    press(&mut app, KeyCode::Char('+'));
    press(&mut app, KeyCode::Char('-'));
    assert_eq!(app.query, "+-");
    assert_eq!(app.zoom.level(), 0);
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('?'));
    press(&mut app, KeyCode::Char('+'));
    assert_eq!(app.zoom.level(), 0);
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::F(10));
    press(&mut app, KeyCode::Char('+'));
    assert_eq!(app.zoom.level(), 0);
    press(&mut app, KeyCode::Esc);

    for key in ['1', '2', '0', 'v', 's'] {
        let mut app = fixture();
        draw(&mut app, 160, 30);
        press(&mut app, KeyCode::Char('+'));
        assert_eq!(app.zoom.level(), 1);
        press(&mut app, KeyCode::Char(key));
        assert_eq!(app.zoom.level(), 0); // Explicit settings begin a fresh zoom sequence.
    }
}

#[test]
fn empty_split_cells_stay_clean_including_wrapped_and_selected_rows() {
    for kind in [DKind::Add, DKind::Del] {
        for paired in [false, true] {
            for line_numbers in [false, true] {
                let mut app = fixture();
                app.line_numbers = line_numbers;
                let lines = app.diff_lines[..5]
                    .iter()
                    .filter(|line| {
                        paired || line.kind == kind || !matches!(line.kind, DKind::Add | DKind::Del)
                    })
                    .cloned()
                    .map(|mut line| {
                        if line.kind == kind {
                            line.text = format!(
                                "{}{}",
                                if kind == DKind::Add { '+' } else { '-' },
                                "x".repeat(200)
                            );
                        }
                        line
                    })
                    .collect();
                app.set_diff(0, "freezy/src/main.rs".into(), lines, vec![1], vec![]);
                draw(&mut app, 160, 30);
                app.cursor_row = 0;
                let baseline = draw(&mut app, 160, 30);
                let area = app.layout.diff;
                let middle = area.x + (area.width - 2) / 2;
                let (empty, changed_x) = if kind == DKind::Add {
                    (area.x..middle, middle + 8)
                } else {
                    (middle..area.right() - 2, area.x + 8)
                };
                let rows = app.display.code_rows.clone();
                assert!(rows.len() > 3, "the changed line must wrap");
                for &row in rows.iter().skip(1 + usize::from(paired)) {
                    let y = area.y + row as u16;
                    app.cursor_row = row;
                    let selected = draw(&mut app, 160, 30);
                    for x in empty.clone() {
                        assert_eq!(baseline[(x, y)].bg, theme::BG, "empty cell at ({x}, {y})");
                        assert_eq!(
                            selected[(x, y)].bg,
                            theme::BG,
                            "selected empty cell at ({x}, {y})"
                        );
                    }
                    assert_ne!(selected[(changed_x, y)].bg, baseline[(changed_x, y)].bg);
                }
                // Actual context still receives the current-line highlight on both sides.
                app.cursor_row = rows[0];
                let selected = draw(&mut app, 160, 30);
                for x in [area.x + 8, middle + 8] {
                    let y = area.y + rows[0] as u16;
                    assert_eq!(baseline[(x, y)].bg, theme::BG);
                    assert_ne!(selected[(x, y)].bg, theme::BG);
                }
            }
        }
    }
}

#[test]
fn tiny_windows_empty_loading_and_overlays_never_panic() {
    let mut app = fixture();
    for (width, height) in [
        (0, 0),
        (1, 1),
        (2, 2),
        (10, 4),
        (40, 12),
        (80, 24),
        (240, 60),
    ] {
        for mode in 0..4 {
            app.searching = mode == 1;
            app.show_help = mode == 2;
            app.menu = (mode == 3).then_some(1);
            draw(&mut app, width, height);
        }
    }
    let mut empty = App::new("empty".into());
    assert!(screen(&draw(&mut empty, 100, 20)).contains("Loading changes"));
    empty.set_files(0, vec![]);
    assert!(screen(&draw(&mut empty, 100, 20)).contains("No local changes"));
    empty.set_files(0, vec![file("empty", "binary.png")]);
    assert!(screen(&draw(&mut empty, 100, 20)).contains("No text diff available"));
}

#[test]
fn cached_syntax_unicode_gutters_and_stale_loads() {
    let mut app = fixture();
    assert!(app
        .syntax
        .iter()
        .flat_map(|l| &l.spans)
        .any(|s| s.style.fg == Some(ratatui::style::Color::Rgb(203, 166, 247))));
    let original = app.diff_title.clone();
    app.set_diff(99, "stale".into(), vec![], vec![], vec![]);
    assert_eq!(app.diff_title, original);
    app.files[0].kind = 'U';
    let lines = vec![
        DLine::plain("new file".into(), DKind::File),
        DLine {
            text: "+/target".into(),
            kind: DKind::Add,
            old_no: None,
            new_no: Some(1),
        },
    ];
    let syntax = ui::syntax::highlight(&lines, ".gitignore");
    app.set_diff(0, original, lines, vec![0], syntax);
    let text = screen(&draw(&mut app, 160, 46));
    assert!(text.contains("▌1 + /target"));
    assert!(text.contains("@@ -0,0 +1,1 @@"));
    assert_eq!(
        app.ruler_marks(45).iter().filter(|m| m.is_some()).count(),
        1
    );
    assert_eq!(theme::clipped("界界", 3), "界…");
    assert_eq!(theme::clipped("abc", 0), "");
}

#[test]
fn git_context_gaps_expand_and_collapse_in_both_views() {
    // A real throwaway repository exercises HEAD/index/worktree loading, not a mock diff.
    struct TempRepo(PathBuf);
    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let dir = TempRepo(std::env::temp_dir().join(format!(
            "freezy-context-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    std::fs::create_dir(&dir.0).unwrap();
    let repo = git2::Repository::init(&dir.0).unwrap();
    let path = dir.0.join("review.rs");
    let old: Vec<_> = (1..=73)
        .map(|i| format!("let line_{i:03} = {i};"))
        .collect();
    std::fs::write(&path, format!("{}\n", old.join("\n"))).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("review.rs")).unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let signature = git2::Signature::now("Test", "test@example.invalid").unwrap();
    repo.commit(Some("HEAD"), &signature, &signature, "fixture", &tree, &[])
        .unwrap();

    let mut new = old.clone();
    new.splice(
        13..14,
        (1..=7).map(|i| format!("let first_added_{i} = {i};")),
    );
    new.splice(
        76..77,
        (1..=38).map(|i| format!("let second_added_{i} = {i};")),
    );
    std::fs::write(&path, format!("{}\n", new.join("\n"))).unwrap();
    let entry = git::scan_all_files(&dir.0)
        .into_iter()
        .find(|f| f.rel == "review.rs")
        .unwrap();
    let (lines, hunks) = git::load_diff_text(&dir.0, &entry);
    assert_eq!(hunks.len(), 2);
    assert_eq!(lines[hunks[1]].text, "@@ -68,6 +74,43 @@");
    let gaps: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind == DKind::Gap)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(gaps.len(), 2);
    assert_eq!(lines[gaps[0]].text, "10 unchanged lines");
    assert_eq!(lines[gaps[1]].text, "50 unchanged lines");
    let middle = &lines[gaps[1] + 1..hunks[1]];
    assert_eq!(middle.len(), 50);
    assert_eq!((middle[0].old_no, middle[0].new_no), (Some(18), Some(24)));
    assert_eq!((middle[49].old_no, middle[49].new_no), (Some(67), Some(73)));
    assert!(middle.iter().all(|l| l.kind == DKind::Ctx));

    // Reject a newer full diff instead of presenting a changed line as unchanged.
    let mut compact = vec![lines[0].clone()];
    for &h in &hunks {
        compact.extend(
            lines[h..]
                .iter()
                .take_while(|l| l.kind != DKind::Gap)
                .cloned(),
        );
    }
    let mut full: Vec<_> = lines
        .iter()
        .filter(|l| l.kind != DKind::Gap)
        .cloned()
        .collect();
    assert!(model::with_context(&compact, &full).is_some());
    full.iter_mut()
        .find(|l| l.kind == DKind::Add)
        .unwrap()
        .text
        .push_str("changed again");
    assert!(model::with_context(&compact, &full).is_none());
    assert!(model::with_context(&compact, &[]).is_none());

    for split in [true, false] {
        let mut app = App::new("test".into());
        app.set_files(0, vec![entry.clone()]);
        let syntax = ui::syntax::highlight(&lines, &entry.rel);
        app.set_diff(0, entry.path.clone(), lines.clone(), hunks.clone(), syntax);
        app.side_by_side = split;
        app.auto_layout = false;
        draw(&mut app, 160, 24);
        let text = |app: &App| {
            app.display
                .lines
                .iter()
                .map(|l| l.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        };
        let original_count = app.display.lines.len();
        assert!(text(&app).contains("Show 50 unchanged lines"));
        assert!(!text(&app).contains("line_050"));
        let marks = app.ruler_marks(60);
        assert_eq!(marks[35], None); // hidden context isn't a changed hunk in the ruler

        press(&mut app, KeyCode::Char(']'));
        let label = app
            .display
            .gaps
            .iter()
            .find(|(id, _)| *id == gaps[1])
            .unwrap()
            .1
            .start;
        let offset = label - app.rendered_scroll();
        press(&mut app, KeyCode::Char('z'));
        draw(&mut app, 160, 24);
        assert!(text(&app).contains("Hide 50 unchanged lines"));
        assert!(text(&app).contains("line_050"));
        assert_eq!(app.display.lines.len(), original_count + 50);
        let label = app
            .display
            .gaps
            .iter()
            .find(|(id, _)| *id == gaps[1])
            .unwrap()
            .1
            .start;
        assert_eq!(label - app.rendered_scroll(), offset);
        assert_eq!(app.ruler_marks(60), marks);
        assert_eq!(app.diff_lines[app.hunks[1]].text, "@@ -68,6 +74,43 @@");

        // Clicking the same row collapses only that gap; z can reopen it.
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let mouse = event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: app.layout.diff.x + 2,
            row: app.layout.diff.y + offset as u16,
            modifiers: KeyModifiers::NONE,
        };
        assert!(!handle_input(Event::Mouse(mouse), &mut app, &tx, &dir.0));
        draw(&mut app, 160, 24);
        assert!(!app.expanded_gaps.contains(&gaps[1]));
        assert!(!text(&app).contains("line_050"));
        press(&mut app, KeyCode::Char('z'));
        draw(&mut app, 160, 24);
        assert!(text(&app).contains("line_050"));
        press(&mut app, KeyCode::Char('m'));
        draw(&mut app, 120, 24);
        assert!(!text(&app).contains("@@"));
        assert!(text(&app).contains("Hide 50 unchanged lines"));
        press(&mut app, KeyCode::Char('z'));
        draw(&mut app, 120, 24);
        assert!(!text(&app).contains("line_050"));
        app.toggle_gap(gaps[0]);
        draw(&mut app, 120, 24);
        assert!(text(&app).contains("line_001"));
        assert!(!text(&app).contains("line_050"));
        app.set_diff(99, "stale".into(), vec![], vec![], vec![]);
        assert!(app.expanded_gaps.contains(&gaps[0]));
        app.set_diff(0, entry.path.clone(), lines.clone(), hunks.clone(), vec![]);
        assert!(app.expanded_gaps.is_empty());
    }

    // Staged-only changes also retain the trailing context, with no missing rows.
    let mut staged = old.clone();
    staged[13] = "let staged = true;".into();
    std::fs::write(&path, format!("{}\n", staged.join("\n"))).unwrap();
    index.add_path(Path::new("review.rs")).unwrap();
    index.write().unwrap();
    let (trailing, _) = git::load_diff_text(&dir.0, &entry);
    assert!(trailing
        .iter()
        .any(|l| l.kind == DKind::Gap && l.text == "56 unchanged lines"));
    assert_eq!(trailing.last().unwrap().new_no, Some(73));
    let mut tail_app = App::new("test".into());
    tail_app.set_files(0, vec![entry.clone()]);
    let (trailing, trailing_hunks) = git::load_diff_text(&dir.0, &entry);
    let syntax = ui::syntax::highlight(&trailing, &entry.rel);
    tail_app.set_diff(0, entry.path.clone(), trailing, trailing_hunks, syntax);
    draw(&mut tail_app, 160, 24);
    let tail_id = tail_app
        .diff_lines
        .iter()
        .rposition(|l| l.kind == DKind::Gap)
        .unwrap();
    tail_app.toggle_gap(tail_id);
    draw(&mut tail_app, 160, 24);
    assert!(tail_app.expanded_gaps.contains(&tail_id));
    press(&mut tail_app, KeyCode::Char('z'));
    draw(&mut tail_app, 160, 24);
    assert!(!tail_app.expanded_gaps.contains(&tail_id));

    // A file without a final newline must still support its unchanged tail.
    std::fs::write(&path, old.join("\n")).unwrap();
    index.add_path(Path::new("review.rs")).unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let parent = repo.head().unwrap().peel_to_commit().unwrap();
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "no final newline",
        &tree,
        &[&parent],
    )
    .unwrap();
    std::fs::write(&path, staged.join("\n")).unwrap();
    let (no_newline, _) = git::load_diff_text(&dir.0, &entry);
    assert!(no_newline
        .iter()
        .any(|l| l.kind == DKind::Gap && l.text == "56 unchanged lines"));
    std::fs::remove_file(&path).unwrap();
    let (deleted, _) = git::load_diff_text(&dir.0, &entry);
    assert!(!deleted.iter().any(|l| l.kind == DKind::Gap));
}
