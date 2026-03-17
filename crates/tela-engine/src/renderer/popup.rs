use ratatui::layout::Rect;
use ratatui::widgets::Clear;
use ratatui::Frame;

use super::render_element;
use crate::elements::Element;

/// Render children as a floating popup overlay.
/// Clears the area first, then renders children on top.
/// Props: x, y, width, height (all in terminal cells, relative to frame origin).
/// If `anchor="bottom"` is set, y is measured from the bottom of the frame.
pub fn render(frame: &mut Frame, _area: Rect, element: &Element) {
    let frame_area = frame.area();

    let width = element
        .props
        .get("width")
        .and_then(|v| v.as_u64())
        .unwrap_or(20) as u16;
    let height = element
        .props
        .get("height")
        .and_then(|v| v.as_u64())
        .unwrap_or(5) as u16;

    let x = element.props.get("x").and_then(|v| v.as_u64()).unwrap_or(0) as u16;

    let anchor = element
        .props
        .get("anchor")
        .and_then(|v| v.as_str())
        .unwrap_or("top");

    let y = if anchor == "bottom" {
        let from_bottom = element.props.get("y").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
        frame_area.height.saturating_sub(from_bottom + height)
    } else {
        element.props.get("y").and_then(|v| v.as_u64()).unwrap_or(0) as u16
    };

    let popup_area = Rect::new(
        frame_area.x + x.min(frame_area.width.saturating_sub(width)),
        frame_area.y + y.min(frame_area.height.saturating_sub(height)),
        width.min(frame_area.width),
        height.min(frame_area.height),
    );

    frame.render_widget(Clear, popup_area);
    for child in &element.children {
        render_element(frame, popup_area, child);
    }
}
