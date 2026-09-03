//! Feature views: tabs, sidebar, diff, ruler, search, help.

pub mod diff;
pub mod help;
pub mod ruler;
pub mod search;
pub mod sidebar;
pub mod tabs;

use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    widgets::{Borders, Padding, Paragraph},
};

use crate::{
    ui::primitives::window::Window,
    utils::theme::{clipped, BLUE},
};

/// Centered modal frame shared by the search and help overlays.
pub(super) fn modal(
    frame: &mut ratatui::Frame,
    area: Rect,
    width: u16,
    height: u16,
    title: &str,
    border: ratatui::style::Color,
) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    let rect = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let inside = Window::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border).add_modifier(Modifier::BOLD))
        .padding(Padding::vertical(1))
        .overlay()
        .render(frame, rect);
    let close_width = 7.min(inside.width);
    frame.render_widget(
        Paragraph::new(format!(
            " {}",
            clipped(title, inside.width.saturating_sub(close_width + 1) as usize)
        ))
        .style(Style::default().fg(border).add_modifier(Modifier::BOLD)),
        Rect::new(
            inside.x,
            inside.y,
            inside.width.saturating_sub(close_width),
            1.min(inside.height),
        ),
    );
    frame.render_widget(
        Paragraph::new("[Esc] ")
            .alignment(Alignment::Right)
            .style(Style::default().fg(BLUE)),
        Rect::new(
            inside.right().saturating_sub(close_width),
            inside.y,
            close_width,
            1.min(inside.height),
        ),
    );
    Rect::new(
        inside.x,
        inside.y + 2.min(inside.height),
        inside.width,
        inside.height.saturating_sub(2),
    )
}
