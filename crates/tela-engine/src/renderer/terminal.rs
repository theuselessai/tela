use crate::{elements::Element, native::TerminalHandles};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    Frame,
};

/// Draw a parsed virtual-terminal grid into its assigned rectangle. The grid is
/// never serialized through JavaScript and is clipped rather than resized.
pub fn render(frame: &mut Frame, area: Rect, element: &Element, terminals: &TerminalHandles) {
    let Some(id) = element.props.get("id").and_then(|value| value.as_u64()) else {
        return;
    };
    let focused = element
        .props
        .get("focused")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let Ok(store) = terminals.lock() else {
        return;
    };
    let Some(parser) = store.get(id) else {
        return;
    };
    let screen = parser.screen();
    let (rows, cols) = screen.size();
    let width = area.width.min(cols);
    let height = area.height.min(rows);
    let buffer = frame.buffer_mut();
    for row in 0..height {
        for col in 0..width {
            let Some(cell) = screen.cell(row, col) else {
                continue;
            };
            if cell.is_wide_continuation() {
                continue;
            }
            let mut style = Style::default()
                .fg(color(cell.fgcolor()))
                .bg(color(cell.bgcolor()));
            if cell.bold() {
                style = style.add_modifier(Modifier::BOLD);
            }
            if cell.italic() {
                style = style.add_modifier(Modifier::ITALIC);
            }
            if cell.underline() {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            if cell.inverse() {
                style = style.add_modifier(Modifier::REVERSED);
            }
            // A clipped wide glyph must not overwrite the widget's border or
            // a neighboring pane when ratatui flushes the buffer.
            let symbol = if cell.is_wide() && col + 1 >= width {
                String::new()
            } else {
                cell.contents()
            };
            put(buffer, area.x + col, area.y + row, &symbol, style);
        }
    }
    if focused && !screen.hide_cursor() {
        let (row, col) = screen.cursor_position();
        if row < height && col < width {
            let Some(target) = buffer.cell_mut((area.x + col, area.y + row)) else {
                return;
            };
            let style = Style::default()
                .fg(target.fg)
                .bg(target.bg)
                .add_modifier(target.modifier | Modifier::REVERSED);
            target.set_style(style);
            if target.symbol().is_empty() {
                target.set_symbol(" ");
            }
        }
    }
}

fn put(buffer: &mut Buffer, x: u16, y: u16, symbol: &str, style: Style) {
    let Some(cell) = buffer.cell_mut((x, y)) else {
        return;
    };
    cell.set_symbol(if symbol.is_empty() { " " } else { symbol })
        .set_style(style);
}

fn color(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(index) => Color::Indexed(index),
        vt100::Color::Rgb(red, green, blue) => Color::Rgb(red, green, blue),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::terminal::new_terminal_handles;
    use ratatui::{backend::TestBackend, Terminal};
    use std::collections::HashMap;

    #[test]
    fn clipped_wide_glyph_does_not_escape_widget() {
        let terminals = new_terminal_handles();
        let id = terminals.lock().unwrap().create(8, 2).unwrap();
        terminals
            .lock()
            .unwrap()
            .write(id, "a界".as_bytes())
            .unwrap();
        let element = Element {
            tag: "terminal".into(),
            props: HashMap::from([("id".into(), serde_json::json!(id))]),
            children: vec![],
        };
        let mut terminal = Terminal::new(TestBackend::new(4, 1)).unwrap();
        terminal
            .draw(|frame| render(frame, Rect::new(0, 0, 2, 1), &element, &terminals))
            .unwrap();
        assert_eq!(
            terminal.backend().buffer().cell((0, 0)).unwrap().symbol(),
            "a"
        );
        assert_eq!(
            terminal.backend().buffer().cell((1, 0)).unwrap().symbol(),
            " "
        );
    }

    #[test]
    fn renders_colors_and_focused_cursor_clipped() {
        let terminals = new_terminal_handles();
        let id = terminals.lock().unwrap().create(8, 2).unwrap();
        terminals
            .lock()
            .unwrap()
            .write(id, b"\x1b[31mred\x1b[m\x1b[1;2H")
            .unwrap();
        let element = Element {
            tag: "terminal".into(),
            props: HashMap::from([
                ("id".into(), serde_json::json!(id)),
                ("focused".into(), serde_json::json!(true)),
            ]),
            children: vec![],
        };
        let mut terminal = Terminal::new(TestBackend::new(2, 1)).unwrap();
        terminal
            .draw(|frame| render(frame, frame.area(), &element, &terminals))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "r");
        assert_eq!(buffer.cell((0, 0)).unwrap().fg, Color::Indexed(1));
        assert!(buffer
            .cell((1, 0))
            .unwrap()
            .modifier
            .contains(Modifier::REVERSED));
    }
}
