use super::{draw, press};
use crate::app::App;
use crate::core::model::DKind;
use crate::input::handle_input;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use std::path::{Path, PathBuf};

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
    let entry = crate::git::scan_all_files(&dir.0)
        .into_iter()
        .find(|f| f.rel == "review.rs")
        .unwrap();
    let (lines, hunks) = crate::git::load_diff_text(&dir.0, &entry);
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
    assert!(crate::core::model::with_context(&compact, &full).is_some());
    full.iter_mut()
        .find(|l| l.kind == DKind::Add)
        .unwrap()
        .text
        .push_str("changed again");
    assert!(crate::core::model::with_context(&compact, &full).is_none());
    assert!(crate::core::model::with_context(&compact, &[]).is_none());

    for split in [true, false] {
        let mut app = App::new();
        app.set_files(0, vec![entry.clone()]);
        let syntax = crate::ui::syntax::highlight(&lines, &entry.rel);
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
    let (trailing, _) = crate::git::load_diff_text(&dir.0, &entry);
    assert!(trailing
        .iter()
        .any(|l| l.kind == DKind::Gap && l.text == "56 unchanged lines"));
    assert_eq!(trailing.last().unwrap().new_no, Some(73));
    let mut tail_app = App::new();
    tail_app.set_files(0, vec![entry.clone()]);
    let (trailing, trailing_hunks) = crate::git::load_diff_text(&dir.0, &entry);
    let syntax = crate::ui::syntax::highlight(&trailing, &entry.rel);
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
    let (no_newline, _) = crate::git::load_diff_text(&dir.0, &entry);
    assert!(no_newline
        .iter()
        .any(|l| l.kind == DKind::Gap && l.text == "56 unchanged lines"));
    std::fs::remove_file(&path).unwrap();
    let (deleted, _) = crate::git::load_diff_text(&dir.0, &entry);
    assert!(!deleted.iter().any(|l| l.kind == DKind::Gap));
}
