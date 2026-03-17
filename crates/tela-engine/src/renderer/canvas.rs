use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::symbols;
use ratatui::text::Span;
use ratatui::widgets::canvas::{Canvas, Circle, Map, Rectangle};
use ratatui::Frame;

use super::{collect_canvas_shapes, parse_bounds, parse_color, CanvasShape};
use crate::elements::Element;

pub fn render(frame: &mut Frame, area: Rect, element: &Element) {
    let (x_bounds, y_bounds) =
        if let Some(vb) = element.props.get("viewBox").and_then(|v| v.as_str()) {
            let parts: Vec<f64> = vb
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if parts.len() == 4 {
                (
                    [parts[0], parts[0] + parts[2]],
                    [parts[1], parts[1] + parts[3]],
                )
            } else {
                ([-180.0, 180.0], [-90.0, 90.0])
            }
        } else {
            parse_bounds(
                element,
                "xBounds",
                "yBounds",
                [-180.0, 180.0],
                [-90.0, 90.0],
            )
        };

    let marker = match element.props.get("marker").and_then(|v| v.as_str()) {
        Some("dot") => symbols::Marker::Dot,
        Some("block") => symbols::Marker::Block,
        Some("bar") => symbols::Marker::Bar,
        _ => symbols::Marker::Braille,
    };

    let shapes = collect_canvas_shapes(element);

    let mut canvas = Canvas::default()
        .x_bounds(x_bounds)
        .y_bounds(y_bounds)
        .marker(marker)
        .paint(move |ctx| {
            for shape in &shapes {
                match shape {
                    CanvasShape::MapShape { resolution, color } => {
                        ctx.draw(&Map {
                            color: *color,
                            resolution: *resolution,
                        });
                    }
                    CanvasShape::Rect {
                        x,
                        y,
                        width,
                        height,
                        color,
                    } => {
                        ctx.draw(&Rectangle {
                            x: *x,
                            y: *y,
                            width: *width,
                            height: *height,
                            color: *color,
                        });
                    }
                    CanvasShape::CircleShape { cx, cy, r, color } => {
                        ctx.draw(&Circle {
                            x: *cx,
                            y: *cy,
                            radius: *r,
                            color: *color,
                        });
                    }
                    CanvasShape::Line {
                        x1,
                        y1,
                        x2,
                        y2,
                        color,
                    } => {
                        ctx.draw(&ratatui::widgets::canvas::Line {
                            x1: *x1,
                            y1: *y1,
                            x2: *x2,
                            y2: *y2,
                            color: *color,
                        });
                    }
                    CanvasShape::Text {
                        x,
                        y,
                        color,
                        content,
                    } => {
                        ctx.print(
                            *x,
                            *y,
                            Span::styled(content.clone(), Style::default().fg(*color)),
                        );
                    }
                    CanvasShape::Lines { segments, color } => {
                        for seg in segments {
                            ctx.draw(&ratatui::widgets::canvas::Line {
                                x1: seg.0,
                                y1: seg.1,
                                x2: seg.2,
                                y2: seg.3,
                                color: *color,
                            });
                        }
                    }
                }
            }
        });

    if let Some(c) = element
        .props
        .get("backgroundColor")
        .and_then(|v| v.as_str())
    {
        canvas = canvas.background_color(parse_color(c));
    }

    frame.render_widget(canvas, area);
}
