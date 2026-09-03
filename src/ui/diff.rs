//! Hunk-style review rows. Wrapping, syntax, and inline changes are cached.

use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Borders, Paragraph},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::{window::REVIEW, Window};

use crate::{
    app::App,
    model::{emph_ranges, Cell, DKind, SideRow},
    theme::{
        clipped, spinner, ADD_BG, ADD_EMPH, BG, BORDER, DEL_BG, DEL_EMPH, FAINT, GREEN, MUTED,
        PANEL, PANEL_ALT, RED, TEXT,
    },
};

#[derive(Default)]
pub struct DiffView {
    key: Option<(u16, bool, bool, bool, bool, usize)>,
    pub lines: Vec<Line<'static>>,
    pub hunks: Vec<usize>,
    pub code_rows: Vec<usize>,
    // Populated columns for each code row; empty split padding is never highlighted.
    code_columns: Vec<std::ops::Range<usize>>,
    source_rows: Vec<[Option<usize>; 2]>,
    pub gaps: Vec<(usize, std::ops::Range<usize>)>,
}

impl DiffView {
    pub fn invalidate(&mut self) {
        self.key = None;
    }

    pub fn source_at(&self, row: usize) -> Option<usize> {
        let index = self.code_rows.binary_search(&row).ok()?;
        let sources = self.source_rows[index];
        sources[1].or(sources[0])
    }
}

fn push_span(spans: &mut Vec<Span<'static>>, text: &str, style: Style) {
    if let Some(last) = spans.last_mut().filter(|s| s.style == style) {
        last.content.to_mut().push_str(text);
    } else {
        spans.push(Span::styled(text.to_owned(), style));
    }
}

fn background(kind: DKind) -> Color {
    match kind {
        DKind::Add => ADD_BG,
        DKind::Del => DEL_BG,
        _ => BG,
    }
}

fn code_spans(app: &App, cell: &Cell, peer: Option<&Cell>) -> Vec<Span<'static>> {
    let emphasis = peer.map(|peer| emph_ranges(&cell.body, &peer.body));
    let fallback = Line::styled(cell.body.clone(), Style::default().fg(TEXT));
    let syntax = app.syntax.get(cell.source).unwrap_or(&fallback);
    let mut spans = Vec::new();
    let mut column = 0;
    let mut index = 0;
    for token in &syntax.spans {
        for ch in token.content.chars() {
            let changed = emphasis.is_some_and(|(start, end, _, _)| index >= start && index < end);
            let bg = if changed {
                if cell.kind == DKind::Del {
                    DEL_EMPH
                } else {
                    ADD_EMPH
                }
            } else {
                background(cell.kind)
            };
            let style = token.style.bg(bg);
            let text = if ch == '\t' {
                " ".repeat(4 - column % 4)
            } else if ch.is_control() {
                "�".into()
            } else {
                ch.to_string()
            };
            column += text.width();
            push_span(&mut spans, &text, style);
            index += 1;
        }
    }
    spans
}

/// Wrap styled terminal cells without removing code indentation.
fn wrap_spans(
    spans: &[Span<'static>],
    width: usize,
    wrap: bool,
    offset: usize,
) -> Vec<Vec<Span<'static>>> {
    let width = width.max(1);
    let mut rows = vec![Vec::new()];
    let mut used = 0;
    let mut column = 0;
    for span in spans {
        for ch in span.content.chars() {
            let size = ch.width().unwrap_or(0);
            let end = column + size;
            if !wrap && column < offset {
                // Preserve columns when the viewport starts inside a wide glyph.
                if end > offset {
                    let blanks = end - offset;
                    push_span(rows.last_mut().unwrap(), &" ".repeat(blanks), span.style);
                    used += blanks;
                }
                column = end;
                continue;
            }
            column = end;
            if used + size > width {
                if !wrap {
                    return rows;
                }
                rows.push(Vec::new());
                used = 0;
            }
            if size <= width {
                push_span(rows.last_mut().unwrap(), &ch.to_string(), span.style);
                used += size;
            }
        }
    }
    rows
}

fn cell_lines(
    app: &App,
    cell: Option<&Cell>,
    peer: Option<&Cell>,
    width: usize,
    digits: usize,
    stack: bool,
) -> Vec<Line<'static>> {
    let kind = cell.map_or(DKind::Ctx, |c| c.kind);
    let bg = background(kind);
    let fg = match kind {
        DKind::Add => GREEN,
        DKind::Del => RED,
        _ => FAINT,
    };
    let gutter_width = if app.line_numbers {
        if stack {
            digits * 2 + 5
        } else {
            digits + 4
        }
    } else {
        2
    };
    let gutter_width = gutter_width.min(width);
    let code_width = width.saturating_sub(gutter_width);
    let sign = match kind {
        DKind::Add => "+",
        DKind::Del => "-",
        _ => " ",
    };
    let number =
        |no: Option<u32>| no.map_or_else(|| " ".repeat(digits), |n| format!("{n:>digits$}"));
    let gutter = if let Some(cell) = cell {
        if app.line_numbers {
            if stack {
                let line = &app.diff_lines[cell.source];
                format!(" {} {} {sign} ", number(line.old_no), number(line.new_no))
            } else {
                format!(
                    "{}{number} {sign} ",
                    if kind == DKind::Ctx { " " } else { "▌" },
                    number = number(cell.no)
                )
            }
        } else {
            format!("{sign} ")
        }
    } else {
        " ".repeat(gutter_width)
    };
    let gutter = clipped(&gutter, gutter_width);
    let spans = cell
        .map(|cell| code_spans(app, cell, peer))
        .unwrap_or_default();
    let chunks = if code_width == 0 {
        vec![vec![]]
    } else {
        wrap_spans(&spans, code_width, app.wrap_lines, app.scroll_x)
    };
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, spans)| {
            let used: usize = spans.iter().map(|s| s.content.width()).sum();
            let mut row = vec![Span::styled(
                if i == 0 {
                    gutter.clone()
                } else {
                    " ".repeat(gutter_width)
                },
                Style::default().fg(fg).bg(bg),
            )];
            row.extend(spans);
            row.push(Span::styled(
                " ".repeat(code_width.saturating_sub(used)),
                Style::default().bg(bg),
            ));
            Line::from(row).style(Style::default().bg(bg))
        })
        .collect()
}

fn add_hunk(view: &mut DiffView, header: &str, width: usize, visible: bool) {
    finish_gap(view);
    if !view.hunks.is_empty() {
        view.lines
            .push(Line::styled(" ".repeat(width), Style::default().bg(PANEL)));
    }
    view.hunks.push(view.lines.len());
    if visible {
        let header = format!(" {}", clipped(header, width.saturating_sub(3)));
        let padding = " ".repeat(width.saturating_sub(header.width() + 2));
        view.lines.push(Line::styled(
            format!("{header}{padding}"),
            Style::default().fg(MUTED).bg(PANEL_ALT),
        ));
    }
}

fn finish_gap(view: &mut DiffView) {
    if let Some((_, rows)) = view.gaps.last_mut() {
        if rows.end == usize::MAX {
            rows.end = view.lines.len();
        }
    }
}

fn add_gap(view: &mut DiffView, id: usize, text: &str, width: usize, expanded: bool) {
    finish_gap(view);
    view.gaps.push((id, view.lines.len()..usize::MAX));
    let label = clipped(
        &format!(
            " {} {} {text}  [z]",
            if expanded { "▾" } else { "▸" },
            if expanded { "Hide" } else { "Show" }
        ),
        width.saturating_sub(2),
    );
    let padding = " ".repeat(width.saturating_sub(label.width() + 2));
    view.lines.push(Line::styled(
        format!("{label}{padding}"),
        Style::default().fg(MUTED).bg(PANEL_ALT),
    ));
}

fn prepare(app: &mut App, area: Rect) {
    let split = app.side_by_side && (!app.auto_layout || area.width >= 116) && area.width >= 20;
    let key = (
        area.width,
        split,
        app.line_numbers,
        app.wrap_lines,
        app.hunk_headers,
        if app.wrap_lines { 0 } else { app.scroll_x },
    );
    if app.display.key == Some(key) {
        return;
    }
    let source = app.display.source_at(app.cursor_row);
    let cursor_offset = app
        .cursor_row
        .checked_sub(app.rendered_scroll())
        .filter(|offset| *offset < area.height as usize);
    let mut view = DiffView {
        key: Some(key),
        ..Default::default()
    };
    let width = area.width as usize;
    view.lines
        .push(Line::styled("", Style::default().bg(PANEL)));

    let selected = app.files.iter().find(|f| f.path == app.diff_title);
    let filename = selected
        .as_ref()
        .map_or(app.diff_title.as_str(), |f| f.rel.as_str());
    let suffix = selected.as_ref().map_or("", |f| match f.kind {
        'U' => " (untracked)",
        'A' => " (added)",
        'D' => " (deleted)",
        _ => "",
    });
    let (adds, dels) = app.diff_lines.iter().fold((0, 0), |(a, d), line| {
        (
            a + usize::from(line.kind == DKind::Add),
            d + usize::from(line.kind == DKind::Del),
        )
    });
    let stats = format!("+{adds} -{dels}  ");
    let name = clipped(
        filename,
        width.saturating_sub(suffix.width() + stats.width() + 2),
    );
    let gap = " ".repeat(width.saturating_sub(name.width() + suffix.width() + stats.width() + 1));
    view.lines.push(
        Line::from(vec![
            Span::styled(format!(" {name}"), Style::default().fg(TEXT)),
            Span::styled(suffix, Style::default().fg(MUTED)),
            Span::raw(gap),
            Span::styled(format!("+{adds}"), Style::default().fg(GREEN)),
            Span::raw(" "),
            Span::styled(format!("-{dels}  "), Style::default().fg(RED)),
        ])
        .style(Style::default().bg(PANEL)),
    );
    let digits = app
        .diff_lines
        .iter()
        .flat_map(|l| [l.old_no, l.new_no])
        .flatten()
        .max()
        .unwrap_or(1)
        .to_string()
        .len();
    let untracked = app
        .hunks
        .first()
        .is_some_and(|&h| app.diff_lines.get(h).is_some_and(|l| l.kind == DKind::File));
    if untracked {
        add_hunk(
            &mut view,
            &format!("@@ -0,0 +1,{adds} @@"),
            width,
            app.hunk_headers,
        );
    }
    let code_width = width.saturating_sub(2);
    let mut folded = false;
    if split {
        let left_width = code_width / 2;
        let right_width = code_width - left_width;
        let mut gap_ids = app
            .diff_lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.kind == DKind::Gap)
            .map(|(id, _)| id);
        for row in &app.side_cache.0 {
            match row {
                SideRow::Full(_, DKind::File) => {}
                SideRow::Full(text, DKind::Hunk) => {
                    folded = false;
                    add_hunk(&mut view, text, width, app.hunk_headers)
                }
                SideRow::Full(text, DKind::Gap) => {
                    if let Some(id) = gap_ids.next() {
                        folded = !app.expanded_gaps.contains(&id);
                        add_gap(&mut view, id, text, width, !folded);
                    }
                }
                SideRow::Full(text, _) => view
                    .lines
                    .push(Line::styled(text.clone(), Style::default().fg(MUTED))),
                SideRow::Pair(old, new) => {
                    if folded {
                        continue;
                    }
                    let left =
                        cell_lines(app, old.as_ref(), new.as_ref(), left_width, digits, false);
                    let right =
                        cell_lines(app, new.as_ref(), old.as_ref(), right_width, digits, false);
                    for i in 0..left.len().max(right.len()) {
                        let mut spans = left.get(i).map(|l| l.spans.clone()).unwrap_or_else(|| {
                            vec![Span::styled(
                                " ".repeat(left_width),
                                Style::default().bg(BG),
                            )]
                        });
                        spans.extend(right.get(i).map(|l| l.spans.clone()).unwrap_or_else(|| {
                            vec![Span::styled(
                                " ".repeat(right_width),
                                Style::default().bg(BG),
                            )]
                        }));
                        view.code_rows.push(view.lines.len());
                        view.source_rows.push([
                            old.as_ref().map(|cell| cell.source),
                            new.as_ref().map(|cell| cell.source),
                        ]);
                        let start = if old.is_some() && i < left.len() {
                            0
                        } else {
                            left_width
                        };
                        let end = if new.is_some() && i < right.len() {
                            code_width
                        } else {
                            left_width
                        };
                        view.code_columns.push(start..end);
                        view.lines.push(Line::from(spans));
                    }
                }
            }
        }
    } else {
        let mut peers = vec![None; app.diff_lines.len()];
        for row in &app.side_cache.0 {
            if let SideRow::Pair(Some(old), Some(new)) = row {
                peers[old.source] = Some(new);
                peers[new.source] = Some(old);
            }
        }
        for (source, line) in app.diff_lines.iter().enumerate() {
            match line.kind {
                DKind::File => {}
                DKind::Hunk => {
                    folded = false;
                    add_hunk(&mut view, &line.text, width, app.hunk_headers);
                }
                DKind::Gap => {
                    folded = !app.expanded_gaps.contains(&source);
                    add_gap(&mut view, source, &line.text, width, !folded);
                }
                _ => {
                    if folded {
                        continue;
                    }
                    let cell = Cell {
                        source,
                        no: line.new_no.or(line.old_no),
                        kind: line.kind,
                        body: line
                            .text
                            .strip_prefix(['+', '-', ' '])
                            .unwrap_or(&line.text)
                            .into(),
                    };
                    let lines =
                        cell_lines(app, Some(&cell), peers[source], code_width, digits, true);
                    view.code_rows
                        .extend(view.lines.len()..view.lines.len() + lines.len());
                    view.code_columns
                        .extend(lines.iter().map(|_| 0..code_width));
                    view.source_rows
                        .extend(lines.iter().map(|_| [Some(source); 2]));
                    view.lines.extend(lines);
                }
            }
        }
    }
    finish_gap(&mut view);
    let cursor_index = app
        .display
        .code_rows
        .partition_point(|&row| row < app.cursor_row);
    let anchored_row = source.and_then(|source| {
        view.source_rows
            .iter()
            .position(|sources| sources.contains(&Some(source)))
            .map(|index| view.code_rows[index])
    });
    app.cursor_row = anchored_row
        .or_else(|| {
            view.code_rows
                .get(cursor_index)
                .copied()
                .or_else(|| view.code_rows.last().copied())
        })
        .unwrap_or(0);
    if let (Some(row), Some(offset)) = (anchored_row, cursor_offset) {
        app.diff_scroll = row.saturating_sub(offset);
        app.anim_scroll = app.diff_scroll as f64;
    }
    if let Some((id, offset)) = app.gap_anchor.take() {
        if let Some((_, rows)) = view.gaps.iter().find(|(gap, _)| *gap == id) {
            app.diff_scroll = rows.start.saturating_sub(offset);
            app.anim_scroll = app.diff_scroll as f64;
            app.cursor_row = if app.expanded_gaps.contains(&id) {
                view.code_rows
                    .iter()
                    .copied()
                    .find(|row| rows.contains(row))
                    .unwrap_or(rows.start)
            } else {
                rows.start
            };
        }
    }
    app.display = view;
}

pub fn render_diff(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let window = Window::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .render(frame, area);
    // The controls have their own fixed header, outside the scrolling content.
    frame.render_widget(
        Paragraph::new("─".repeat(window.width as usize)).style(Style::default().fg(BORDER)),
        Rect::new(window.x, window.y, window.width, 1.min(window.height)),
    );
    let area = Rect::new(
        window.x,
        window.y + 1.min(window.height),
        window.width,
        window.height.saturating_sub(1),
    );
    app.layout.diff = area;
    if app.loading_diff
        || app.scanning
        || app.diff_lines.is_empty()
        || app.selected_file().is_none()
    {
        let message = if app.loading_diff || app.scanning {
            format!("{} Loading changes…", spinner(app.frame))
        } else if app.selected_file().is_none() {
            if app.query.is_empty() {
                "No local changes".into()
            } else {
                "No matching files".into()
            }
        } else {
            "No text diff available (empty or binary file)".into()
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(message).alignment(Alignment::Center),
                Line::from(""),
                Line::from("r reload  ·  / files  ·  ? help").alignment(Alignment::Center),
            ])
            .style(Style::default().fg(MUTED).bg(PANEL)),
            area,
        );
        Window::controls(frame, &mut app.zoom, REVIEW, REVIEW, window);
        return;
    }
    prepare(app, area);
    let max_scroll = app.display.lines.len().saturating_sub(area.height as usize);
    app.diff_scroll = app.diff_scroll.min(max_scroll);
    app.anim_scroll = app.anim_scroll.min(max_scroll as f64);
    let start = app.rendered_scroll().min(max_scroll);
    let current = app.display.hunks.get(app.hunk_idx).copied();
    let end = app
        .display
        .hunks
        .get(app.hunk_idx + 1)
        .copied()
        .unwrap_or(app.display.lines.len());
    for (offset, line) in app
        .display
        .lines
        .iter()
        .skip(start)
        .take(area.height as usize)
        .enumerate()
    {
        let display_row = start + offset;
        let row = Rect::new(area.x, area.y + offset as u16, area.width, 1);
        frame.render_widget(Paragraph::new(line.clone()), row);
        if display_row == app.cursor_row {
            let columns = app
                .display
                .code_rows
                .binary_search(&app.cursor_row)
                .map(|index| app.display.code_columns[index].clone())
                .unwrap_or_default();
            for column in columns {
                let x = area.x + column as u16;
                let cell = &mut frame.buffer_mut()[(x, row.y)];
                if let Color::Rgb(r, g, b) = cell.bg {
                    // Hunk's current-line tint is a 20% blend toward its foreground.
                    cell.set_bg(Color::Rgb(
                        (r as f32 * 0.8 + 205.0 * 0.2).round() as u8,
                        (g as f32 * 0.8 + 214.0 * 0.2).round() as u8,
                        (b as f32 * 0.8 + 244.0 * 0.2).round() as u8,
                    ));
                }
            }
        }
        if area.width > 0 && current.is_some_and(|from| display_row >= from && display_row < end) {
            frame.buffer_mut()[(row.x, row.y)]
                .set_symbol("▌")
                .set_fg(MUTED);
        }
    }
    Window::controls(frame, &mut app.zoom, REVIEW, REVIEW, window);
}
