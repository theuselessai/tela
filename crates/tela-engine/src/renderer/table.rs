use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Cell, Row, Table, TableState};
use ratatui::Frame;

use super::{
    collect_element_children, collect_row_children, collect_text_content, parse_highlight_style,
    parse_style,
};
use crate::elements::Element;

pub fn render(frame: &mut Frame, area: Rect, element: &Element) {
    let row_elements = collect_row_children(element);
    let rows: Vec<Row> = row_elements
        .iter()
        .map(|row_el| {
            let cell_children = collect_element_children(row_el);
            let cells: Vec<Cell> = cell_children
                .iter()
                .map(|c| {
                    let mut cell = Cell::from(Line::from(collect_text_content(c)));
                    cell = cell.style(parse_style(c));
                    cell
                })
                .collect();
            Row::new(cells)
        })
        .collect();

    let widths: Vec<Constraint> = element
        .props
        .get("widths")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| Constraint::Length(v.as_u64().unwrap_or(10) as u16))
                .collect()
        })
        .unwrap_or_else(|| vec![Constraint::Min(0)]);

    let mut table = Table::new(rows, &widths);

    if let Some(header_arr) = element.props.get("header").and_then(|v| v.as_array()) {
        let header_cells: Vec<Cell> = header_arr
            .iter()
            .map(|v| Cell::from(v.as_str().unwrap_or("").to_string()))
            .collect();
        table = table.header(
            Row::new(header_cells)
                .style(Style::default().add_modifier(Modifier::BOLD))
                .bottom_margin(1),
        );
    }

    let highlight_style = parse_highlight_style(element);
    table = table.row_highlight_style(highlight_style);

    if let Some(s) = element
        .props
        .get("highlightSymbol")
        .and_then(|v| v.as_str())
    {
        table = table.highlight_symbol(s.to_string());
    }
    if let Some(s) = element.props.get("columnSpacing").and_then(|v| v.as_u64()) {
        table = table.column_spacing(s as u16);
    }

    if let Some(selected) = element.props.get("selected").and_then(|v| v.as_u64()) {
        let mut state = TableState::default();
        state.select(Some(selected as usize));
        frame.render_stateful_widget(table, area, &mut state);
    } else {
        frame.render_widget(table, area);
    }
}
