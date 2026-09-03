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
    assert_eq!(buffer[(2, 2)].symbol(), "╭");
    assert_eq!(buffer[(2, 2)].fg, theme::BORDER);
    assert_eq!(buffer[(3, 4)].symbol(), "c");
    assert_eq!(buffer[(3, 5)].symbol(), " ");
    assert_eq!(buffer[(3, 5)].bg, theme::BG);
    assert_eq!(buffer[(1, 5)].symbol(), "x");
}

#[test]
fn hunk_layout_controls_and_rendered_navigation() {
    use ratatui::style::{Color, Modifier};
    use ui::window::{REVIEW, SIDEBAR};

    let mut app = fixture();
    let buffer = draw(&mut app, 160, 46);
    let text = screen(&buffer);
    assert!(text.lines().next().unwrap().contains("1. Files"));
    assert!(!text.contains("File  View  Navigate"));
    assert!(text.contains("SOURCE CONTROL  3"));
    assert!(!text.contains("ORIGINAL"));
    assert!(!text.contains("diff --git"));
    assert_eq!(app.layout.side.x, 2);
    assert_eq!(app.layout.side.width, 34);
    assert_eq!(app.layout.diff.x, 38);
    assert_eq!(app.layout.workspace_tabs[0].y, 0);
    assert_eq!(app.layout.workspace_tabs[0].height, 2);
    for position in [(0, 0), (2, 20), (100, 30)] {
        assert_eq!(buffer[position].bg, Color::Rgb(0, 0, 0));
    }
    assert_eq!(buffer[(2, 4)].bg, theme::BG);
    assert_eq!(buffer[(4, 4)].symbol(), "M");
    assert_eq!(buffer[(4, 4)].fg, theme::YELLOW);
    assert!(buffer[(2, 4)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(1, 2)].bg, theme::PANEL_ALT);
    for (x, y, symbol, focused) in [
        (1, 2, "╭", false),
        (36, 2, "╮", false),
        (1, 45, "╰", false),
        (36, 45, "╯", false),
        (37, 2, "╭", true),
        (158, 2, "╮", true),
        (37, 45, "╰", true),
        (158, 45, "╯", true),
    ] {
        let cell = &buffer[(x, y)];
        assert_eq!(cell.symbol(), symbol);
        assert_eq!(
            cell.fg,
            if focused {
                theme::ACCENT
            } else {
                theme::BORDER
            }
        );
        assert_eq!(cell.modifier.contains(Modifier::BOLD), focused);
    }
    for x in app.layout.side.x..app.layout.side.right() {
        assert_eq!(buffer[(x, 45)].symbol(), "─");
    }
    press(&mut app, KeyCode::F(6));
    assert_eq!(app.zoom.focused, SIDEBAR);
    let focused = draw(&mut app, 160, 46);
    assert_eq!(focused[(1, 2)].fg, theme::ACCENT);
    assert_eq!(focused[(37, 2)].fg, theme::BORDER);
    assert_eq!(focused[(2, 4)].bg, theme::SEL_BG);
    assert_eq!(focused[(2, 4)].fg, theme::SEL_FG);
    press(&mut app, KeyCode::F(6));
    assert_eq!(app.zoom.focused, REVIEW);
    assert_eq!(draw(&mut app, 160, 46), buffer);
    for (kind, color) in [
        ('A', theme::GREEN),
        ('D', theme::RED),
        ('U', theme::RED),
        ('M', theme::YELLOW),
    ] {
        app.files[1].kind = kind;
        let status = draw(&mut app, 160, 46);
        assert_eq!(status[(4, 5)].symbol(), kind.to_string());
        assert_eq!(status[(4, 5)].fg, color);
        assert_eq!(status[(4, 5)].bg, Color::Rgb(0, 0, 0));
    }
    assert_eq!(buffer[(120, 5)].bg, theme::PANEL_ALT);
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
    let ruler_height = app.layout.ruler.height as usize;
    let marked = app
        .ruler_marks(ruler_height)
        .iter()
        .position(|mark| *mark == Some(0))
        .unwrap();
    app.ruler_jump(marked as f64 / ruler_height as f64);
    assert_eq!(app.hunk_idx, 0);

    let without_navbar = screen(&draw(&mut app, 160, 46));
    for key in [KeyCode::Char('M'), KeyCode::F(10)] {
        press(&mut app, key);
        assert_eq!(screen(&draw(&mut app, 160, 46)), without_navbar);
    }
    for (key, split) in [('<', false), ('>', true), ('<', false)] {
        press(&mut app, KeyCode::Char(key));
        assert_eq!(app.side_by_side, split);
        assert!(!app.auto_layout);
        assert_eq!(app.workspace, Workspace::Files);
    }
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
fn workspace_tabs_switch_without_changing_the_files_review() {
    let input = |app: &mut App, event| {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        assert!(!handle_input(event, app, &tx, Path::new(".")));
    };
    let state = |app: &App| {
        (
            app.selected,
            app.hunk_idx,
            app.cursor_row,
            app.diff_scroll,
            app.scroll_x,
            app.gen_files,
            app.gen_diff,
            app.side_by_side,
            app.auto_layout,
            app.show_sidebar,
            app.diff_title.clone(),
        )
    };
    let mut app = fixture();
    app.auto_layout = false;
    app.side_by_side = false;
    app.scroll_x = 4;
    let first = draw(&mut app, 160, 12);
    let first_text = screen(&first);
    for (index, workspace) in Workspace::ALL.iter().enumerate() {
        assert!(first_text.contains(&format!("{}. {}", index + 1, workspace.label())));
    }
    press(&mut app, KeyCode::Char(']'));
    let original = screen(&draw(&mut app, 160, 12));
    let saved = state(&app);

    for index in [1, 2, 3, 0] {
        let tab = app.layout.workspace_tabs[index];
        input(
            &mut app,
            Event::Mouse(event::MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: tab.x + tab.width / 2,
                row: tab.y + (index % 2) as u16,
                modifiers: KeyModifiers::NONE,
            }),
        );
        let buffer = draw(&mut app, 160, 12);
        assert_eq!(app.workspace, Workspace::ALL[index]);
        if app.workspace != Workspace::Files {
            assert!(app.layout.side.is_empty() && app.layout.diff.is_empty());
            assert!(app.zoom.regions.is_empty());
            assert!(!screen(&buffer).contains("SOURCE CONTROL"));
            assert_eq!(buffer[(10, 6)].bg, theme::PANEL);
            for key in ['n', '>', '<'] {
                press(&mut app, KeyCode::Char(key));
                assert_eq!(app.workspace, Workspace::ALL[index]);
                assert_eq!(state(&app), saved);
            }
            input(
                &mut app,
                Event::Mouse(event::MouseEvent {
                    kind: MouseEventKind::ScrollDown,
                    column: 5,
                    row: 6,
                    modifiers: KeyModifiers::NONE,
                }),
            );
        }
        assert_eq!(state(&app), saved);
    }
    assert_eq!(screen(&draw(&mut app, 160, 12)), original);
    for (index, number) in ['1', '2', '3', '4'].into_iter().enumerate() {
        press(&mut app, KeyCode::Char(number));
        draw(&mut app, 160, 12);
        assert_eq!(app.workspace, Workspace::ALL[index]);
        assert_eq!(state(&app), saved);
    }
    press(&mut app, KeyCode::Char('1'));
    press(&mut app, KeyCode::Char('/'));
    for number in "1234<>".chars() {
        press(&mut app, KeyCode::Char(number));
        assert_eq!(app.workspace, Workspace::Files);
    }
    assert_eq!(app.query, "1234<>");
    assert!(!app.side_by_side && !app.auto_layout);
    press(&mut app, KeyCode::Esc);
    for workspace in Workspace::ALL {
        app.open_workspace(workspace);
        draw(&mut app, 1, 1);
    }
}

#[test]
fn control_hjkl_focuses_windows_without_running_plain_key_actions() {
    use ratatui::layout::Rect;
    use ui::window::{WindowId, WindowRegion, REVIEW, SIDEBAR};

    let control = |app: &mut App, key| {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        assert!(!handle_key(
            event::KeyEvent::new(KeyCode::Char(key), KeyModifiers::CONTROL),
            app,
            &tx,
            Path::new("."),
        ));
    };
    let state = |app: &App| {
        (
            app.cursor_row,
            app.diff_scroll,
            app.line_numbers,
            app.selected,
            app.scroll_x,
            app.wrap_lines,
        )
    };
    let mut app = fixture();
    draw(&mut app, 160, 30);
    let saved = state(&app);
    for (key, focused) in [
        ('h', SIDEBAR),
        ('j', SIDEBAR),
        ('k', SIDEBAR),
        ('h', SIDEBAR),
        ('l', REVIEW),
        ('l', REVIEW),
        ('j', REVIEW),
        ('k', REVIEW),
    ] {
        control(&mut app, key);
        assert_eq!(app.zoom.focused, focused);
        assert_eq!(state(&app), saved);
        let buffer = draw(&mut app, 160, 30);
        for (id, x) in [(SIDEBAR, 1), (REVIEW, 37)] {
            assert_eq!(
                buffer[(x, 2)].fg,
                if id == focused {
                    theme::ACCENT
                } else {
                    theme::BORDER
                }
            );
        }
    }
    press(&mut app, KeyCode::Char('L'));
    assert_ne!(app.line_numbers, saved.2);
    press(&mut app, KeyCode::Char('L'));
    press(&mut app, KeyCode::Char('j'));
    assert!(app.cursor_row > saved.0);
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(state(&app), saved);

    for (focused, hidden) in [(SIDEBAR, REVIEW), (REVIEW, SIDEBAR)] {
        app.zoom.focused = focused;
        press(&mut app, KeyCode::Char('+'));
        // Even before the next frame, hidden windows in stale regions are ignored.
        for key in ['h', 'j', 'k', 'l'] {
            control(&mut app, key);
            assert_eq!(app.zoom.focused, focused);
            assert!(app.zoom.is_hidden(hidden));
            assert_eq!(app.zoom.level(), 1);
            draw(&mut app, 160, 30);
            assert_eq!(app.zoom.regions.len(), 1);
        }
        press(&mut app, KeyCode::Char('-'));
        draw(&mut app, 160, 30);
    }

    for open in ['/', '?'] {
        press(&mut app, KeyCode::Char(open));
        let focused = app.zoom.focused;
        let query = app.query.clone();
        for key in ['h', 'j', 'k', 'l'] {
            control(&mut app, key);
            assert_eq!(app.zoom.focused, focused);
            assert_eq!(app.query, query);
            assert!(app.searching || app.show_help);
        }
        press(&mut app, KeyCode::Esc);
    }
    for workspace in &Workspace::ALL[1..] {
        app.open_workspace(*workspace);
        let buffer = draw(&mut app, 160, 30);
        let saved = state(&app);
        let focused = app.zoom.focused;
        for key in ['h', 'j', 'k', 'l'] {
            control(&mut app, key);
            assert_eq!(app.workspace, *workspace);
            assert_eq!(app.zoom.focused, focused);
            assert_eq!(state(&app), saved);
            assert_eq!(draw(&mut app, 160, 30), buffer);
        }
    }

    // A 2×2 window layout exercises vertical neighbors without adding UI components.
    app.open_workspace(Workspace::Files);
    let ids = [WindowId("a"), WindowId("b"), WindowId("c"), WindowId("d")];
    app.zoom.regions = ids
        .into_iter()
        .enumerate()
        .map(|(index, id)| WindowRegion {
            id,
            group: REVIEW,
            area: Rect::new(index as u16 % 2 * 20, index as u16 / 2 * 10, 20, 10),
            restore: Rect::default(),
            maximize: Rect::default(),
        })
        .collect();
    app.zoom.focused = ids[0];
    for (key, index) in [('l', 1), ('j', 3), ('h', 2), ('k', 0), ('h', 0), ('k', 0)] {
        control(&mut app, key);
        assert_eq!(app.zoom.focused, ids[index]);
    }
    press(&mut app, KeyCode::Char('+'));
    assert!(app.zoom.is_hidden(ids[1]));
    for key in ['l', 'j', 'h', 'k'] {
        control(&mut app, key);
        assert_ne!(app.zoom.focused, ids[1]);
        assert!(app.zoom.is_hidden(ids[1]));
        assert_eq!(app.zoom.level(), 1);
    }
}

#[tokio::test]
async fn hjkl_navigation_stays_in_the_active_window() {
    use ui::window::{REVIEW, SIDEBAR};

    let view = |app: &App| {
        (
            app.cursor_row,
            app.scroll_x,
            app.wrap_lines,
            app.line_numbers,
            app.diff_scroll,
        )
    };
    let mut app = fixture();
    draw(&mut app, 160, 30);
    for focused in [SIDEBAR, REVIEW] {
        app.zoom.focused = focused;
        let saved = (app.selected, app.gen_diff, view(&app));
        for key in ['n', 'p', '.', ','] {
            press(&mut app, KeyCode::Char(key));
            assert_eq!((app.selected, app.gen_diff, view(&app)), saved);
            assert_eq!(app.zoom.focused, focused);
        }
    }
    app.zoom.focused = SIDEBAR;
    app.scroll_x = 12;
    let saved = view(&app);
    for key in [
        KeyCode::Char('h'),
        KeyCode::Char('l'),
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Char('L'),
    ] {
        press(&mut app, key);
        assert_eq!(view(&app), saved);
        assert_eq!(app.zoom.focused, SIDEBAR);
    }
    for (key, selected) in [
        (KeyCode::Char('j'), 1),
        (KeyCode::Char('k'), 0),
        (KeyCode::Down, 1),
        (KeyCode::Up, 0),
    ] {
        let generation = app.gen_diff;
        press(&mut app, key);
        assert_eq!(app.selected, selected);
        assert_eq!(app.diff_title, app.files[selected].path);
        assert!(app.loading_diff);
        assert_eq!(app.gen_diff, generation + 1);
        assert_eq!(app.cursor_row, saved.0);
        assert_eq!(app.zoom.focused, SIDEBAR);
    }

    app = fixture();
    draw(&mut app, 160, 30);
    let cursor = app.cursor_row;
    for (down, up) in [
        (KeyCode::Char('j'), KeyCode::Char('k')),
        (KeyCode::Down, KeyCode::Up),
    ] {
        press(&mut app, down);
        assert!(app.cursor_row > cursor);
        press(&mut app, up);
        assert_eq!(app.cursor_row, cursor);
        assert_eq!(app.selected, 0);
        assert_eq!(app.gen_diff, 0);
        assert_eq!(app.zoom.focused, REVIEW);
    }
    for (key, offset) in [
        (KeyCode::Char('l'), 4),
        (KeyCode::Right, 8),
        (KeyCode::Char('h'), 4),
        (KeyCode::Left, 0),
    ] {
        press(&mut app, key);
        assert_eq!(app.scroll_x, offset);
        assert!(!app.wrap_lines);
        assert!(app.line_numbers);
        assert_eq!(app.selected, 0);
    }
    press(&mut app, KeyCode::Char('L'));
    assert!(!app.line_numbers);

    for (focused, hidden) in [(SIDEBAR, REVIEW), (REVIEW, SIDEBAR)] {
        app = fixture();
        draw(&mut app, 160, 30);
        app.zoom.focused = focused;
        press(&mut app, KeyCode::Char('+'));
        for key in ['h', 'j', 'k', 'l'] {
            press(&mut app, KeyCode::Char(key));
            assert_eq!(app.zoom.focused, focused);
            assert!(app.zoom.is_hidden(hidden));
            assert_eq!(app.zoom.level(), 1);
        }
    }

    app = fixture();
    draw(&mut app, 160, 30);
    app.zoom.focused = SIDEBAR;
    let saved = view(&app);
    press(&mut app, KeyCode::Char('?'));
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.help_scroll, 1);
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(app.help_scroll, 0);
    press(&mut app, KeyCode::Char('h'));
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(view(&app), saved);
    assert_eq!(app.selected, 0);
    assert_eq!(app.zoom.focused, SIDEBAR);
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('/'));
    for key in "hjklLnp.,".chars() {
        press(&mut app, KeyCode::Char(key));
    }
    assert_eq!(app.query, "hjklLnp.,");
    assert_eq!(view(&app), saved);
    assert_eq!(app.zoom.focused, SIDEBAR);
}

#[tokio::test]
async fn navigation_clamps_at_file_code_and_hunk_boundaries() {
    use ratatui::layout::Rect;
    use ui::window::SIDEBAR;

    let wheel = |app: &mut App, kind, area: Rect| {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        assert!(!handle_input(
            Event::Mouse(event::MouseEvent {
                kind,
                column: area.x,
                row: area.y,
                modifiers: KeyModifiers::NONE,
            }),
            app,
            &tx,
            Path::new("."),
        ));
    };
    let preview = |app: &App| {
        (
            app.selected,
            app.gen_diff,
            app.hunk_idx,
            app.cursor_row,
            app.diff_scroll,
            app.scroll_x,
            app.anim_scroll,
            app.display.lines.len(),
        )
    };
    for count in [0, 1, 3] {
        let mut app = fixture();
        app.set_files(0, app.files[..count].to_vec());
        draw(&mut app, 160, 20);
        app.zoom.focused = SIDEBAR;
        for (selected, key, arrow, scroll, delta) in [
            (0, 'k', KeyCode::Up, MouseEventKind::ScrollUp, isize::MIN),
            (
                count.saturating_sub(1),
                'j',
                KeyCode::Down,
                MouseEventKind::ScrollDown,
                isize::MAX,
            ),
        ] {
            app.selected = selected;
            app.hunk_idx = 1;
            app.diff_scroll = 7;
            app.scroll_x = 8;
            app.anim_scroll = 6.5;
            let saved = preview(&app);
            let side = app.layout.side;
            for _ in 0..2 {
                press(&mut app, KeyCode::Char(key));
                press(&mut app, arrow);
                wheel(&mut app, scroll, side);
                assert!(!app.move_file(delta));
                assert!(!app.move_file(0));
                assert_eq!(preview(&app), saved);
                assert!(!app.loading_diff);
            }
        }
    }

    let mut app = fixture();
    assert!(app.move_file(isize::MAX));
    assert_eq!(app.selected, 2);
    assert!(app.move_file(isize::MIN));
    assert_eq!(app.selected, 0);
    draw(&mut app, 160, 20);
    let side = app.layout.side;
    for (scroll, selected) in [
        (MouseEventKind::ScrollDown, 2),
        (MouseEventKind::ScrollUp, 0),
    ] {
        let generation = app.gen_diff;
        wheel(&mut app, scroll, side);
        assert_eq!(app.selected, selected);
        assert_eq!(app.gen_diff, generation + 1);
        assert_eq!(app.diff_title, app.files[selected].path);
    }

    app = fixture();
    draw(&mut app, 160, 8);
    let max_scroll = app.display.lines.len() - app.layout.diff.height as usize;
    assert!(max_scroll > 0);
    for (jump, key, arrow, scroll, expected_scroll, cursor) in [
        (
            KeyCode::End,
            'j',
            KeyCode::Down,
            MouseEventKind::ScrollDown,
            max_scroll,
            *app.display.code_rows.last().unwrap(),
        ),
        (
            KeyCode::Home,
            'k',
            KeyCode::Up,
            MouseEventKind::ScrollUp,
            0,
            app.display.code_rows[0],
        ),
    ] {
        press(&mut app, jump);
        for _ in 0..3 {
            press(&mut app, KeyCode::Char(key));
            press(&mut app, arrow);
            let area = app.layout.diff;
            wheel(&mut app, scroll, area);
            draw(&mut app, 160, 8);
            assert_eq!(app.cursor_row, cursor);
            assert_eq!(app.diff_scroll, expected_scroll);
            assert_eq!(app.selected, 0);
        }
    }
    for (delta, index, key) in [(isize::MAX, 1, ']'), (isize::MIN, 0, '[')] {
        app.move_hunk(delta);
        assert_eq!(app.hunk_idx, index);
        app.diff_scroll = app.hunk_scroll_target() + 1;
        let saved = (app.diff_scroll, app.cursor_row);
        app.move_hunk(delta);
        press(&mut app, KeyCode::Char(key));
        assert_eq!(app.hunk_idx, index);
        assert_eq!((app.diff_scroll, app.cursor_row), saved);
        app.move_hunk(0); // The ruler must still explicitly reposition the current hunk.
        assert_eq!(app.diff_scroll, app.hunk_scroll_target());
    }
}

#[test]
fn window_zoom_follows_clicks_and_restores_layout_step_by_step() {
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
        for target in [SIDEBAR, REVIEW] {
            let pane = region(&app, target);
            let x = if target == SIDEBAR {
                pane.area.x // Click the frame, not a file row that would start a Git load.
            } else {
                pane.area.x + half * pane.area.width / 2 + 2
            };
            let y = pane.area.y + 3;
            let focused = app.zoom.focused;
            assert_ne!(focused, target);
            for kind in [
                MouseEventKind::Moved,
                MouseEventKind::Drag(MouseButton::Left),
                MouseEventKind::Up(MouseButton::Left),
                MouseEventKind::ScrollDown,
                MouseEventKind::ScrollUp,
                MouseEventKind::Down(MouseButton::Right),
            ] {
                mouse(&mut app, kind, x, y);
                assert_eq!(app.zoom.focused, focused);
            }
            mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
            assert_eq!(app.zoom.focused, target);
        }
        let pane = region(&app, REVIEW);
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
        assert_eq!(app.zoom.focused, SIDEBAR);
        mouse(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            ruler.x,
            ruler.y + 2,
        );
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
    assert_eq!(app.layout.side.width, 156);
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
    for mode in ['>', '<', '0'] {
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
            if mode == '>' {
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
    // Changing diff presentation must not reopen a window hidden by maximize.
    for (focused, hidden) in [(REVIEW, SIDEBAR), (SIDEBAR, REVIEW)] {
        let mut app = fixture();
        draw(&mut app, 160, 30);
        let original = regions(&app);
        app.zoom.focused = focused;
        press(&mut app, KeyCode::Char('+'));
        draw(&mut app, 160, 30);
        let maximized = regions(&app);
        for key in ['<', '>', 'v', '0', '<', '>'] {
            press(&mut app, KeyCode::Char(key));
            draw(&mut app, 160, 30);
            assert_eq!(app.zoom.level(), 1, "layout key {key} reset zoom");
            assert!(app.zoom.is_hidden(hidden));
            assert_eq!(regions(&app), maximized);
        }
        press(&mut app, KeyCode::Char('-'));
        draw(&mut app, 160, 30);
        assert_eq!(app.zoom.level(), 0);
        assert_eq!(regions(&app), original);
    }

    let mut app = fixture();
    draw(&mut app, 160, 30);
    press(&mut app, KeyCode::Char('+'));
    press(&mut app, KeyCode::Char('s'));
    assert_eq!(app.zoom.level(), 0); // Explicit sidebar toggles still reset zoom.
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
                    assert_eq!(selected[(x, y)].bg, theme::SEL_BG);
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
        for mode in 0..3 {
            app.searching = mode == 1;
            app.show_help = mode == 2;
            draw(&mut app, width, height);
        }
    }
    let mut empty = App::new();
    assert!(screen(&draw(&mut empty, 100, 20)).contains("Loading changes"));
    empty.set_files(0, vec![]);
    assert!(screen(&draw(&mut empty, 100, 20)).contains("No local changes"));
    empty.set_files(0, vec![file("empty", "binary.png")]);
    assert!(screen(&draw(&mut empty, 100, 20)).contains("No text diff available"));
}

#[test]
fn tsx_syntax_colors_survive_split_stack_and_change_backgrounds() {
    let purple = theme::MAGENTA;
    let comment = theme::FAINT;
    let mut lines = vec![
        DLine::plain("diff --git a/Widget.tsx b/Widget.tsx".into(), DKind::File),
        DLine::plain("@@ -1,4 +1,4 @@".into(), DKind::Hunk),
    ];
    lines.extend(
        [
            (DKind::Ctx, 1, "import React from \"react\";"),
            (DKind::Ctx, 2, "type Props = { title: string };"),
            (DKind::Ctx, 3, "// Render the title"),
            (
                DKind::Del,
                4,
                "export const Widget = () => <h1 className=\"old\">Title</h1>;",
            ),
            (
                DKind::Add,
                4,
                "export const Widget = () => <h1 className=\"new\">Title</h1>;",
            ),
        ]
        .into_iter()
        .map(|(kind, number, body)| DLine {
            text: format!(
                "{}{body}",
                match kind {
                    DKind::Del => '-',
                    DKind::Add => '+',
                    _ => ' ',
                }
            ),
            kind,
            old_no: (kind != DKind::Add).then_some(number),
            new_no: (kind != DKind::Del).then_some(number),
        }),
    );
    let syntax = ui::syntax::highlight(&lines, "src/Widget.tsx");
    assert_eq!(syntax.len(), lines.len());
    for (source, highlighted) in lines.iter().zip(&syntax).skip(2) {
        assert_eq!(highlighted.to_string(), source.text[1..]);
    }
    for (token, color) in [
        ("import", purple),
        ("type", purple),
        ("h1", purple),
        ("react", theme::GREEN),
        ("old", theme::GREEN),
        ("Render", comment),
    ] {
        assert!(
            syntax
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| { span.content.contains(token) && span.style.fg == Some(color) }),
            "TSX token {token:?} must have its semantic foreground"
        );
    }
    for split in [true, false] {
        let mut app = App::new();
        let entry = file("freezy", "src/Widget.tsx");
        app.set_files(0, vec![entry.clone()]);
        app.set_diff(0, entry.path, lines.clone(), vec![1], syntax.clone());
        app.side_by_side = split;
        app.auto_layout = false;
        let buffer = draw(&mut app, 220, 26);
        for (symbol, foreground, background) in [
            ("e", purple, theme::DEL_BG),
            ("e", purple, theme::ADD_BG),
            ("o", theme::GREEN, theme::DEL_EMPH),
            ("n", theme::GREEN, theme::ADD_EMPH),
            ("R", comment, theme::BG),
        ] {
            assert!(buffer.content.iter().any(|cell| {
                cell.symbol() == symbol && cell.fg == foreground && cell.bg == background
            }), "split={split}: missing {symbol:?} with foreground {foreground:?} and background {background:?}");
        }
    }
}

#[test]
fn cached_syntax_unicode_gutters_and_stale_loads() {
    let mut app = fixture();
    assert!(app
        .syntax
        .iter()
        .flat_map(|l| &l.spans)
        .any(|s| s.style.fg == Some(theme::MAGENTA)));
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
        let mut app = App::new();
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
    let mut tail_app = App::new();
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
