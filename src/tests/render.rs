use super::{draw, file, fixture, screen};
use crate::app::App;
use crate::core::model::{DKind, DLine};
use crate::utils::theme;

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
                app.set_diff(
                    0,
                    "freezy/src/main.rs".into(),
                    lines,
                    vec![1],
                    vec![],
                    "src/main.rs".into(),
                    'M',
                );
                draw(&mut app, 160, 30);
                app.cursor_row = 0;
                let baseline = draw(&mut app, 160, 30);
                let area = app.layout.diff;
                // `area` is already columns[0] after ruler split; code_width == width.
                let middle = area.x + area.width / 2;
                let (empty, changed_x) = if kind == DKind::Add {
                    (area.x..middle, middle + 8)
                } else {
                    (middle..area.right(), area.x + 8)
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
    let syntax = crate::ui::syntax::highlight(&lines, "src/Widget.tsx", None);
    assert_eq!(syntax.len(), lines.len());
    for (source, highlighted) in lines.iter().zip(&syntax).skip(2) {
        // Del lives on old side [0], Add/Ctx on new side [1] (Ctx has both).
        let text = match source.kind {
            DKind::Del => highlighted[0].to_string(),
            _ => highlighted[1].to_string(),
        };
        assert_eq!(text, source.text[1..]);
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
                .flat_map(|line| [&line[0], &line[1]])
                .flat_map(|line| &line.spans)
                .any(|span| { span.content.contains(token) && span.style.fg == Some(color) }),
            "TSX token {token:?} must have its semantic foreground"
        );
    }
    for split in [true, false] {
        let mut app = App::new();
        let entry = file("freezy", "src/Widget.tsx");
        app.set_files(0, vec![entry.clone()]);
        app.set_diff(
            0,
            entry.path,
            lines.clone(),
            vec![1],
            syntax.clone(),
            "src/Widget.tsx".into(),
            'M',
        );
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
        .flat_map(|l| [&l[0], &l[1]])
        .flat_map(|l| &l.spans)
        .any(|s| s.style.fg == Some(theme::MAGENTA)));
    let original = app.diff_title.clone();
    app.set_diff(
        99,
        "stale".into(),
        vec![],
        vec![],
        vec![],
        String::new(),
        'M',
    );
    assert_eq!(app.diff_title, original);
    app.files[0].kind = 'U';
    let lines = vec![
        DLine::plain("new file".into(), DKind::File),
        DLine::plain("@@ -0,0 +1,1 @@".into(), DKind::Hunk),
        DLine {
            text: "+/target".into(),
            kind: DKind::Add,
            old_no: None,
            new_no: Some(1),
        },
    ];
    let syntax = crate::ui::syntax::highlight(&lines, ".gitignore", None);
    app.set_diff(
        0,
        original.clone(),
        lines,
        vec![1],
        syntax,
        ".gitignore".into(),
        'U',
    );
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
