use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::symbols;
use ratatui::text::Span;
use ratatui::widgets::{Axis, Chart, Dataset};
use ratatui::Frame;

use super::{collect_element_children, parse_bounds, parse_color};
use crate::elements::Element;

pub fn render(frame: &mut Frame, area: Rect, element: &Element) {
    let children = collect_element_children(element);

    let owned_datasets: Vec<(String, Vec<(f64, f64)>, Style, symbols::Marker)> = children
        .iter()
        .filter(|c| c.tag == "dataset")
        .map(|ds| {
            let name = ds
                .props
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let points: Vec<(f64, f64)> = ds
                .props
                .get("data")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|p| {
                            let pair = p.as_array()?;
                            Some((pair.first()?.as_f64()?, pair.get(1)?.as_f64()?))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let style = ds
                .props
                .get("fg")
                .and_then(|v| v.as_str())
                .map(|fg| Style::default().fg(parse_color(fg)))
                .unwrap_or_default();
            let marker = match ds.props.get("marker").and_then(|v| v.as_str()) {
                Some("braille") => symbols::Marker::Braille,
                Some("block") => symbols::Marker::Block,
                Some("bar") => symbols::Marker::Bar,
                _ => symbols::Marker::Dot,
            };
            (name, points, style, marker)
        })
        .collect();

    let datasets: Vec<Dataset> = owned_datasets
        .iter()
        .map(|(name, points, style, marker)| {
            Dataset::default()
                .name(name.as_str())
                .data(points)
                .style(*style)
                .marker(*marker)
        })
        .collect();

    let mut chart = Chart::new(datasets);

    let (x_bounds, y_bounds) =
        parse_bounds(element, "xBounds", "yBounds", [0.0, 100.0], [0.0, 100.0]);

    let x_title = element
        .props
        .get("xTitle")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let y_title = element
        .props
        .get("yTitle")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let x_labels: Vec<Span> = element
        .props
        .get("xLabels")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| Span::raw(v.as_str().unwrap_or("").to_string()))
                .collect()
        })
        .unwrap_or_default();

    let y_labels: Vec<Span> = element
        .props
        .get("yLabels")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| Span::raw(v.as_str().unwrap_or("").to_string()))
                .collect()
        })
        .unwrap_or_default();

    let mut x_axis = Axis::default().bounds(x_bounds);
    if !x_title.is_empty() {
        x_axis = x_axis.title(x_title.to_string());
    }
    if !x_labels.is_empty() {
        x_axis = x_axis.labels(x_labels);
    }

    let mut y_axis = Axis::default().bounds(y_bounds);
    if !y_title.is_empty() {
        y_axis = y_axis.title(y_title.to_string());
    }
    if !y_labels.is_empty() {
        y_axis = y_axis.labels(y_labels);
    }

    chart = chart.x_axis(x_axis).y_axis(y_axis);

    frame.render_widget(chart, area);
}
