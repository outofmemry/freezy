//! Shared shell for every UI surface. Views render their content in the returned area.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Padding},
    Frame,
};

use crate::theme::{BORDER, PANEL};

pub struct Window {
    block: Block<'static>,
    overlay: bool,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            block: Block::default()
                .border_style(Style::default().fg(BORDER))
                .style(Style::default().bg(PANEL)),
            overlay: false,
        }
    }
}

impl Window {
    pub fn borders(mut self, borders: Borders) -> Self {
        self.block = self.block.borders(borders);
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.block = self.block.style(Style::default().bg(color));
        self
    }

    pub fn padding(mut self, padding: Padding) -> Self {
        self.block = self.block.padding(padding);
        self
    }

    pub fn overlay(mut self) -> Self {
        self.overlay = true;
        self
    }

    /// Paint the shell and return its content area, excluding borders and padding.
    pub fn render(self, frame: &mut Frame, area: Rect) -> Rect {
        let area = area.intersection(frame.area());
        let content = self.block.inner(area);
        if self.overlay {
            frame.render_widget(Clear, area);
        }
        frame.render_widget(self.block, area);
        content
    }
}
