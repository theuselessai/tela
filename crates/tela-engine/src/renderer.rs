//! Element tree → ratatui widget rendering.
//!
//! Walks the `Element` tree and maps each node to ratatui widgets,
//! performing layout via `ratatui::layout::Layout` and drawing
//! into a `Frame`.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Axis, Bar, BarChart, BarGroup, Block, BorderType, Borders, Cell, Chart, Dataset, Gauge,
    LineGauge, List, ListItem, ListState, Paragraph, Row, Sparkline, Table, TableState, Tabs, Wrap,
};
use ratatui::Frame;

use crate::elements::Element;

/// Render an element tree into the terminal frame.
pub fn render_element(frame: &mut Frame, area: Rect, element: &Element) {
    match element.tag.as_str() {
        "__fragment__" | "row" => render_children(frame, area, element),
        "__text__" | "span" => {} // consumed by parent
        "box" => render_box(frame, area, element),
        "text" => render_text(frame, area, element),
        "layout" => render_layout(frame, area, element),
        "list" => render_list(frame, area, element),
        "table" => render_table(frame, area, element),
        "tabs" => render_tabs(frame, area, element),
        "input" => render_input(frame, area, element),
        "textarea" => render_textarea(frame, area, element),
        "gauge" => render_gauge(frame, area, element),
        "linegauge" => render_linegauge(frame, area, element),
        "sparkline" => render_sparkline(frame, area, element),
        "barchart" => render_barchart(frame, area, element),
        "chart" => render_chart(frame, area, element),
        _ => {}
    }
}

fn render_children(frame: &mut Frame, area: Rect, element: &Element) {
    for child in &element.children {
        render_element(frame, area, child);
    }
}

fn render_box(frame: &mut Frame, area: Rect, element: &Element) {
    let border_prop = element
        .props
        .get("border")
        .and_then(|v| v.as_str())
        .unwrap_or("single");

    let block = if border_prop == "none" {
        let mut b = Block::default();
        if let Some(title) = element.props.get("title").and_then(|v| v.as_str()) {
            b = b.title(title.to_string());
        }
        b
    } else {
        let mut b = Block::default()
            .borders(Borders::ALL)
            .border_type(parse_border_type(border_prop));
        if let Some(title) = element.props.get("title").and_then(|v| v.as_str()) {
            b = b.title(title.to_string());
        }
        b
    };

    let inner = block.inner(area);
    frame.render_widget(block, area);
    render_children(frame, inner, element);
}

fn render_text(frame: &mut Frame, area: Rect, element: &Element) {
    let spans = collect_text_content(element);
    let line = Line::from(spans);

    let mut style = Style::default();
    if let Some(fg) = element.props.get("fg").and_then(|v| v.as_str()) {
        style = style.fg(parse_color(fg));
    }
    if let Some(bg) = element.props.get("bg").and_then(|v| v.as_str()) {
        style = style.bg(parse_color(bg));
    }
    if element
        .props
        .get("bold")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        style = style.add_modifier(Modifier::BOLD);
    }
    if element
        .props
        .get("italic")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if element
        .props
        .get("underline")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        style = style.add_modifier(Modifier::UNDERLINED);
    }

    let alignment = match element.props.get("align").and_then(|v| v.as_str()) {
        Some("center") => Alignment::Center,
        Some("right") => Alignment::Right,
        _ => Alignment::Left,
    };

    let mut paragraph = Paragraph::new(line).style(style).alignment(alignment);
    if element.props.get("wrap").is_some() {
        let trim = element.props.get("wrap").and_then(|v| v.as_str()) == Some("trim");
        paragraph = paragraph.wrap(Wrap { trim });
    }
    if let Some(offset) = element.props.get("scroll").and_then(|v| v.as_u64()) {
        paragraph = paragraph.scroll((offset as u16, 0));
    }
    frame.render_widget(paragraph, area);
}

fn render_layout(frame: &mut Frame, area: Rect, element: &Element) {
    let direction = match element.props.get("direction").and_then(|v| v.as_str()) {
        Some("horizontal") => Direction::Horizontal,
        _ => Direction::Vertical,
    };

    let constraints: Vec<Constraint> = element
        .children
        .iter()
        .map(|child| {
            if let Some(h) = child.props.get("height").and_then(|v| v.as_u64()) {
                Constraint::Length(h as u16)
            } else if let Some(w) = child.props.get("width").and_then(|v| v.as_u64()) {
                Constraint::Length(w as u16)
            } else if let Some(flex) = child.props.get("flex").and_then(|v| v.as_u64()) {
                Constraint::Min(flex as u16)
            } else if let Some(m) = child.props.get("maxHeight").and_then(|v| v.as_u64()) {
                Constraint::Max(m as u16)
            } else if let Some(m) = child.props.get("maxWidth").and_then(|v| v.as_u64()) {
                Constraint::Max(m as u16)
            } else if let Some(p) = child.props.get("layoutPercent").and_then(|v| v.as_u64()) {
                Constraint::Percentage(p as u16)
            } else {
                Constraint::Min(0)
            }
        })
        .collect();

    let chunks = Layout::default()
        .direction(direction)
        .constraints(constraints)
        .split(area);

    for (i, child) in element.children.iter().enumerate() {
        if i < chunks.len() {
            render_element(frame, chunks[i], child);
        }
    }
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

fn render_list(frame: &mut Frame, area: Rect, element: &Element) {
    let children = collect_element_children(element);
    let items: Vec<ListItem> = children
        .iter()
        .map(|child| ListItem::new(Line::from(collect_text_content(child))))
        .collect();

    let highlight_symbol = element
        .props
        .get("highlight_symbol")
        .and_then(|v| v.as_str())
        .unwrap_or("▶ ");

    let mut highlight_style = Style::default();
    if let Some(fg) = element.props.get("highlight_fg").and_then(|v| v.as_str()) {
        highlight_style = highlight_style.fg(parse_color(fg));
    }
    if let Some(bg) = element.props.get("highlight_bg").and_then(|v| v.as_str()) {
        highlight_style = highlight_style.bg(parse_color(bg));
    }

    let list = List::new(items)
        .highlight_style(highlight_style)
        .highlight_symbol(highlight_symbol);

    if let Some(selected) = element.props.get("selected").and_then(|v| v.as_u64()) {
        let mut state = ListState::default();
        state.select(Some(selected as usize));
        frame.render_stateful_widget(list, area, &mut state);
    } else {
        frame.render_widget(list, area);
    }
}

// ---------------------------------------------------------------------------
// Table
// ---------------------------------------------------------------------------

fn render_table(frame: &mut Frame, area: Rect, element: &Element) {
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

    let mut highlight_style = Style::default();
    if let Some(fg) = element.props.get("highlight_fg").and_then(|v| v.as_str()) {
        highlight_style = highlight_style.fg(parse_color(fg));
    }
    if let Some(bg) = element.props.get("highlight_bg").and_then(|v| v.as_str()) {
        highlight_style = highlight_style.bg(parse_color(bg));
    }
    table = table.row_highlight_style(highlight_style);

    if let Some(selected) = element.props.get("selected").and_then(|v| v.as_u64()) {
        let mut state = TableState::default();
        state.select(Some(selected as usize));
        frame.render_stateful_widget(table, area, &mut state);
    } else {
        frame.render_widget(table, area);
    }
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

fn render_tabs(frame: &mut Frame, area: Rect, element: &Element) {
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

    let tabs = Tabs::new(titles)
        .select(selected)
        .highlight_style(highlight_style)
        .divider(divider);

    frame.render_widget(tabs, area);
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

fn render_input(frame: &mut Frame, area: Rect, element: &Element) {
    let value = element
        .props
        .get("value")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let placeholder = element
        .props
        .get("placeholder")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let cursor = element
        .props
        .get("cursor")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(value.len());

    let mut fg = Color::Reset;
    if let Some(f) = element.props.get("fg").and_then(|v| v.as_str()) {
        fg = parse_color(f);
    }
    let mut bg = Color::Reset;
    if let Some(b) = element.props.get("bg").and_then(|v| v.as_str()) {
        bg = parse_color(b);
    }

    let spans = if value.is_empty() && !placeholder.is_empty() {
        vec![
            Span::styled(placeholder.to_string(), Style::default().fg(Color::Gray)),
            Span::styled(" ".to_string(), Style::default().fg(bg).bg(fg)),
        ]
    } else {
        let chars: Vec<char> = value.chars().collect();
        let cursor_pos = cursor.min(chars.len());
        let before: String = chars[..cursor_pos].iter().collect();
        let cursor_char = if cursor_pos < chars.len() {
            chars[cursor_pos].to_string()
        } else {
            " ".to_string()
        };
        let after: String = if cursor_pos < chars.len() {
            chars[cursor_pos + 1..].iter().collect()
        } else {
            String::new()
        };

        let normal_style = Style::default().fg(fg).bg(bg);
        let cursor_style = Style::default().fg(bg).bg(fg);

        let mut result = vec![
            Span::styled(before, normal_style),
            Span::styled(cursor_char, cursor_style),
        ];
        if !after.is_empty() {
            result.push(Span::styled(after, normal_style));
        }
        result
    };

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Textarea
// ---------------------------------------------------------------------------

fn render_textarea(frame: &mut Frame, area: Rect, element: &Element) {
    let value = element
        .props
        .get("value")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let placeholder = element
        .props
        .get("placeholder")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let cursor = element
        .props
        .get("cursor")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(value.len());

    let mut fg = Color::Reset;
    if let Some(f) = element.props.get("fg").and_then(|v| v.as_str()) {
        fg = parse_color(f);
    }
    let mut bg = Color::Reset;
    if let Some(b) = element.props.get("bg").and_then(|v| v.as_str()) {
        bg = parse_color(b);
    }

    let width = area.width as usize;
    if width == 0 {
        return;
    }

    if value.is_empty() && !placeholder.is_empty() {
        let mut lines: Vec<Line> = vec![Line::from(vec![
            Span::styled(placeholder.to_string(), Style::default().fg(Color::Gray)),
            Span::styled(" ".to_string(), Style::default().fg(bg).bg(fg)),
        ])];
        for _ in 1..area.height {
            lines.push(Line::from(""));
        }
        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, area);
        return;
    }

    let normal_style = Style::default().fg(fg).bg(bg);
    let cursor_style = Style::default().fg(bg).bg(fg);

    let raw_lines: Vec<&str> = value.split('\n').collect();
    let mut display_lines: Vec<Vec<char>> = Vec::new();
    let mut line_map: Vec<(usize, usize)> = Vec::new(); // (raw_line_idx, start_offset_in_raw)

    let should_wrap = element
        .props
        .get("wrap")
        .map(|v| v.as_bool().unwrap_or(true))
        .unwrap_or(true);

    for (raw_idx, raw_line) in raw_lines.iter().enumerate() {
        let chars: Vec<char> = raw_line.chars().collect();
        if chars.is_empty() {
            display_lines.push(Vec::new());
            line_map.push((raw_idx, 0));
        } else if !should_wrap || chars.len() <= width {
            display_lines.push(chars);
            line_map.push((raw_idx, 0));
        } else {
            let mut offset = 0;
            while offset < chars.len() {
                let end = (offset + width).min(chars.len());
                display_lines.push(chars[offset..end].to_vec());
                line_map.push((raw_idx, offset));
                offset = end;
            }
        }
    }

    let cursor_pos = cursor.min(value.len());
    let mut flat_idx = 0;
    let mut cursor_display_line = 0;
    let mut cursor_display_col = 0;

    for (raw_idx, raw_line) in raw_lines.iter().enumerate() {
        let line_len = raw_line.len();
        if flat_idx + line_len >= cursor_pos || raw_idx == raw_lines.len() - 1 {
            let offset_in_line = cursor_pos - flat_idx;
            let char_offset = raw_line
                .char_indices()
                .enumerate()
                .find(|(i, _)| *i >= offset_in_line)
                .map(|(i, _)| i)
                .unwrap_or(raw_line.chars().count());

            for (dl_idx, (map_raw, map_start)) in line_map.iter().enumerate() {
                if *map_raw == raw_idx {
                    let dl_len = display_lines[dl_idx].len();
                    if char_offset >= *map_start && char_offset <= *map_start + dl_len {
                        cursor_display_line = dl_idx;
                        cursor_display_col = char_offset - map_start;
                        break;
                    }
                    if char_offset < *map_start {
                        cursor_display_line = dl_idx;
                        cursor_display_col = 0;
                        break;
                    }
                    cursor_display_line = dl_idx;
                    cursor_display_col = dl_len;
                }
            }
            break;
        }
        flat_idx += line_len + 1; // +1 for \n
    }

    let visible_height = area.height as usize;
    let scroll_offset = if cursor_display_line >= visible_height {
        cursor_display_line - visible_height + 1
    } else {
        0
    };

    let mut rendered_lines: Vec<Line> = Vec::new();
    for dl_idx in scroll_offset..(scroll_offset + visible_height).min(display_lines.len()) {
        let chars = &display_lines[dl_idx];

        if dl_idx == cursor_display_line {
            let mut spans = Vec::new();
            let before: String = chars[..cursor_display_col.min(chars.len())]
                .iter()
                .collect();
            let cursor_char = if cursor_display_col < chars.len() {
                chars[cursor_display_col].to_string()
            } else {
                " ".to_string()
            };
            let after: String = if cursor_display_col < chars.len() {
                chars[cursor_display_col + 1..].iter().collect()
            } else {
                String::new()
            };
            spans.push(Span::styled(before, normal_style));
            spans.push(Span::styled(cursor_char, cursor_style));
            if !after.is_empty() {
                spans.push(Span::styled(after, normal_style));
            }
            rendered_lines.push(Line::from(spans));
        } else {
            let text: String = chars.iter().collect();
            rendered_lines.push(Line::from(Span::styled(text, normal_style)));
        }
    }

    for _ in rendered_lines.len()..visible_height {
        rendered_lines.push(Line::from(""));
    }

    let paragraph = Paragraph::new(rendered_lines);
    frame.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Gauge
// ---------------------------------------------------------------------------

fn render_gauge(frame: &mut Frame, area: Rect, element: &Element) {
    let percent = element
        .props
        .get("percent")
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        .min(100) as u16;

    let mut gauge = Gauge::default().percent(percent);

    if let Some(label) = element.props.get("label").and_then(|v| v.as_str()) {
        gauge = gauge.label(label.to_string());
    }

    let mut style = Style::default();
    if let Some(fg) = element.props.get("fg").and_then(|v| v.as_str()) {
        style = style.fg(parse_color(fg));
    }
    if let Some(bg) = element.props.get("bg").and_then(|v| v.as_str()) {
        style = style.bg(parse_color(bg));
    }
    gauge = gauge.gauge_style(style);

    frame.render_widget(gauge, area);
}

// ---------------------------------------------------------------------------
// LineGauge
// ---------------------------------------------------------------------------

fn render_linegauge(frame: &mut Frame, area: Rect, element: &Element) {
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

    let mut style = Style::default();
    if let Some(fg) = element.props.get("fg").and_then(|v| v.as_str()) {
        style = style.fg(parse_color(fg));
    }
    if let Some(bg) = element.props.get("bg").and_then(|v| v.as_str()) {
        style = style.bg(parse_color(bg));
    }
    lg = lg.filled_style(style);

    if let Some(ls) = element.props.get("lineSet").and_then(|v| v.as_str()) {
        lg = lg.line_set(match ls {
            "thick" => symbols::line::THICK,
            "double" => symbols::line::DOUBLE,
            _ => symbols::line::NORMAL,
        });
    }

    frame.render_widget(lg, area);
}

// ---------------------------------------------------------------------------
// Sparkline
// ---------------------------------------------------------------------------

fn render_sparkline(frame: &mut Frame, area: Rect, element: &Element) {
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

    let mut style = Style::default();
    if let Some(fg) = element.props.get("fg").and_then(|v| v.as_str()) {
        style = style.fg(parse_color(fg));
    }
    if let Some(bg) = element.props.get("bg").and_then(|v| v.as_str()) {
        style = style.bg(parse_color(bg));
    }
    sparkline = sparkline.style(style);

    frame.render_widget(sparkline, area);
}

// ---------------------------------------------------------------------------
// BarChart
// ---------------------------------------------------------------------------

fn render_barchart(frame: &mut Frame, area: Rect, element: &Element) {
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

    let mut bar_style = Style::default();
    if let Some(fg) = element.props.get("fg").and_then(|v| v.as_str()) {
        bar_style = bar_style.fg(parse_color(fg));
    }
    chart = chart.bar_style(bar_style);

    if let Some(fg) = element.props.get("valueFg").and_then(|v| v.as_str()) {
        chart = chart.value_style(Style::default().fg(parse_color(fg)));
    }
    if let Some(fg) = element.props.get("labelFg").and_then(|v| v.as_str()) {
        chart = chart.label_style(Style::default().fg(parse_color(fg)));
    }

    frame.render_widget(chart, area);
}

// ---------------------------------------------------------------------------
// Chart
// ---------------------------------------------------------------------------

fn render_chart(frame: &mut Frame, area: Rect, element: &Element) {
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

    let x_bounds = element
        .props
        .get("xBounds")
        .and_then(|v| v.as_array())
        .map(|a| {
            [
                a.first().and_then(|v| v.as_f64()).unwrap_or(0.0),
                a.get(1).and_then(|v| v.as_f64()).unwrap_or(100.0),
            ]
        })
        .unwrap_or([0.0, 100.0]);

    let y_bounds = element
        .props
        .get("yBounds")
        .and_then(|v| v.as_array())
        .map(|a| {
            [
                a.first().and_then(|v| v.as_f64()).unwrap_or(0.0),
                a.get(1).and_then(|v| v.as_f64()).unwrap_or(100.0),
            ]
        })
        .unwrap_or([0.0, 100.0]);

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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Collect styled spans from an element's children.
/// `__text__` children become unstyled spans, `<span>` children become styled
/// spans, and fragments are recursively flattened.
fn collect_text_content(element: &Element) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for child in &element.children {
        if child.is_text() {
            if let Some(content) = child.text_content() {
                spans.push(Span::raw(content.to_string()));
            }
        } else if child.tag == "span" {
            let text = collect_plain_text(child);
            let style = parse_style(child);
            spans.push(Span::styled(text, style));
        } else if child.is_fragment() {
            spans.extend(collect_text_content(child));
        }
    }
    spans
}

/// Collect plain text from an element's children (discarding styles).
fn collect_plain_text(element: &Element) -> String {
    collect_text_content(element)
        .into_iter()
        .map(|s| s.content.into_owned())
        .collect()
}

/// Collect non-text children, flattening fragments.
fn collect_element_children(element: &Element) -> Vec<&Element> {
    let mut result = Vec::new();
    for child in &element.children {
        if child.is_fragment() {
            result.extend(collect_element_children(child));
        } else if !child.is_text() {
            result.push(child);
        }
    }
    result
}

/// Collect `<row>` children for table rendering, flattening fragments.
fn collect_row_children(element: &Element) -> Vec<&Element> {
    let mut result = Vec::new();
    for child in &element.children {
        if child.tag == "row" {
            result.push(child);
        } else if child.is_fragment() {
            result.extend(collect_row_children(child));
        }
    }
    result
}

/// Parse style props from a `<span>` element.
fn parse_style(element: &Element) -> Style {
    let mut style = Style::default();
    if let Some(fg) = element.props.get("fg").and_then(|v| v.as_str()) {
        style = style.fg(parse_color(fg));
    }
    if let Some(bg) = element.props.get("bg").and_then(|v| v.as_str()) {
        style = style.bg(parse_color(bg));
    }
    if element
        .props
        .get("bold")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        style = style.add_modifier(Modifier::BOLD);
    }
    if element
        .props
        .get("italic")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if element
        .props
        .get("underline")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

fn parse_color(name: &str) -> Color {
    let lower = name.to_lowercase();
    if lower.starts_with('#') && lower.len() == 7 {
        let r = u8::from_str_radix(&lower[1..3], 16).unwrap_or(0);
        let g = u8::from_str_radix(&lower[3..5], 16).unwrap_or(0);
        let b = u8::from_str_radix(&lower[5..7], 16).unwrap_or(0);
        return Color::Rgb(r, g, b);
    }
    match lower.as_str() {
        "red" => Color::Red,
        "green" => Color::Green,
        "blue" => Color::Blue,
        "yellow" => Color::Yellow,
        "cyan" => Color::Cyan,
        "magenta" => Color::Magenta,
        "white" => Color::White,
        "black" => Color::Black,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightblue" => Color::LightBlue,
        "lightyellow" => Color::LightYellow,
        "lightcyan" => Color::LightCyan,
        "lightmagenta" => Color::LightMagenta,
        _ => Color::Reset,
    }
}

fn parse_border_type(name: &str) -> BorderType {
    match name.to_lowercase().as_str() {
        "double" => BorderType::Double,
        "rounded" => BorderType::Rounded,
        "thick" => BorderType::Thick,
        _ => BorderType::Plain,
    }
}
