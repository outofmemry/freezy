//! Application state: file catalog, selection, diff, view options.
//! Navigation only flips indices — git always loads on background threads.

use std::{cmp::Reverse, collections::HashSet, time::Instant};

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use ratatui::{layout::Rect, text::Line};

use crate::model::{side_rows, DKind, DLine, FileEntry, SideRow};

pub enum Msg {
    Files(u64, Vec<FileEntry>),
    Diff(u64, String, Vec<DLine>, Vec<usize>, Vec<Line<'static>>),
}

#[derive(Default)]
pub struct LayoutCache {
    pub side: Rect,
    pub diff: Rect,
    pub ruler: Rect,
    /// Sidebar window offset (manual windowing) — for click mapping.
    pub side_win: usize,
    pub menus: Vec<Rect>,
    pub menu_items: Rect,
    pub search_results: Rect,
}

pub struct App {
    pub files: Vec<FileEntry>,
    pub query: String,
    /// Cached fuzzy matches (indices into `files`); recomputed only when
    /// files or query change — never per frame.
    pub filtered: Vec<usize>,
    pub searching: bool,
    pub selected: usize, // index into visible()
    pub diff_lines: Vec<DLine>,
    pub syntax: Vec<Line<'static>>,
    pub hunks: Vec<usize>, // indices into diff_lines
    pub expanded_gaps: HashSet<usize>,
    pub gap_anchor: Option<(usize, usize)>, // source gap, screen row to preserve
    pub hunk_idx: usize,
    pub diff_scroll: usize, // target scroll (rows)
    pub cursor_row: usize,
    pub scroll_x: usize,  // horizontal scroll (columns)
    pub anim_scroll: f64, // eased render scroll — glides toward target
    pub diff_title: String,
    pub loading_diff: bool,
    pub side_by_side: bool,
    pub auto_layout: bool,
    pub show_sidebar: bool,
    pub show_menu_bar: bool,
    pub line_numbers: bool,
    pub wrap_lines: bool,
    pub hunk_headers: bool,
    pub menu: Option<usize>,
    pub menu_row: usize,
    pub show_help: bool,
    pub help_scroll: u16,
    pub scanning: bool,
    pub status: String,
    pub gen_files: u64,
    pub gen_diff: u64,
    pub matcher: SkimMatcherV2,
    pub last_refresh: Instant,
    pub frame: u64,
    pub root_name: String,
    /// Precomputed side-by-side rows + per-hunk row ranges. Rebuilt only
    /// when a diff loads — render just slices it.
    pub side_cache: (Vec<SideRow>, Vec<(usize, usize)>),
    pub layout: LayoutCache,
    pub display: crate::ui::diff::DiffView,
}

impl App {
    pub fn new(root_name: String) -> Self {
        Self {
            files: vec![],
            query: String::new(),
            filtered: vec![],
            searching: false,
            selected: 0,
            diff_lines: vec![],
            syntax: vec![],
            hunks: vec![],
            expanded_gaps: HashSet::new(),
            gap_anchor: None,
            hunk_idx: 0,
            diff_scroll: 0,
            cursor_row: 0,
            scroll_x: 0,
            anim_scroll: 0.0,
            diff_title: String::from("—"),
            loading_diff: false,
            side_by_side: true,
            auto_layout: true,
            show_sidebar: true,
            show_menu_bar: true,
            line_numbers: true,
            wrap_lines: true,
            hunk_headers: true,
            menu: None,
            menu_row: 0,
            show_help: false,
            help_scroll: 0,
            scanning: true,
            status: String::new(),
            gen_files: 0,
            gen_diff: 0,
            matcher: SkimMatcherV2::default(),
            last_refresh: Instant::now(),
            frame: 0,
            root_name,
            side_cache: (vec![], vec![]),
            layout: LayoutCache::default(),
            display: Default::default(),
        }
    }

    pub fn refresh_filter(&mut self) {
        if self.query.is_empty() {
            self.filtered.clear();
            return;
        }
        let qlower = self.query.to_lowercase();
        let mut scored: Vec<(i64, usize)> = vec![];
        for (i, f) in self.files.iter().enumerate() {
            if let Some(s) = self.matcher.fuzzy_match(&f.path, &self.query) {
                scored.push((s, i));
            } else if f.path.to_lowercase().contains(&qlower) {
                scored.push((0, i));
            }
        }
        scored.sort_by_key(|(score, _)| Reverse(*score));
        self.filtered = scored.into_iter().map(|(_, i)| i).collect();
    }

    /// Indices into `files` that match the current query, in order.
    pub fn visible(&self) -> Vec<usize> {
        if self.query.is_empty() {
            return (0..self.files.len()).collect();
        }
        self.filtered.clone()
    }

    pub fn selected_file(&self) -> Option<FileEntry> {
        let vis = self.visible();
        vis.get(self.selected)
            .and_then(|&i| self.files.get(i))
            .cloned()
    }

    /// Instant index flip. Never touches git — caller spawns the async diff load.
    pub fn move_file(&mut self, delta: isize) {
        let n = self.visible().len();
        if n == 0 {
            return;
        }
        let next = (self.selected as isize + delta).rem_euclid(n as isize) as usize;
        self.selected = next;
        self.hunk_idx = 0;
        self.diff_scroll = 0;
        self.scroll_x = 0;
        self.anim_scroll = 0.0;
    }

    /// Selected file's row index including repo headers.
    pub fn selected_row(&self) -> Option<usize> {
        let vis = self.visible();
        let sel_path = vis
            .get(self.selected)
            .and_then(|&i| self.files.get(i))
            .map(|f| f.path.clone())
            .unwrap_or_default();
        let mut row = 0usize;
        let mut found = None;
        let mut last_repo = String::new();
        for &i in &vis {
            let f = &self.files[i];
            if f.repo != last_repo {
                if !last_repo.is_empty() {
                    row += 1;
                }
                last_repo = f.repo.clone();
                row += 1; // header row
            }
            if f.path == sel_path {
                found = Some(row);
            }
            row += 1;
        }
        found
    }

    /// Map a sidebar inner-row (0-based, below the border) to a visible index.
    pub fn sidebar_hit(&self, y: usize) -> Option<usize> {
        let vis = self.visible();
        let mut row = 0usize;
        let mut last_repo = String::new();
        for (vi, &i) in vis.iter().enumerate() {
            let f = &self.files[i];
            if f.repo != last_repo {
                if !last_repo.is_empty() {
                    if row == y {
                        return None;
                    }
                    row += 1;
                }
                last_repo = f.repo.clone();
                if row == y {
                    return Some(vi); // header click → first file of repo
                }
                row += 1;
            }
            if row == y {
                return Some(vi);
            }
            row += 1;
        }
        None
    }

    pub fn move_hunk(&mut self, delta: isize) {
        if self.hunks.is_empty() {
            return;
        }
        let n = self.hunks.len() as isize;
        self.hunk_idx = (self.hunk_idx as isize + delta).rem_euclid(n) as usize;
        self.diff_scroll = self.hunk_scroll_target();
        self.cursor_row = self
            .display
            .code_rows
            .iter()
            .copied()
            .find(|&row| row >= self.diff_scroll)
            .unwrap_or(self.diff_scroll);
    }

    pub fn selected_gap(&self) -> Option<usize> {
        if self.loading_diff || self.scanning {
            return None;
        }
        if let Some((id, _)) = self
            .display
            .gaps
            .iter()
            .find(|(_, rows)| rows.contains(&self.cursor_row))
        {
            return Some(*id);
        }
        // Hunk's z command uses the selected hunk's leading gap, then the next
        // available leading gap, finally the file's trailing gap.
        let start = self
            .hunk_idx
            .checked_sub(1)
            .and_then(|i| self.hunks.get(i))
            .copied()
            .unwrap_or(0);
        self.diff_lines
            .iter()
            .enumerate()
            .skip(start)
            .find(|(_, line)| line.kind == DKind::Gap)
            .map(|(i, _)| i)
    }

    pub fn toggle_gap(&mut self, id: usize) {
        if self.loading_diff
            || self.scanning
            || !self
                .diff_lines
                .get(id)
                .is_some_and(|l| l.kind == DKind::Gap)
        {
            return;
        }
        let top = self.rendered_scroll();
        let offset = self
            .display
            .gaps
            .iter()
            .find(|(gap, _)| *gap == id)
            .map(|(_, rows)| rows.start)
            .filter(|&row| row >= top && row < top + self.layout.diff.height as usize)
            .map(|row| row - top)
            .unwrap_or(2);
        self.gap_anchor = Some((id, offset));
        if !self.expanded_gaps.insert(id) {
            self.expanded_gaps.remove(&id);
        }
        self.hunk_idx = self
            .hunks
            .partition_point(|&h| h < id)
            .min(self.hunks.len().saturating_sub(1));
        self.display.invalidate();
        self.status.clear();
    }

    pub fn move_cursor(&mut self, delta: isize) {
        let rows = &self.display.code_rows;
        if rows.is_empty() {
            return;
        }
        let current = rows
            .partition_point(|&row| row < self.cursor_row)
            .min(rows.len() - 1);
        let index = current.saturating_add_signed(delta).min(rows.len() - 1);
        self.cursor_row = rows[index];
        self.hunk_idx = self
            .display
            .hunks
            .partition_point(|&row| row <= self.cursor_row)
            .saturating_sub(1);
        let height = self.layout.diff.height.max(1) as usize;
        if self.cursor_row < self.diff_scroll {
            self.diff_scroll = self.cursor_row;
        } else if self.cursor_row >= self.diff_scroll + height {
            self.diff_scroll = self.cursor_row + 1 - height;
        }
    }

    /// Scroll target for the current hunk in the active view mode.
    pub fn hunk_scroll_target(&self) -> usize {
        if let Some(&row) = self.display.hunks.get(self.hunk_idx) {
            return row;
        }
        if self.side_by_side {
            self.side_cache
                .1
                .get(self.hunk_idx)
                .map(|(s, _)| *s)
                .unwrap_or(0)
        } else {
            self.hunks.get(self.hunk_idx).copied().unwrap_or(0)
        }
    }

    /// Same source-line projection for both the overview paint and its click targets.
    pub fn ruler_marks(&self, height: usize) -> Vec<Option<usize>> {
        let mut marks = vec![None; height];
        if height == 0 || self.loading_diff || self.scanning {
            return marks;
        }
        let last = self
            .diff_lines
            .iter()
            .filter_map(|l| l.new_no.or(l.old_no))
            .max()
            .unwrap_or(1)
            .max(1) as usize;
        for (hi, &start) in self.hunks.iter().enumerate() {
            let end = self
                .hunks
                .get(hi + 1)
                .copied()
                .unwrap_or(self.diff_lines.len());
            let mut numbers = self.diff_lines[start..end]
                .iter()
                .take_while(|l| l.kind != DKind::Gap)
                .filter_map(|l| l.new_no.or(l.old_no));
            let first = numbers.next().unwrap_or(1).saturating_sub(1) as usize;
            let final_line = numbers
                .last()
                .map_or(first, |n| n.saturating_sub(1) as usize);
            let y0 = (first * height / last).min(height - 1);
            let y1 = (final_line * height / last).min(height - 1).max(y0);
            marks[y0..=y1].fill(Some(hi));
        }
        marks
    }

    pub fn ruler_jump(&mut self, frac: f64) {
        let height = self.layout.ruler.height as usize;
        let y = ((frac.clamp(0.0, 1.0) * height as f64) as usize).min(height.saturating_sub(1));
        if let Some(Some(hunk)) = self.ruler_marks(height).get(y) {
            self.hunk_idx = *hunk;
            self.move_hunk(0);
        }
    }

    /// Ease the rendered scroll toward the target. Call once per frame.
    /// Returns true once settled (used to idle the render loop at ~0% CPU).
    pub fn ease_scroll(&mut self) -> bool {
        let target = self.diff_scroll as f64;
        let d = target - self.anim_scroll;
        if d.abs() < 0.15 {
            self.anim_scroll = target;
            true
        } else {
            self.anim_scroll += d * 0.35;
            false
        }
    }

    pub fn rendered_scroll(&self) -> usize {
        self.anim_scroll.round() as usize
    }

    pub fn set_files(&mut self, gen: u64, files: Vec<FileEntry>) {
        if gen != self.gen_files {
            return; // stale
        }
        // Preserve selection by path across refreshes.
        let cur = self.selected_file().map(|f| f.path);
        self.files = files;
        self.scanning = false;
        self.refresh_filter();
        let vis = self.visible();
        self.selected = cur
            .and_then(|p| vis.iter().position(|&i| self.files[i].path == p))
            .unwrap_or(0)
            .min(vis.len().saturating_sub(1));
        self.last_refresh = Instant::now();
    }

    pub fn set_diff(
        &mut self,
        gen: u64,
        title: String,
        lines: Vec<DLine>,
        hunks: Vec<usize>,
        syntax: Vec<Line<'static>>,
    ) {
        if gen != self.gen_diff {
            return; // stale — user already moved on
        }
        self.diff_title = title;
        self.diff_lines = lines;
        self.syntax = syntax;
        self.hunks = hunks;
        self.expanded_gaps.clear();
        self.gap_anchor = None;
        self.status.clear();
        self.hunk_idx = 0;
        self.diff_scroll = 0;
        self.scroll_x = 0;
        self.anim_scroll = 0.0;
        self.loading_diff = false;
        self.side_cache = side_rows(&self.diff_lines, &self.hunks);
        self.display = Default::default();
    }
}
