# freezy — Hunk-style Git review, in Rust

A read-only terminal diff viewer for one repository or a directory of sibling
repositories. The UI follows the installed Hunk workspace layout: Catppuccin
Mocha, a compact Source Control pane, menu dropdowns, a change overview ruler,
and a border-framed review stream. No activity rail, editor tabs, or status bar.

Split and stacked diffs include syntax coloring, full-row change backgrounds,
inline emphasis, current-line highlighting, Unicode-safe wrapping, and optional
line numbers and hunk metadata. File search and help use keyboard-accessible
modals. Every changed file remains available in the sidebar.

## Run

```bash
cargo build --release
./target/release/freezy [dir]         # a repo or parent of sibling repos
./target/release/freezy --scan [dir]  # headless changed-file listing
```

## Keys

| Key | Action |
|---|---|
| `n` / `p` · `.` / `,` | next / previous changed file |
| `j` / `k` · `↓` / `↑` | next / previous code line |
| `g` / `G` · Home / End | start / end of diff |
| `]` / `[` | next / previous hunk |
| `z` / click Show/Hide gap row | expand / collapse unchanged context near the selected hunk |
| `1` / `2` / `0` | split / stack / auto |
| `v` | toggle split / stack |
| `s` | show / hide sidebar |
| `l` / `w` / `m` / `M` | toggle line numbers / wrap / hunk metadata / menu bar |
| `/` / `o` / Tab | open the fuzzy file picker |
| ↑ / ↓ · Enter · Esc in picker | select · open · cancel and clear filter |
| F10 · arrows · Enter / Esc | open menus · navigate · activate / close |
| PgDown / Space / `f` · PgUp / `b` | page down / up |
| `d` / `u` (also Ctrl-d / Ctrl-u) | half-page down / up |
| ← / → · Shift+wheel | scroll horizontally (disables wrap) |
| `r` | reload files and current diff |
| `?` · Esc | open / close help (arrows or wheel scroll) |
| click / wheel | select files or code · use menus/ruler · scroll |
| `q` / Ctrl-C | quit |

The Agent and Extensions menus describe Freezy's capabilities; they do not
implement Hunk's agent-note system or load Hunk extensions. The theme is fixed
to the user's reference, Catppuccin Mocha.

## Shared UI component

`ui::Window` in `src/ui/window.rs` is the common shell for the sidebar, diff
viewer, menu bar, overview ruler, dropdowns, and dialogs. It owns the base
background, border styling, padding, and optional overlay clearing. Views use
composition: render a Window, then draw their content in the returned rectangle.

```rust
let body = Window::default()
    .borders(Borders::ALL)
    .padding(Padding::vertical(1))
    .overlay() // Use only for floating windows over existing content.
    .render(frame, area);
frame.render_widget(Paragraph::new("Window content"), body);
```

Change shared frame defaults in `Window::default()` and palette colors in
`src/theme.rs`. Content-specific styles such as diff highlights remain in their views.

## Performance and checks

Git runs in-process through libgit2 on background threads. Syntax highlighting
also runs off the UI thread, using Syntect with Mocha token colors. Paired rows
and wrapped terminal cells are cached; resizing or changing view settings
rebuilds the display cache. Stale file/diff results are discarded.

Unchanged context is retained from a full-context Git diff on the background
worker and folded by default. Each gap expands independently in split and stack
views, without changing the original hunk ranges. `m` only toggles metadata;
`z` toggles the context itself. Reloading or switching files resets expansion.

```bash
cargo test
cargo clippy -- -D warnings
```

Tests cover split/stack rendering, exact shell geometry and palette, file
groups and hit targets, wrapping, Unicode, hunk navigation, menus, filtering,
empty/loading states, stale results, and tiny terminal sizes.
