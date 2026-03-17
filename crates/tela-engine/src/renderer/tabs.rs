use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Tabs;
use ratatui::Frame;

use super::{collect_element_children, collect_plain_text, parse_color, parse_style};
use crate::elements::Element;

pub fn render(frame: &mut Frame, area: Rect, element: &Element) {
    let children = collect_element_children(element);
    let titles: Vec<String> = children
        .iter()
        .map(|child| collect_plain_text(child))
        .collect();

    let divider = element
        .props
        .get("divider")
        .and_then(|v| v.as_str())
        .unwrap_or(" | ")
        .to_string();

    let mut highlight_style = Style::default().add_modifier(Modifier::BOLD);
    if let Some(fg) = element.props.get("highlight_fg").and_then(|v| v.as_str()) {
        highlight_style = highlight_style.fg(parse_color(fg));
    }

    let selected = element
        .props
        .get("selected")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let mut tabs = Tabs::new(titles)
        .select(selected)
        .highlight_style(highlight_style)
        .divider(divider);

    tabs = tabs.style(parse_style(element));

    frame.render_widget(tabs, area);
}
