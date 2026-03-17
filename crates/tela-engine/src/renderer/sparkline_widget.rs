use ratatui::layout::Rect;
use ratatui::symbols;
use ratatui::widgets::{RenderDirection, Sparkline};
use ratatui::Frame;

use super::parse_style;
use crate::elements::Element;

pub fn render(frame: &mut Frame, area: Rect, element: &Element) {
    let data: Vec<u64> = element
        .props
        .get("data")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(|v| v.as_u64().unwrap_or(0)).collect())
        .unwrap_or_default();

    let mut sparkline = Sparkline::default().data(&data);

    if let Some(max) = element.props.get("max").and_then(|v| v.as_u64()) {
        sparkline = sparkline.max(max);
    }

    sparkline = sparkline.style(parse_style(element));

    match element.props.get("barSet").and_then(|v| v.as_str()) {
        Some("three") => {
            sparkline = sparkline.bar_set(symbols::bar::THREE_LEVELS);
        }
        Some("nine") => {
            sparkline = sparkline.bar_set(symbols::bar::NINE_LEVELS);
        }
        _ => {}
    }

    match element.props.get("direction").and_then(|v| v.as_str()) {
        Some("rtl") => {
            sparkline = sparkline.direction(RenderDirection::RightToLeft);
        }
        _ => {}
    }

    frame.render_widget(sparkline, area);
}
