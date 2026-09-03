//! Shared shell for every UI surface. Views render their content in the returned area.

use ratatui::{
    layout::{Position, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Clear, Padding},
    Frame,
};

use crate::utils::theme::{ACCENT, BORDER, MUTED, PANEL, TEXT};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Identity of a zoomable window, never of content such as an old/new diff column.
pub struct WindowId(pub &'static str);

pub const SIDEBAR: WindowId = WindowId("sidebar");
pub const REVIEW: WindowId = WindowId("review");

#[derive(Clone, Copy, Debug)]
pub struct WindowRegion {
    pub id: WindowId,
    pub group: WindowId,
    pub area: Rect,
    pub restore: Rect,
    pub maximize: Rect,
}

pub struct WindowZoom {
    pub focused: WindowId,
    pub regions: Vec<WindowRegion>,
    hidden: Vec<WindowId>,
}

impl Default for WindowZoom {
    fn default() -> Self {
        Self {
            focused: REVIEW,
            regions: Vec::new(),
            hidden: Vec::new(),
        }
    }
}

impl WindowZoom {
    pub fn begin_frame(&mut self) {
        self.regions.clear();
    }

    pub fn is_hidden(&self, id: WindowId) -> bool {
        self.hidden.contains(&id)
    }

    pub fn reset(&mut self) {
        self.hidden.clear();
    }

    #[cfg(test)]
    pub fn level(&self) -> usize {
        self.hidden.len()
    }

    fn visible(&self, region: &WindowRegion) -> bool {
        !region.area.is_empty() && !self.is_hidden(region.id)
    }

    pub fn ensure_focus(&mut self) {
        if self
            .regions
            .iter()
            .any(|region| region.id == self.focused && self.visible(region))
        {
            return;
        }
        if let Some(region) = self
            .regions
            .iter()
            .filter(|region| self.visible(region))
            .min_by_key(|region| region.id != REVIEW)
        {
            self.focused = region.id;
        }
    }

    /// Hide outer panes nearest-first, then peers in the focused pane's group.
    pub fn maximize(&mut self) -> bool {
        self.ensure_focus();
        let Some(focused) = self
            .regions
            .iter()
            .position(|region| region.id == self.focused && self.visible(region))
        else {
            return false;
        };
        let candidate = self
            .regions
            .iter()
            .enumerate()
            .filter(|(_, region)| region.id != self.focused && self.visible(region))
            .min_by_key(|(index, region)| {
                (
                    region.group == self.regions[focused].group,
                    index.abs_diff(focused),
                    *index > focused,
                )
            })
            .map(|(_, region)| region.id);
        if let Some(id) = candidate {
            self.hidden.push(id);
            true
        } else {
            false
        }
    }

    pub fn restore(&mut self) -> bool {
        self.hidden.pop().is_some()
    }

    pub fn focus_at(&mut self, x: u16, y: u16) {
        if let Some(region) = self
            .regions
            .iter()
            .rev()
            .find(|region| self.visible(region) && region.area.contains(Position::new(x, y)))
        {
            self.focused = region.id;
        }
    }

    /// Focus the nearest visible window in a direction, without wrapping or restoring it.
    pub fn focus_direction(&mut self, direction: char) {
        self.ensure_focus();
        let Some(from) = self
            .regions
            .iter()
            .find(|region| region.id == self.focused && self.visible(region))
            .map(|region| region.area)
        else {
            return;
        };
        let horizontal = matches!(direction, 'h' | 'l');
        let bounds = |rect: Rect| {
            if horizontal {
                (rect.x, rect.right(), rect.y, rect.bottom())
            } else {
                (rect.y, rect.bottom(), rect.x, rect.right())
            }
        };
        let (start, end, cross_start, cross_end) = bounds(from);
        let target = self
            .regions
            .iter()
            .filter(|region| region.id != self.focused && self.visible(region))
            .filter_map(|region| {
                let (near, far, cross_near, cross_far) = bounds(region.area);
                let gap = match direction {
                    'h' | 'k' if far <= start => start - far,
                    'j' | 'l' if near >= end => near - end,
                    _ => return None,
                };
                let cross_gap = cross_start
                    .saturating_sub(cross_far)
                    .max(cross_near.saturating_sub(cross_end));
                let alignment = (u32::from(cross_start) + u32::from(cross_end))
                    .abs_diff(u32::from(cross_near) + u32::from(cross_far));
                Some(((cross_gap, gap, alignment), region.id))
            })
            .min_by_key(|(distance, _)| *distance);
        if let Some((_, id)) = target {
            self.focused = id;
        }
    }

    pub fn control_at(&self, x: u16, y: u16) -> Option<(WindowId, bool)> {
        let position = Position::new(x, y);
        self.regions
            .iter()
            .rev()
            .filter(|region| self.visible(region))
            .find_map(|region| {
                if region.maximize.contains(position) {
                    Some((region.id, true))
                } else if region.restore.contains(position) {
                    Some((region.id, false))
                } else {
                    None
                }
            })
    }
}

pub struct Window {
    block: Block<'static>,
    overlay: bool,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            block: Block::default()
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER))
                .style(Style::default().fg(TEXT).bg(PANEL)),
            overlay: false,
        }
    }
}

impl Window {
    /// Register a whole window and paint one control pair over its header background.
    pub fn controls(
        frame: &mut Frame,
        zoom: &mut WindowZoom,
        id: WindowId,
        group: WindowId,
        area: Rect,
    ) {
        let area = area.intersection(frame.area());
        if area.is_empty() || zoom.is_hidden(id) {
            return;
        }
        // Keep both rounded frame corners intact.
        let (restore, maximize) = if area.width >= 8 {
            (
                Rect::new(area.right() - 7, area.y, 3, 1),
                Rect::new(area.right() - 4, area.y, 3, 1),
            )
        } else {
            (Rect::default(), Rect::default())
        };
        zoom.regions.push(WindowRegion {
            id,
            group,
            area,
            restore,
            maximize,
        });
        if !restore.is_empty() {
            let color = if zoom.focused == id { ACCENT } else { MUTED };
            for (offset, symbol) in ["[", "-", "]", "[", "+", "]"].into_iter().enumerate() {
                frame.buffer_mut()[(restore.x + offset as u16, area.y)]
                    .set_symbol(symbol)
                    .set_fg(color)
                    .set_style(Style::default().add_modifier(if zoom.focused == id {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }));
            }
        }
    }

    pub fn borders(mut self, borders: Borders) -> Self {
        self.block = self.block.borders(borders);
        self
    }

    pub fn border_style(mut self, style: Style) -> Self {
        self.block = self.block.border_style(style);
        self
    }

    pub fn focused(self, focused: bool) -> Self {
        self.border_style(
            Style::default()
                .fg(if focused { ACCENT } else { BORDER })
                .add_modifier(if focused {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn zoom_hides_nearest_outer_windows_then_peer_and_restores_in_reverse() {
        let ids = [WindowId("1"), WindowId("2"), WindowId("3"), WindowId("4")];
        let regions: Vec<_> = ids
            .into_iter()
            .enumerate()
            .map(|(index, id)| WindowRegion {
                id,
                group: if index < 2 { id } else { REVIEW },
                area: Rect::new(index as u16 * 10, 0, 10, 10),
                restore: Rect::default(),
                maximize: Rect::default(),
            })
            .collect();
        let mut zoom = WindowZoom {
            focused: ids[2],
            ..Default::default()
        };
        for (level, hidden) in [ids[1], ids[0], ids[3]].into_iter().enumerate() {
            zoom.begin_frame();
            zoom.regions = regions.clone();
            assert!(zoom.maximize());
            assert!(zoom.is_hidden(hidden));
            assert_eq!(zoom.level(), level + 1);
            assert_eq!(zoom.focused, ids[2]);
        }
        assert!(!zoom.maximize());
        zoom.focus_at(1, 1); // Hidden windows cannot steal focus.
        assert_eq!(zoom.focused, ids[2]);
        for restored in [ids[3], ids[0], ids[1]] {
            assert!(zoom.restore());
            assert!(!zoom.is_hidden(restored));
        }
        assert!(!zoom.restore());
        zoom.focus_at(11, 1);
        assert_eq!(zoom.focused, ids[1]);
        assert!(zoom.maximize());
        assert!(zoom.is_hidden(ids[0])); // Equal distance prefers the preceding pane.
        zoom.reset();
        assert_eq!(zoom.level(), 0);
        assert_eq!(zoom.focused, ids[1]);
        zoom.begin_frame();
        assert!(!zoom.maximize());
    }

    #[test]
    fn controls_preserve_background_clip_and_ignore_tiny_or_hidden_windows() {
        let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();
        let mut zoom = WindowZoom::default();
        terminal
            .draw(|frame| {
                let area = frame.area();
                Window::default().render(frame, area);
                Window::controls(frame, &mut zoom, SIDEBAR, SIDEBAR, Rect::new(0, 0, 5, 4));
                Window::controls(frame, &mut zoom, REVIEW, REVIEW, Rect::new(6, 0, 20, 4));
                Window::controls(
                    frame,
                    &mut zoom,
                    WindowId("offscreen"),
                    REVIEW,
                    Rect::new(20, 0, 8, 4),
                );
            })
            .unwrap();
        assert_eq!(zoom.regions.len(), 2);
        assert_eq!(zoom.control_at(14, 0), Some((REVIEW, false)));
        assert_eq!(zoom.control_at(18, 0), Some((REVIEW, true)));
        assert_eq!(zoom.control_at(19, 0), None); // Right frame corner is never a button.
        assert_eq!(zoom.control_at(4, 0), None);
        assert_eq!(zoom.control_at(19, 1), None);
        let buffer = terminal.backend().buffer();
        for (offset, symbol) in ["[", "-", "]", "[", "+", "]"].into_iter().enumerate() {
            let cell = &buffer[(13 + offset as u16, 0)];
            assert_eq!(cell.symbol(), symbol);
            assert_eq!(cell.bg, PANEL);
            assert_eq!(cell.fg, ACCENT);
        }
        zoom.focused = WindowId("offscreen");
        zoom.ensure_focus();
        assert_eq!(zoom.focused, REVIEW);
        assert!(zoom.maximize());
        zoom.begin_frame();
        terminal
            .draw(|frame| {
                Window::controls(frame, &mut zoom, SIDEBAR, SIDEBAR, Rect::new(0, 0, 10, 4));
            })
            .unwrap();
        assert!(zoom.regions.is_empty());
    }
}
