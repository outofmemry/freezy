# freezy — terminal Git review, in Rust

A read-only terminal diff viewer for one repository or a directory of sibling
repositories. The UI uses a LazyGit-inspired visual theme: fixed
black backgrounds, rounded frames, green focused borders, blue selections and hints,
and red/green change highlights. Freezy retains its own compact Source Control
pane, overview ruler, and review stream. The top workspace bar switches between Files,
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
| `1` / `2` / `3` / `4` · click workspace | open Files / Commits / Branch / Stash |
| `j` / `k` · `↓` / `↑` | next / previous file in the active sidebar, or code line in the active viewer |
| `g` / `G` · Home / End | start / end of diff |
| `]` / `[` | next / previous hunk |
| `z` / click Show/Hide gap row | expand / collapse unchanged context near the selected hunk |
| `+` / `-` · window `[-][+]` buttons | maximize / restore one window level |
| click a pane · F6 | choose the window to maximize |
| Ctrl-h / Ctrl-j / Ctrl-k / Ctrl-l | focus window left / down / up / right |
| `<` / `>` / `0` | stack / split / auto |
| `v` | toggle split / stack |
| `s` | show / hide sidebar |
| Shift-L / `w` / `m` | toggle line numbers (active viewer) / wrap / hunk metadata |
| `/` / `o` / Tab | open the fuzzy file picker |
| ↑ / ↓ · Enter · Esc in picker | select · open · cancel and clear filter |
| PgDown / Space / `f` · PgUp / `b` | page down / up |
| `d` / `u` (also Ctrl-d / Ctrl-u) | half-page down / up |
| `h` / `l` · ← / → | scroll the active viewer horizontally (disables wrap) |
| Shift+wheel | scroll code horizontally (disables wrap) |
| `r` | reload files and current diff |
| `?` · Esc | open / close help (arrows or wheel scroll) |
| click / wheel | select workspaces, files or code · use ruler · scroll |
| `q` / Ctrl-C | quit |

Plain h/j/k/l acts only in the focused window and never switches focus. The sidebar
uses j/k to select files and update their diff preview; h/l does nothing there.
In search these letters remain text input, and help handles its own scrolling.
Ctrl-h/j/k/l still switches window focus. Shift-L replaces the former l line-number toggle.
File selection, code navigation, and hunk navigation stop at the first/last item;
they never wrap around. Scrolling at the sidebar's edge does not reload the file
or reset its diff position.

The former File/View/Navigate menu bar has been removed. `?` opens the keyboard
controls. The styling is inspired by [LazyGit's defaults](https://github.com/jesseduffield/lazygit/blob/master/docs/Config.md#default):
Freezy's components, shortcuts, and workflows are unchanged. Chrome and syntax
use a fixed dark palette so your terminal theme cannot replace the black background
or recolor highlights. Modified files are yellow, additions green, and deletions
red. Code additions/deletions retain subtle red/green backgrounds and stronger
inline emphasis. Empty diff padding stays black.

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
windows. Click either column (or use Ctrl-h/l or F6), then press `+` or click
the viewer's `[+]` to hide the sidebar and expand the entire diff. Another `+`
does nothing: it never hides either diff column. Maximizing the sidebar instead
hides the whole viewer in one step. `-` or `[-]` restores the previous layout and
widths. Moving the mouse or scrolling does not switch window focus.

Hidden windows retain their data, and zoom does not change the saved sidebar or
split/stack options; Auto keeps its usual width-responsive layout. Changing the
diff layout (`<`, `>`, `0`, `v`) preserves maximization and hidden windows; `-`
still restores the previous window size. Toggling the sidebar (`s`) or switching
workspaces starts a fresh zoom sequence. Search and help overlays keep their
existing input behavior; typing digits in search does not switch workspaces.

Future actual windows can use a stable `WindowId` and register their rectangle
through `Window::controls`. Give related windows the same group ID, and exclude
hidden IDs from their layout. Ctrl-h/j/k/l uses those window rectangles to focus
the nearest visible window in that direction, without wrapping or reopening hidden
windows. Diff columns are not focus targets; with today's side-by-side windows,
Ctrl-j/k does nothing because no window is above or below. The shared `WindowZoom`
controller is independent of the current window count: it hides the nearest outside window before a peer
in the same group, restoring each step in reverse order. Internal content such
as diff columns does not need a window ID.

## Performance and checks

Git runs in-process through libgit2 on background threads. Syntax highlighting
also runs off the UI thread, using Syntect with shared-palette token colors. Paired rows
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
