//! Overview ruler: one row per screen line marking hunk positions.

use ratatui::layout::Rect;

use crate::ui::primitives::window::Window;

use crate::{
    app::App,
    utils::theme::{CYAN, GREEN},
};

pub fn render_ruler(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let area = Window::default().render(frame, area);
    for (y, hunk) in app
        .ruler_marks(area.height as usize)
        .into_iter()
        .enumerate()
    {
        if let Some(hunk) = hunk {
            Window::default()
                .background(if hunk == app.hunk_idx { CYAN } else { GREEN })
                .render(frame, Rect::new(area.x, area.y + y as u16, area.width, 1));
        }
    }
}
