//! Core data model: files, diff lines, side-by-side rows.

use crate::theme::{ACCENT, GREEN, RED};

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: String, // "repo/rel"
    pub repo: String,
    pub rel: String,
    pub kind: char, // M A D U
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DKind {
    File,
    Hunk,
    Gap,
    Add,
    Del,
    Ctx,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DLine {
    pub text: String,
    pub kind: DKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
}

/// Keep Git's original hunk boundaries while retaining omitted context as folded rows.
/// Reject mismatched snapshots: the worktree may change between the two Git reads.
pub fn with_context(compact: &[DLine], full: &[DLine]) -> Option<(Vec<DLine>, Vec<usize>)> {
    fn append_gap(out: &mut Vec<DLine>, context: &[&DLine]) -> Option<()> {
        if context.is_empty() {
            return Some(());
        }
        if context
            .iter()
            .any(|l| l.kind != DKind::Ctx || l.old_no.is_some() != l.new_no.is_some())
        {
            return None;
        }
        // libgit2's no-final-newline marker has no line numbers; it isn't a source line.
        let count = context.iter().filter(|l| l.old_no.is_some()).count();
        if count == 0 {
            return Some(());
        }
        out.push(DLine::plain(
            format!(
                "{} unchanged {}",
                count,
                if count == 1 { "line" } else { "lines" }
            ),
            DKind::Gap,
        ));
        out.extend(context.iter().map(|line| (*line).clone()));
        Some(())
    }
    let full: Vec<_> = full
        .iter()
        .filter(|l| !matches!(l.kind, DKind::File | DKind::Hunk))
        .collect();
    let mut out = Vec::new();
    let mut hunks = Vec::new();
    let mut cursor = 0;
    for (index, line) in compact.iter().enumerate() {
        match line.kind {
            DKind::File => out.push(line.clone()),
            DKind::Hunk => {
                let first = compact.get(index + 1)?;
                let offset = full[cursor..].iter().position(|l| *l == first)?;
                append_gap(&mut out, &full[cursor..cursor + offset])?;
                cursor += offset;
                hunks.push(out.len());
                out.push(line.clone());
            }
            _ => {
                if full.get(cursor).copied() != Some(line) {
                    return None;
                }
                out.push(line.clone());
                cursor += 1;
            }
        }
    }
    append_gap(&mut out, &full[cursor..])?;
    Some((out, hunks))
}

impl DLine {
    pub fn plain(text: String, kind: DKind) -> Self {
        Self {
            text,
            kind,
            old_no: None,
            new_no: None,
        }
    }
}

pub fn kind_color(k: char) -> ratatui::style::Color {
    match k {
        'A' => GREEN,
        'D' => RED,
        'U' => GREEN,
        _ => ACCENT,
    }
}

pub fn stats_label(f: &FileEntry) -> String {
    match f.kind {
        'U' => "U".into(),
        'D' => format!("-{}", f.deletions),
        'A' => format!("+{}", f.additions),
        _ => format!("+{} -{}", f.additions, f.deletions),
    }
}

// ── side-by-side rows ───────────────────────────────────────────────────────
#[derive(Clone)]
pub struct Cell {
    pub source: usize,
    pub no: Option<u32>,
    pub body: String, // code without +/- marker
    pub kind: DKind,
}

pub enum SideRow {
    Full(String, DKind), // file / hunk header
    Pair(Option<Cell>, Option<Cell>),
}

fn cell_of(dl: &DLine, side: u8, source: usize) -> Cell {
    let body = dl
        .text
        .strip_prefix(['+', '-', ' '])
        .unwrap_or(&dl.text)
        .to_owned();
    Cell {
        source,
        no: if side == 0 { dl.old_no } else { dl.new_no },
        body,
        kind: dl.kind,
    }
}

/// Pair one hunk body: context 1:1, del/add runs zipped line-by-line.
fn pair_hunk(body: &[DLine], offset: usize) -> Vec<SideRow> {
    let mut rows = vec![];
    let mut i = 0;
    while i < body.len() {
        match body[i].kind {
            DKind::Ctx => {
                let c = cell_of(&body[i], 0, offset + i);
                let c2 = cell_of(&body[i], 1, offset + i);
                rows.push(SideRow::Pair(Some(c), Some(c2)));
                i += 1;
            }
            DKind::Del => {
                let mut dels = vec![];
                while i < body.len() && body[i].kind == DKind::Del {
                    dels.push(cell_of(&body[i], 0, offset + i));
                    i += 1;
                }
                let mut adds = vec![];
                while i < body.len() && body[i].kind == DKind::Add {
                    adds.push(cell_of(&body[i], 1, offset + i));
                    i += 1;
                }
                let n = dels.len().max(adds.len());
                for k in 0..n {
                    rows.push(SideRow::Pair(dels.get(k).cloned(), adds.get(k).cloned()));
                }
            }
            DKind::Add => {
                rows.push(SideRow::Pair(None, Some(cell_of(&body[i], 1, offset + i))));
                i += 1;
            }
            _ => {
                rows.push(SideRow::Full(body[i].text.clone(), body[i].kind));
                i += 1;
            }
        }
    }
    rows
}

/// Build side-by-side rows + per-hunk row ranges (for current-hunk highlight).
pub fn side_rows(lines: &[DLine], hunks: &[usize]) -> (Vec<SideRow>, Vec<(usize, usize)>) {
    if hunks.is_empty() {
        let rows = lines
            .iter()
            .map(|dl| SideRow::Full(dl.text.clone(), dl.kind))
            .collect();
        return (rows, vec![]);
    }
    let mut rows: Vec<SideRow> = vec![];
    let mut ranges: Vec<(usize, usize)> = vec![];
    let mut cursor = 0;
    for (hi, &hstart) in hunks.iter().enumerate() {
        rows.extend(pair_hunk(&lines[cursor..hstart.min(lines.len())], cursor));
        let hend = hunks
            .get(hi + 1)
            .copied()
            .unwrap_or(lines.len())
            .min(lines.len());
        if hstart < lines.len() {
            rows.push(SideRow::Full(
                lines[hstart].text.clone(),
                lines[hstart].kind,
            ));
            let start = rows.len();
            rows.extend(pair_hunk(&lines[hstart + 1..hend], hstart + 1));
            ranges.push((start, rows.len()));
        }
        cursor = hend;
    }
    for dl in &lines[cursor..] {
        rows.push(SideRow::Full(dl.text.clone(), dl.kind));
    }
    (rows, ranges)
}

/// Char-index ranges of the changed middle between two paired lines.
/// Returns (a_start, a_end, b_start, b_end) as char counts.
pub fn emph_ranges(a: &str, b: &str) -> (usize, usize, usize, usize) {
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    let mut pre = 0;
    while pre < ac.len() && pre < bc.len() && ac[pre] == bc[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < ac.len() - pre
        && suf < bc.len() - pre
        && ac[ac.len() - 1 - suf] == bc[bc.len() - 1 - suf]
    {
        suf += 1;
    }
    (pre, ac.len() - suf, pre, bc.len() - suf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dl(text: &str, kind: DKind, old_no: Option<u32>, new_no: Option<u32>) -> DLine {
        DLine {
            text: text.into(),
            kind,
            old_no,
            new_no,
        }
    }

    #[test]
    fn pairs_del_add_runs_and_numbers() {
        let lines = vec![
            dl("diff --git a/f b/f", DKind::File, None, None),
            dl("@@ -1,3 +1,3 @@", DKind::Hunk, None, None),
            dl(" same", DKind::Ctx, Some(1), Some(1)),
            dl("-old1", DKind::Del, Some(2), None),
            dl("-old2", DKind::Del, Some(3), None),
            dl("+new1", DKind::Add, None, Some(2)),
        ];
        let (rows, ranges) = side_rows(&lines, &[1]);
        assert_eq!(rows.len(), 5);
        assert_eq!(ranges, vec![(2, 5)]);
        match &rows[2] {
            SideRow::Pair(Some(l), Some(r)) => {
                assert_eq!((l.no, r.no), (Some(1), Some(1)));
                assert_eq!((l.body.as_str(), r.body.as_str()), ("same", "same"));
            }
            _ => panic!("ctx should pair"),
        }
        match &rows[4] {
            SideRow::Pair(Some(l), None) => {
                assert_eq!(l.no, Some(3));
                assert_eq!(l.body, "old2");
            }
            _ => panic!("extra del should be single-sided"),
        }
    }

    #[test]
    fn emph_finds_changed_middle() {
        let (a0, a1, b0, b1) = emph_ranges("let foo = 1;", "let foo = 2;");
        assert_eq!((a0, b0), (10, 10));
        assert!(a1 > a0 && b1 > b0);
    }
}
