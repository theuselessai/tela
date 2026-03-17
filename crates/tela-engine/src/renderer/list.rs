use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{List, ListDirection, ListItem, ListState};
use ratatui::Frame;

use super::{collect_element_children, collect_text_content, parse_highlight_style};
use crate::elements::Element;

pub fn render(frame: &mut Frame, area: Rect, element: &Element) {
    let children = collect_element_children(element);
    let items: Vec<ListItem> = children
        .iter()
        .map(|child| ListItem::new(Line::from(collect_text_content(child))))
        .collect();

    let highlight_symbol = element
        .props
        .get("highlight_symbol")
        .and_then(|v| v.as_str())
        .unwrap_or("\u{25b6} ");

    let highlight_style = parse_highlight_style(element);

    let mut list = List::new(items)
        .highlight_style(highlight_style)
        .highlight_symbol(highlight_symbol);

    match element.props.get("direction").and_then(|v| v.as_str()) {
        Some("horizontal") => {
            list = list.direction(ListDirection::BottomToTop);
        }
        _ => {}
    }

    if let Some(selected) = element.props.get("selected").and_then(|v| v.as_u64()) {
        let mut state = ListState::default();
        state.select(Some(selected as usize));
        frame.render_stateful_widget(list, area, &mut state);
    } else {
        frame.render_widget(list, area);
    }
}
