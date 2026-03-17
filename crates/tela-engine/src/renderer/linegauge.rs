use ratatui::layout::Rect;
use ratatui::symbols;
use ratatui::widgets::LineGauge;
use ratatui::Frame;

use super::parse_style;
use crate::elements::Element;

pub fn render(frame: &mut Frame, area: Rect, element: &Element) {
    let ratio = element
        .props
        .get("ratio")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);

    let mut lg = LineGauge::default().ratio(ratio);

    if let Some(label) = element.props.get("label").and_then(|v| v.as_str()) {
        lg = lg.label(label.to_string());
    }

    lg = lg.filled_style(parse_style(element));

    if let Some(ls) = element.props.get("lineSet").and_then(|v| v.as_str()) {
        lg = lg.line_set(match ls {
            "thick" => symbols::line::THICK,
            "double" => symbols::line::DOUBLE,
            _ => symbols::line::NORMAL,
        });
    }

    frame.render_widget(lg, area);
}
