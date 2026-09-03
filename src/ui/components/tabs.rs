//! Workspace tabs across the top of the window.

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Borders, Paragraph},
};

use crate::ui::primitives::window::Window;

use crate::{
    app::{App, Workspace},
    utils::theme::{ACCENT, MUTED, PANEL},
};

pub fn render_workspace_tabs(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let body = Window::default()
        .borders(Borders::BOTTOM)
        .render(frame, area);
    let tabs = Layout::horizontal([Constraint::Ratio(1, 4); 4]).split(body);
    for (index, (&workspace, &rect)) in Workspace::ALL.iter().zip(tabs.iter()).enumerate() {
        app.layout
            .workspace_tabs
            .push(Rect::new(rect.x, area.y, rect.width, area.height));
        let active = app.workspace == workspace;
        frame.render_widget(
            Paragraph::new(format!("{}. {}", index + 1, workspace.label()))
                .alignment(Alignment::Center)
                .style(
                    Style::default()
                        .fg(if active { ACCENT } else { MUTED })
                        .bg(PANEL)
                        .add_modifier(if active {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
            rect,
        );
        if active && area.height > 0 {
            let y = area.bottom().saturating_sub(1);
            for x in rect.x..rect.right() {
                frame.buffer_mut()[(x, y)]
                    .set_symbol("─")
                    .set_fg(ACCENT)
                    .set_bg(PANEL);
            }
        }
    }
}
