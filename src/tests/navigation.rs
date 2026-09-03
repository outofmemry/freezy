use super::{draw, fixture, press, screen};
use crate::app::{App, Workspace};
use crate::input::handle_input;
use crate::utils::theme;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseEventKind};
use std::path::Path;

#[test]
fn hunk_layout_controls_and_rendered_navigation() {
    use crate::ui::primitives::window::{REVIEW, SIDEBAR};
    use ratatui::style::{Color, Modifier};

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
#[tokio::test]
async fn hjkl_navigation_stays_in_the_active_window() {
    use crate::ui::primitives::window::{REVIEW, SIDEBAR};

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
    use crate::ui::primitives::window::SIDEBAR;
    use ratatui::layout::Rect;

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
