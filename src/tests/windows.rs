use super::{draw, fixture, press, screen};
use crate::app::{App, Workspace};
use crate::input::{handle_input, keys::handle_key};
use crate::utils::theme;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::{backend::TestBackend, Terminal};
use std::path::Path;

#[test]
fn shared_window_frames_content_and_clears_only_its_overlay() {
    use crate::ui::primitives::window::Window;
    use ratatui::{
        layout::Rect,
        widgets::{Borders, Padding, Paragraph},
    };

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
    use crate::ui::primitives::window::{WindowId, WindowRegion, REVIEW, SIDEBAR};
    use ratatui::layout::Rect;

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
#[test]
fn window_zoom_follows_clicks_and_restores_layout_step_by_step() {
    use crate::ui::primitives::window::{WindowId, REVIEW, SIDEBAR};

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
