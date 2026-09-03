//! Git access, fully in-process via libgit2 (zero subprocess spawns).
//! Everything here runs on background threads; the render loop never waits.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use git2::{Diff, DiffOptions, Patch, Repository, Status, StatusOptions};

use crate::model::{DKind, DLine, FileEntry};

/// Repo paths barely change — cache 2s so per-file diffs skip readdir.
type Repos = Vec<(String, PathBuf)>;
static REPOS_CACHE: Mutex<Option<(PathBuf, Instant, Repos)>> = Mutex::new(None);
const REPOS_TTL: Duration = Duration::from_secs(2);

pub fn discover_repos(root: &Path) -> Vec<(String, PathBuf)> {
    if let Ok(cache) = REPOS_CACHE.lock() {
        if let Some((p, at, repos)) = cache.as_ref() {
            if p == root && at.elapsed() < REPOS_TTL {
                return repos.clone();
            }
        }
    }
    let repos = discover_inner(root);
    if let Ok(mut cache) = REPOS_CACHE.lock() {
        *cache = Some((root.to_path_buf(), Instant::now(), repos.clone()));
    }
    repos
}

fn discover_inner(root: &Path) -> Vec<(String, PathBuf)> {
    let mut out = vec![];
    // Single repo opened directly?
    if root.join(".git").exists() {
        let name = root
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "repo".into());
        out.push((name, root.to_path_buf()));
        return out;
    }
    let rd = match std::fs::read_dir(root) {
        Ok(rd) => rd,
        Err(_) => return out,
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() && p.join(".git").exists() {
            if let Some(n) = p.file_name().and_then(|s| s.to_str()) {
                out.push((n.to_owned(), p));
            }
        }
    }
    out.sort();
    out
}

fn status_kind(s: Status) -> char {
    const INDEX_ANY: Status = Status::INDEX_NEW
        .union(Status::INDEX_MODIFIED)
        .union(Status::INDEX_DELETED)
        .union(Status::INDEX_RENAMED)
        .union(Status::INDEX_TYPECHANGE);
    if s.contains(Status::WT_NEW) && !s.intersects(INDEX_ANY) {
        'U'
    } else if s.contains(Status::INDEX_NEW) {
        'A'
    } else if s.contains(Status::INDEX_DELETED) || s.contains(Status::WT_DELETED) {
        'D'
    } else {
        'M'
    }
}

/// One diff (HEAD vs index+workdir) gives per-file +/− for the whole repo.
fn repo_numstat(repo: &Repository) -> HashMap<String, (u32, u32)> {
    let mut counts = HashMap::new();
    let head = repo.head().ok().and_then(|r| r.peel_to_tree().ok());
    let mut opts = DiffOptions::new();
    opts.show_binary(false);
    let diff = match repo.diff_tree_to_workdir_with_index(head.as_ref(), Some(&mut opts)) {
        Ok(d) => d,
        Err(_) => return counts,
    };
    for i in 0..diff.deltas().len() {
        let patch = match Patch::from_diff(&diff, i) {
            Ok(Some(p)) => p,
            _ => continue,
        };
        let delta = patch.delta();
        let path = delta
            .new_file()
            .path()
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(|| {
                let p = delta.old_file().path()?;
                if p.as_os_str().is_empty() {
                    None
                } else {
                    Some(p)
                }
            });
        if let Ok((_ctx, adds, dels)) = patch.line_stats() {
            if let Some(p) = path {
                counts.insert(p.to_string_lossy().into_owned(), (adds as u32, dels as u32));
            }
        }
    }
    counts
}

fn scan_repo(name: String, dir: PathBuf) -> Vec<FileEntry> {
    let repo = match Repository::open(&dir) {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let mut sopts = StatusOptions::new();
    sopts
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);
    let statuses = match repo.statuses(Some(&mut sopts)) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let counts = repo_numstat(&repo);
    let mut files = Vec::with_capacity(statuses.len());
    for e in statuses.iter() {
        let rel = match e.path() {
            Some(p) => p.to_owned(),
            None => continue,
        };
        let kind = status_kind(e.status());
        let (a, d) = counts.get(&rel).copied().unwrap_or((0, 0));
        files.push(FileEntry {
            path: format!("{name}/{rel}"),
            repo: name.clone(),
            rel,
            kind,
            additions: if kind == 'D' { 0 } else { a },
            deletions: if kind == 'A' || kind == 'U' { 0 } else { d },
        });
    }
    files
}

pub fn scan_all_files(root: &Path) -> Vec<FileEntry> {
    let mut all = vec![];
    let repos = discover_repos(root);
    if repos.len() > 4 {
        // Many repos: fan out over blocking threads.
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::scope(|s| {
            for (name, dir) in &repos {
                let tx = tx.clone();
                s.spawn(move || {
                    let _ = tx.send(scan_repo(name.clone(), dir.clone()));
                });
            }
        });
        drop(tx);
        for v in rx {
            all.extend(v);
        }
    } else {
        for (name, dir) in &repos {
            all.extend(scan_repo(name.clone(), dir.clone()));
        }
    }
    all.sort_by(|a, b| a.path.cmp(&b.path));
    all
}

fn push_patch_lines(diff: &Diff, lines: &mut Vec<DLine>, hunks: &mut Vec<usize>) {
    for i in 0..diff.deltas().len() {
        let patch = match Patch::from_diff(diff, i) {
            Ok(Some(p)) => p,
            _ => continue,
        };
        let delta = patch.delta();
        let disp = delta
            .new_file()
            .path()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.to_string_lossy().into_owned())
            .or_else(|| {
                delta
                    .old_file()
                    .path()
                    .map(|p| p.to_string_lossy().into_owned())
            })
            .unwrap_or_default();
        lines.push(DLine::plain(
            format!("diff --git a/{disp} b/{disp}"),
            DKind::File,
        ));
        for h in 0..patch.num_hunks() {
            let (hinfo, nlines) = match patch.hunk(h) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let mut o = hinfo.old_start();
            let mut n = hinfo.new_start();
            hunks.push(lines.len());
            lines.push(DLine::plain(
                format!(
                    "@@ -{},{} +{},{} @@",
                    hinfo.old_start(),
                    hinfo.old_lines(),
                    hinfo.new_start(),
                    hinfo.new_lines()
                ),
                DKind::Hunk,
            ));
            for l in 0..nlines {
                let dl = match patch.line_in_hunk(h, l) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let body = String::from_utf8_lossy(dl.content());
                let body = body.strip_suffix('\n').unwrap_or(&body);
                match dl.origin() {
                    '+' => {
                        lines.push(DLine {
                            text: format!("+{body}"),
                            kind: DKind::Add,
                            old_no: None,
                            new_no: Some(n),
                        });
                        n += 1;
                    }
                    '-' => {
                        lines.push(DLine {
                            text: format!("-{body}"),
                            kind: DKind::Del,
                            old_no: Some(o),
                            new_no: None,
                        });
                        o += 1;
                    }
                    ' ' => {
                        lines.push(DLine {
                            text: format!(" {body}"),
                            kind: DKind::Ctx,
                            old_no: Some(o),
                            new_no: Some(n),
                        });
                        o += 1;
                        n += 1;
                    }
                    _ => lines.push(DLine::plain(body.to_owned(), DKind::Ctx)),
                }
            }
        }
    }
}

/// Full unified diff for one file, in-process. No subprocess, ~1ms.
fn git_diff_lines(dir: &Path, rel: &str) -> (Vec<DLine>, Vec<usize>) {
    let (lines, hunks) = git_diff_lines_with_context(dir, rel, 3);
    if hunks.is_empty() {
        return (lines, hunks);
    }
    let (full, _) = git_diff_lines_with_context(dir, rel, u32::MAX);
    crate::model::with_context(&lines, &full).unwrap_or((lines, hunks))
}

fn git_diff_lines_with_context(dir: &Path, rel: &str, context: u32) -> (Vec<DLine>, Vec<usize>) {
    let repo = match Repository::open(dir) {
        Ok(r) => r,
        Err(_) => return (vec![], vec![]),
    };
    let head = repo.head().ok().and_then(|r| r.peel_to_tree().ok());
    let mut opts = DiffOptions::new();
    opts.pathspec(rel).show_binary(false).context_lines(context);
    let mut lines = vec![];
    let mut hunks = vec![];
    // Staged + unstaged vs HEAD in one shot.
    if let Ok(diff) = repo.diff_tree_to_workdir_with_index(head.as_ref(), Some(&mut opts)) {
        push_patch_lines(&diff, &mut lines, &mut hunks);
    }
    // Staged-only fallback (nothing in workdir diff).
    if lines.is_empty() {
        let mut opts2 = DiffOptions::new();
        opts2
            .pathspec(rel)
            .show_binary(false)
            .context_lines(context);
        if let Ok(diff) = repo.diff_tree_to_index(head.as_ref(), None, Some(&mut opts2)) {
            push_patch_lines(&diff, &mut lines, &mut hunks);
        }
    }
    (lines, hunks)
}

pub fn load_diff_text(root: &Path, f: &FileEntry) -> (Vec<DLine>, Vec<usize>) {
    let repos = discover_repos(root);
    let dir = repos
        .iter()
        .find(|(n, _)| n == &f.repo)
        .map(|(_, d)| d.clone())
        .unwrap_or_else(|| root.to_path_buf());
    if f.kind == 'U' {
        match std::fs::read_to_string(dir.join(&f.rel)) {
            Ok(s) => {
                let mut lines = vec![DLine::plain(format!("new file: {}", f.path), DKind::File)];
                lines.extend(s.lines().enumerate().map(|(i, l)| DLine {
                    text: format!("+{l}"),
                    kind: DKind::Add,
                    old_no: None,
                    new_no: Some(i as u32 + 1),
                }));
                (lines, vec![0])
            }
            Err(_) => (vec![], vec![]),
        }
    } else {
        git_diff_lines(&dir, &f.rel)
    }
}
