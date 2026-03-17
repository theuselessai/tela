use ratatui::layout::Rect;
use ratatui::widgets::Gauge;
use ratatui::Frame;

use super::parse_style;
use crate::elements::Element;

pub fn render(frame: &mut Frame, area: Rect, element: &Element) {
    let mut gauge = if let Some(ratio) = element.props.get("ratio").and_then(|v| v.as_f64()) {
        Gauge::default().ratio(ratio.clamp(0.0, 1.0))
    } else {
        let percent = element
            .props
            .get("percent")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            .min(100) as u16;
        Gauge::default().percent(percent)
    };

    if let Some(label) = element.props.get("label").and_then(|v| v.as_str()) {
        gauge = gauge.label(label.to_string());
    }

    gauge = gauge.gauge_style(parse_style(element));

    frame.render_widget(gauge, area);
}
