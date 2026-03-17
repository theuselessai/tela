use ratatui::layout::{Direction, Rect};
use ratatui::style::Style;
use ratatui::symbols;
use ratatui::widgets::{Bar, BarChart, BarGroup};
use ratatui::Frame;

use super::{parse_color, parse_style};
use crate::elements::Element;

pub fn render(frame: &mut Frame, area: Rect, element: &Element) {
    let data_arr = element
        .props
        .get("data")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let bars: Vec<Bar> = data_arr
        .iter()
        .map(|item| {
            if let Some(arr) = item.as_array() {
                let label = arr.first().and_then(|v| v.as_str()).unwrap_or("");
                let value = arr.get(1).and_then(|v| v.as_u64()).unwrap_or(0);
                Bar::default().label(label.to_string().into()).value(value)
            } else {
                Bar::default().value(item.as_u64().unwrap_or(0))
            }
        })
        .collect();

    let mut chart = BarChart::default().data(BarGroup::default().bars(&bars));

    if let Some(bw) = element.props.get("barWidth").and_then(|v| v.as_u64()) {
        chart = chart.bar_width(bw as u16);
    }
    if let Some(bg) = element.props.get("barGap").and_then(|v| v.as_u64()) {
        chart = chart.bar_gap(bg as u16);
    }
    if let Some(max) = element.props.get("max").and_then(|v| v.as_u64()) {
        chart = chart.max(max);
    }

    chart = chart.bar_style(parse_style(element));

    if let Some(fg) = element.props.get("valueFg").and_then(|v| v.as_str()) {
        chart = chart.value_style(Style::default().fg(parse_color(fg)));
    }
    if let Some(fg) = element.props.get("labelFg").and_then(|v| v.as_str()) {
        chart = chart.label_style(Style::default().fg(parse_color(fg)));
    }

    match element.props.get("barSet").and_then(|v| v.as_str()) {
        Some("three") => {
            chart = chart.bar_set(symbols::bar::THREE_LEVELS);
        }
        Some("nine") => {
            chart = chart.bar_set(symbols::bar::NINE_LEVELS);
        }
        _ => {}
    }

    match element.props.get("direction").and_then(|v| v.as_str()) {
        Some("horizontal") => {
            chart = chart.direction(Direction::Horizontal);
        }
        _ => {}
    }

    if let Some(g) = element.props.get("groupGap").and_then(|v| v.as_u64()) {
        chart = chart.group_gap(g as u16);
    }

    frame.render_widget(chart, area);
}
