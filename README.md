# freezy — Hunk-style Git review, in Rust

A read-only terminal diff viewer for one repository or a directory of sibling
repositories. The UI follows the installed Hunk workspace layout: Catppuccin
Mocha, a compact Source Control pane, a change overview ruler,
and a border-framed review stream. The top workspace bar switches between Files,
Commits, Branch, and Stash; Files contains the current review UI, while the other
three workspaces are intentionally empty until their Git views are implemented.

Split and stacked diffs include syntax coloring, full-row change backgrounds,
inline emphasis, current-line highlighting, Unicode-safe wrapping, and optional
line numbers and hunk metadata. File search and help use keyboard-accessible
modals. Every changed file remains available in the sidebar.

Syntax highlighting uses the bundled [two-face grammar collection](https://docs.rs/two-face/latest/two_face/syntax/),
curated by `bat`: JavaScript/JSX, TypeScript/TSX (including `.mjs`, `.cjs`, `.mts`,
and `.cts`), Python, Rust, Go, Java, C/C++, C#, Ruby, PHP, Swift, Kotlin, Dart,
Vue, Svelte, HTML/CSS, SQL, shell, and many more. It recognizes filenames such as
`Dockerfile`, `Makefile`, and `.env.local`, compound extensions, and shebangs when
the first source line is available. Unsupported formats remain readable as plain
text. No runtime grammar downloads or language servers are needed.

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
| `1` / `2` / `3` / `4` · click workspace | open Files / Commits / Branch / Stash |
| `j` / `k` · `↓` / `↑` | next / previous code line |
| `g` / `G` · Home / End | start / end of diff |
| `]` / `[` | next / previous hunk |
| `z` / click Show/Hide gap row | expand / collapse unchanged context near the selected hunk |
| `+` / `-` · window `[-][+]` buttons | maximize / restore one window level |
| hover / click a pane · F6 | choose the window to maximize |
| `<` / `>` / `0` | stack / split / auto |
| `v` | toggle split / stack |
| `s` | show / hide sidebar |
| `l` / `w` / `m` | toggle line numbers / wrap / hunk metadata |
| `/` / `o` / Tab | open the fuzzy file picker |
| ↑ / ↓ · Enter · Esc in picker | select · open · cancel and clear filter |
| PgDown / Space / `f` · PgUp / `b` | page down / up |
| `d` / `u` (also Ctrl-d / Ctrl-u) | half-page down / up |
| ← / → · Shift+wheel | scroll horizontally (disables wrap) |
| `r` | reload files and current diff |
| `?` · Esc | open / close help (arrows or wheel scroll) |
| click / wheel | select workspaces, files or code · use ruler · scroll |
| `q` / Ctrl-C | quit |

The former File/View/Navigate menu bar has been removed. The theme remains fixed
to the user's reference, Catppuccin Mocha. `?` opens the keyboard controls.

## Shared UI component

`ui::Window` in `src/ui/window.rs` is the common shell for the sidebar, diff
viewer, workspace tabs, empty workspaces, overview ruler, and dialogs. It owns the base
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

### Level-wise maximize

Freezy currently has **two zoomable windows**: the sidebar and the whole file
viewer. The old/new diff columns are content inside the viewer, not separate
windows. Hover either column (or cycle windows with F6), then press `+` or click
the viewer's `[+]` to hide the sidebar and expand the entire diff. Another `+`
does nothing: it never hides either diff column. Maximizing the sidebar instead
hides the whole viewer in one step. `-` or `[-]` restores the previous layout and
widths.

Hidden windows retain their data, and zoom does not change the saved sidebar or
split/stack options; Auto keeps its usual width-responsive layout. Explicit
layout commands (`<`, `>`, `0`, `v`, `s`) start a fresh zoom sequence. Search and
help overlays keep their existing input behavior; typing digits in search does
not switch workspaces.

Future actual windows can use a stable `WindowId` and register their rectangle
through `Window::controls`. Give related windows the same group ID, and exclude
hidden IDs from their layout. The shared `WindowZoom` controller is independent
of the current window count: it hides the nearest outside window before a peer
in the same group, restoring each step in reverse order. Internal content such
as diff columns does not need a window ID.

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
groups and hit targets, wrapping, Unicode, hunk navigation, workspace switching, filtering,
empty/loading states, stale results, tiny terminal sizes, and level-wise window zoom.
