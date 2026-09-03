# Repository Guidelines

`freezy` is a read-only terminal Git review tool written in Rust (edition 2021).
It renders a LazyGit-inspired diff viewer for one repository or a directory of
sibling repositories, using Ratatui + Crossterm for the UI, libgit2 (`git2`
crate, zero subprocess spawns) for Git access, Tokio for async work, and
Syntect (`two-face` grammar bundle) for syntax highlighting.

## Project Structure & Module Organization

All source lives in `src/`; there are no workspace crates, no `tests/`
directory, and no static assets. Module map:

- `src/main.rs` — Binary entry point. Parses CLI args (`[dir]`, `--scan`),
  runs the headless `--scan` path, otherwise owns the Tokio runtime, the
  `Msg` channel, the render loop, and terminal setup/teardown (mouse capture,
  `ratatui::init`/`restore`).
- `src/app/mod.rs` — `App` state: file catalog, selection, query/filter cache,
  diff lines, syntax lines, hunks, gap expansion, scroll/cursor, view toggles,
  workspace tabs, `WindowZoom`, layout cache, generation counters. Pure
  index-flipping navigation helpers (`move_file`, `move_cursor`, `move_hunk`,
  `toggle_gap`, `ruler_jump`) live here.
- `src/core/mod.rs`, `src/core/model.rs` — Domain model: `FileEntry`
  (`path` is `"repo/rel"`, `kind` is `M`/`A`/`D`/`U`), `DLine`/`DKind`
  (`File`/`Hunk`/`Gap`/`Add`/`Del`/`Ctx`), side-by-side row pairing
  (`side_rows`, `pair_hunk`), inline-emphasis ranges (`emph_ranges`), and
  `with_context` (splices folded `Gap` rows into a compact diff using a
  full-context diff; returns `None` on worktree race so callers fall back).
- `src/git/mod.rs` — All libgit2 access: `discover_repos` (single repo vs.
  parent-of-repos, 2 s cache), `scan_all_files` (status + numstat per file,
  fans out over `std::thread::scope` when > 4 repos), `load_diff_text`
  (HEAD vs. index+workdir, staged-only fallback, untracked files read from
  disk). Synchronous functions — callers must run them on background threads.
- `src/input/mod.rs`, `src/input/keys.rs`, `src/input/mouse.rs` —
  `spawn_files` / `spawn_diff` (Tokio + `spawn_blocking` loaders that post
  `Msg::Files` / `Msg::Diff`), `load_selected` (bumps `gen_diff`, resets
  scroll, sets `loading_diff`), and key/mouse dispatch. Key bindings are
  defined in `keys.rs`; hit-testing helpers (`sidebar_hit`, `selected_row`,
  `ruler_marks`) live on `App`.
- `src/ui/mod.rs` — Top-level `render`: background fill, workspace tabs,
  Files-workspace column layout (sidebar 36 cols / max 1/3 width when
  terminal ≥ 60 wide, else hidden), empty-workspace placeholder for
  Commits/Branch/Stash, then search/help overlays.
- `src/ui/components/` — `diff.rs` (split/stack viewer, display cache),
  `sidebar.rs` (file list with repo headers), `tabs.rs` (workspace tabs),
  `ruler.rs` (overview ruler + click-to-jump), `search.rs` (fuzzy picker
  popup), `help.rs` (keybinding overlay).
- `src/ui/primitives/window.rs` — Shared `Window` shell (rounded borders,
  palette defaults, padding, overlay clearing) plus `WindowZoom`
  (maximize/restore stack, directional focus, click controls). See
  "UI Conventions" below.
- `src/ui/syntax.rs` — `highlight(lines, filename)`: extension/shebang
  detection against the bundled `two-face` set, mapped onto the fixed
  palette. Runs on the Git worker, once per diff — never during painting.
- `src/utils/mod.rs`, `src/utils/theme.rs` — Fixed dark palette constants
  (`BG`, `GREEN`, `RED`, `YELLOW`, `BLUE`, `ADD_BG`, `DEL_BG`, …), the
  Unicode-safe `clipped` cell truncator, and the loading `spinner`.
- `src/tests/` — Integration-style unit tests compiled into the binary
  (`#[cfg(test)] mod tests`): `mod.rs` (shared `fixture()`, `draw()`,
  `screen()`, `press()` helpers), `render.rs`, `navigation.rs`,
  `windows.rs`, `git.rs`. Unit tests also live next to code
  (`core/model.rs`, `ui/primitives/window.rs`).

Key paths to know: `src/ui/primitives/window.rs`, `src/utils/theme.rs`,
`src/app/mod.rs`, `src/git/mod.rs`, `src/tests/mod.rs`.

## Architecture Overview

Data flows one way: background Git/highlight workers → `Msg` channel →
`App` state → Ratatui views. The rules that keep the UI fast:

1. **Never block the render loop.** `main.rs` drains `rx.try_recv()`,
   eases scroll, renders only when dirty/busy, then polls input (16 ms
   while animating, 200 ms idle → ~0% CPU). Git and highlighting run via
   `spawn_files`/`spawn_diff` on `spawn_blocking` threads.
2. **Generations discard stale results.** `gen_files`/`gen_diff` bump on
   every request; `set_files`/`set_diff` ignore messages with an old
   generation. A 2 s periodic refresh re-scans files the same way. Preserve
   this pattern for any new background load.
3. **Precompute, don't recompute per frame.** `side_cache`
   (`side_rows` + hunk ranges), `filtered` fuzzy matches, and the diff
   display cache are rebuilt only when a diff/files/query change; `render`
   only slices them. The fuzzy filter caches on files/query change, never
   per frame.
4. **Context folding preserves hunk identity.** `git_diff_lines` takes a
   3-line compact diff plus a full-context diff and merges them with
   `with_context`, keeping Git's original hunk ranges while retaining
   omitted context as collapsed `Gap` rows. `z` toggles a gap, `m` toggles
   hunk-metadata display only. Reloading or switching files clears
   `expanded_gaps`.

## Build, Test, and Development Commands

```bash
cargo build --release              # release binary -> ./target/release/freezy
./target/release/freezy [dir]      # review a repo or a parent of sibling repos
./target/release/freezy --scan [dir]  # headless: print "KIND repo/rel +a -d" lines
cargo run -- [dir]                 # debug run without installing
cargo test                         # full suite (binary unit + src/tests/*)
cargo test <name>                  # e.g. cargo test zoom, cargo test pair
cargo clippy -- -D warnings        # required lint gate — must be clean
cargo fmt --check                  # verify formatting; `cargo fmt` to fix
```

`--scan` defaults to the current directory when `[dir]` is omitted and is
the fastest way to smoke-test Git scanning without launching the TUI. The
`target/` directory is gitignored build output; never commit it.

## Coding Style & Naming Conventions

- **Formatting/lints:** `cargo fmt` style (4-space indent, 100-col default);
  `cargo clippy -- -D warnings` must pass before any PR. Fix the lint, don't
  `#[allow]` it without a comment explaining why.
- **Naming:** `snake_case` for modules/functions/variables, `PascalCase`
  for types/traits, `SCREAMING_SNAKE_CASE` for constants. Test names read as
  `<area>_<scenario>_<expectation>`, e.g.
  `split_wrap_preserves_inline_emphasis`, `zoom_hides_nearest_outer_windows`.
- **Docs:** Every module starts with `//!` crate-docs explaining its role
  and its threading/rendering contract (see `git/mod.rs`, `ui/syntax.rs`).
  Keep them current when you change the contract.
- **Errors:** Use `anyhow::Result` at boundaries (`main`); internal Git
  helpers degrade gracefully (return empty vecs) rather than surfacing
  errors to the UI. Never `panic!`/`unwrap` on user or repo input — only on
  provably-static invariants (e.g. parsing a hardcoded scope string).
- **State placement:** Navigation mutates `App` indices only; I/O stays in
  `input` spawners and `git`. Don't add syscalls, FS reads, or Git calls to
  `app/`, `core/`, or `ui/` render paths.

## UI Conventions (Read Before Touching Views)

- **Compose `Window`, don't hand-roll chrome.** New panes render via
  `Window::default().borders(...).padding(...).render(frame, area)` and draw
  content in the returned rect; floating popups add `.overlay()` (which
  paints `Clear` first). Shared frame defaults change only in
  `Window::default()`; palette changes only in `src/utils/theme.rs`;
  pane-specific highlights stay in the view. Example from the README:

  ```rust
  let body = Window::default()
      .borders(Borders::ALL)
      .padding(Padding::vertical(1))
      .overlay() // Use only for floating windows over existing content.
      .render(frame, area);
  frame.render_widget(Paragraph::new("Window content"), body);
  ```

- **Fixed palette, always.** Backgrounds stay black (`BG`), modified files
  yellow, additions green, deletions red, selection blue. Never use the
  terminal theme's ANSI colors for chrome or diff highlights, so rendering
  is identical everywhere.
- **Zoom/focus protocol.** Whole panes get a `WindowId` and register via
  `Window::controls(frame, zoom, id, group, area)`; internal content such as
  old/new diff columns must NOT get IDs or register. `+`/`-` (or the
  `[-][+]` header buttons) call `zoom.maximize()`/`restore()`; `Ctrl-h/j/k/l`
  moves focus directionally without wrapping or unhiding; `s` and workspace
  switches call `zoom.reset()`. Hidden panes keep their data.
- **Text safety.** Clip with `theme::clipped` (cell widths, never bytes) so
  CJK/wide chars are never split; replace control chars (it maps them to
  `�`). Horizontal scrolling (`h`/`l`) disables wrapping for that viewer.
- **Input semantics.** Plain `h/j/k/l` acts in the focused pane only and
  never moves focus; `j/k` selection clamps at the ends (no wrap); scrolling
  at the sidebar edge must not reload the file or reset diff position;
  typing digits in search must not switch workspaces.

## Testing Guidelines

- **Frameworks:** only Rust's built-in test harness plus Ratatui's
  `TestBackend` for pixel-exact assertions — no external test crates.
  Rendering tests draw at fixed sizes and assert exact shell geometry,
  palette cells, and symbols (see `src/tests/render.rs` and the `window.rs`
  `controls_*` test).
- **Where tests go:** app-level behavior in `src/tests/<area>.rs` reusing
  the `fixture()`/`draw()`/`screen()`/`press()` helpers in
  `src/tests/mod.rs`; pure-logic units (pairing, emphasis, zoom) next to
  the code under `#[cfg(test)]`.
- **Coverage expectations:** every change should extend tests for split AND
  stack rendering, wrapping/Unicode, hunk navigation, filtering,
  empty/loading states, stale-generation drops, tiny terminal sizes, and
  zoom levels. The README's checklist is the definition of done — consult it.
- **Running:** `cargo test` for everything; `cargo test <filter>` while
  iterating. If a rendering test fails, print `screen(&buffer)` actuals
  before guessing.

## Commit & Pull Request Guidelines

- **History style** (from `git log --oneline`): `type: subject`, lowercase
  type, short imperative subject — e.g. `feat: Windows maximizing`,
  `fix: Maximizing fix while changes view`, `feat: syntax higlighting for
  opened files`, `chore : checkpoint 1 (folder restructure)`. Keep the
  `type:` prefix; multi-repo or structural checkpoints use `chore:`.
- **PR requirements:** describe the behavior change and affected views;
  link the related issue; list verification actually run (`cargo test`,
  `cargo clippy -- -D warnings`, `cargo fmt --check`, plus manual TUI
  checks with terminal size noted); attach before/after terminal screenshots
  or `--scan` output for any visual change. One concern per PR — don't mix a
  UI revamp with structural refactors.

## Security & Configuration Tips

- The tool is **read-only by design**: it only reads Git state and worktree
  files. Do not add commands that write to repos, stage changes, or execute
  hooks/subprocesses — `git/mod.rs` exists precisely to avoid subprocess
  spawning.
- No config files, env vars, or network access: syntax grammars are bundled
  via `two-face` (no runtime downloads, no language servers). Keep it that
  way; adding a network or subprocess dependency needs explicit justification.
- `scan_all_files` walks one directory level for `.git` children — beware
  symlink loops and giant directories if you touch discovery. Untrusted
  filenames go through `clipped` before display; non-UTF8 diff bytes use
  `String::from_utf8_lossy`.

## Agent-Specific Instructions

- Prefer `muse.search` over broad `grep -r`; scope scans to `src/` and never
  point recursive scans at the workspace root (large `target/` tree).
- Run the three gates before finishing: `cargo test`,
  `cargo clippy -- -D warnings`, `cargo fmt --check`. A clean-looking patch
  you never compiled is not done.
- Verify TUI changes with `TestBackend` tests or `--scan`, not by launching
  an interactive session. Keep changes minimal and consistent with the
  module contracts above.
- Leave `target/` and other build output alone; don't commit it, and don't
  delete untracked user files to tidy the tree.
