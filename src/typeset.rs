//! Native, baseline-aware mathematical presentation with bounded fallback.
use crate::math::Expr;
use eframe::egui::{self, Color32, FontFamily, FontId, Pos2, Stroke, Vec2};
use std::sync::Arc;

enum Mark {
    Text(Pos2, Arc<egui::Galley>),
    Line(Pos2, Pos2),
}
struct Layout {
    size: Vec2,
    baseline: f32,
    marks: Vec<Mark>,
}
impl Layout {
    fn append(&mut self, other: Layout, offset: Vec2) {
        for mark in other.marks {
            self.marks.push(match mark {
                Mark::Text(p, g) => Mark::Text(p + offset, g),
                Mark::Line(a, b) => Mark::Line(a + offset, b + offset),
            });
        }
    }
}
fn text(ui: &egui::Ui, s: &str, size: f32) -> Layout {
    let family = FontFamily::Name("math".into());
    let family = if ui.fonts(|f| f.families().contains(&family)) {
        family
    } else {
        FontFamily::Proportional
    };
    let galley = ui.painter().layout_no_wrap(
        s.into(),
        FontId::new(size, family),
        ui.visuals().text_color(),
    );
    Layout {
        size: galley.size(),
        baseline: size * 0.8,
        marks: vec![Mark::Text(Pos2::ZERO, galley)],
    }
}
fn row(items: Vec<Layout>, gap: f32) -> Layout {
    let baseline = items.iter().map(|l| l.baseline).fold(0.0, f32::max);
    let descent = items
        .iter()
        .map(|l| l.size.y - l.baseline)
        .fold(0.0, f32::max);
    let mut result = Layout {
        size: Vec2::new(0.0, baseline + descent),
        baseline,
        marks: vec![],
    };
    for item in items {
        let x = result.size.x;
        result.size.x += item.size.x + gap;
        let y = baseline - item.baseline;
        result.append(item, Vec2::new(x, y));
    }
    result.size.x = (result.size.x - gap).max(0.0);
    result
}
fn fraction(a: Layout, b: Layout, size: f32) -> Layout {
    let pad = size * 0.14;
    let gap = size * 0.16;
    let width = a.size.x.max(b.size.x) + 2.0 * pad;
    let bar = a.size.y + gap;
    let mut out = Layout {
        size: Vec2::new(width, bar + gap + b.size.y),
        baseline: bar + size * 0.25,
        marks: vec![Mark::Line(Pos2::new(0.0, bar), Pos2::new(width, bar))],
    };
    let ax = (width - a.size.x) / 2.0;
    let bx = (width - b.size.x) / 2.0;
    out.append(a, Vec2::new(ax, 0.0));
    out.append(b, Vec2::new(bx, bar + gap));
    out
}
fn radical(a: Layout, size: f32, cube: bool, ui: &egui::Ui) -> Layout {
    let lead = size * 0.65;
    let top = size * 0.14;
    let width = lead + a.size.x + size * 0.1;
    let baseline = a.baseline + top;
    let height = a.size.y + top;
    let points = [
        Pos2::new(0.0, baseline - size * 0.18),
        Pos2::new(lead * 0.24, baseline - size * 0.27),
        Pos2::new(lead * 0.48, baseline + size * 0.06),
        Pos2::new(lead * 0.85, top * 0.5),
        Pos2::new(width, top * 0.5),
    ];
    let mut out = Layout {
        size: Vec2::new(width, height),
        baseline,
        marks: points.windows(2).map(|p| Mark::Line(p[0], p[1])).collect(),
    };
    if cube {
        out.append(text(ui, "3", size * 0.5), Vec2::new(0.0, 0.0));
    }
    out.append(a, Vec2::new(lead, top));
    out
}
fn power(a: Layout, b: Layout, size: f32) -> Layout {
    let lift = size * 0.5;
    let by = (a.baseline - lift - b.baseline).min(0.0);
    let ay = -by;
    let x = a.size.x;
    let width = x + b.size.x;
    let baseline = a.baseline + ay;
    let height = (a.size.y + ay).max(b.size.y);
    let mut out = Layout {
        size: Vec2::new(width, height),
        baseline,
        marks: vec![],
    };
    out.append(a, Vec2::new(0.0, ay));
    out.append(b, Vec2::new(x, 0.0));
    out
}
fn parens(ui: &egui::Ui, a: Layout, size: f32) -> Layout {
    row(vec![text(ui, "(", size), a, text(ui, ")", size)], 0.0)
}
fn needs_parens(e: &Expr) -> bool {
    matches!(e, Expr::Binary('+' | '-', _, _) | Expr::Compare(_, _))
}
fn expression(
    ui: &egui::Ui,
    e: &Expr,
    size: f32,
    depth: usize,
    budget: &mut usize,
) -> Option<Layout> {
    if depth > 48 || *budget == 0 {
        return None;
    }
    *budget -= 1;
    let mut child = |e: &Expr, size: f32| expression(ui, e, size, depth + 1, budget);
    Some(match e {
        Expr::Number(n) => text(ui, &n.to_string(), size),
        Expr::Var(v) => {
            let glyph = match v.as_str() {
                "pi" => "π".into(),
                "theta" => "θ".into(),
                "tau" => "τ".into(),
                "e" => "e".into(),
                _ if v.len() == 1 && v.as_bytes()[0].is_ascii_alphabetic() => {
                    let c = v.as_bytes()[0];
                    let italic = if c == b'h' {
                        'ℎ'
                    } else if c.is_ascii_uppercase() {
                        char::from_u32(0x1D434 + u32::from(c - b'A')).unwrap()
                    } else {
                        char::from_u32(0x1D44E + u32::from(c - b'a')).unwrap()
                    };
                    italic.to_string()
                }
                _ => v.clone(),
            };
            text(ui, &glyph, size)
        }
        Expr::Neg(e) => {
            let mut a = child(e, size)?;
            if needs_parens(e) {
                a = parens(ui, a, size);
            }
            row(vec![text(ui, "−", size), a], 0.0)
        }
        Expr::Binary('/', a, b) => fraction(child(a, size * 0.86)?, child(b, size * 0.86)?, size),
        Expr::Binary('^', a, b) => {
            let mut a_layout = child(a, size)?;
            if !matches!(
                a.as_ref(),
                Expr::Number(_) | Expr::Var(_) | Expr::Call(_, _)
            ) {
                a_layout = parens(ui, a_layout, size);
            }
            power(a_layout, child(b, size * 0.65)?, size)
        }
        Expr::Binary(op, a, b) => {
            let mut left = child(a, size)?;
            let mut right = child(b, size)?;
            if matches!(op, '*' | '%') && needs_parens(a) {
                left = parens(ui, left, size);
            }
            if (matches!(op, '*' | '%') && needs_parens(b)) || (*op == '-' && needs_parens(b)) {
                right = parens(ui, right, size);
            }
            let symbol = match op {
                '*' => "·",
                '-' => "−",
                '&' => "and",
                '|' => "or",
                _ => "",
            };
            let symbol = if symbol.is_empty() {
                op.to_string()
            } else {
                symbol.into()
            };
            row(vec![left, text(ui, &symbol, size), right], size * 0.12)
        }
        Expr::Compare(values, ops) => {
            let mut items = vec![];
            for (i, e) in values.iter().enumerate() {
                if i > 0 {
                    items.push(text(
                        ui,
                        match ops[i - 1].as_str() {
                            "<=" => "≤",
                            ">=" => "≥",
                            "!=" => "≠",
                            s => s,
                        },
                        size,
                    ));
                }
                items.push(child(e, size)?);
            }
            row(items, size * 0.15)
        }
        Expr::Call(name, args) if args.len() == 1 && matches!(name.as_str(), "sqrt" | "cbrt") => {
            radical(child(&args[0], size)?, size, name == "cbrt", ui)
        }
        Expr::Call(name, args)
            if name == "root" && args.len() == 2 && matches!(args[1], Expr::Number(2.0 | 3.0)) =>
        {
            radical(
                child(&args[0], size)?,
                size,
                matches!(args[1], Expr::Number(3.0)),
                ui,
            )
        }
        Expr::Call(name, args) => {
            let mut items = vec![];
            for (i, e) in args.iter().enumerate() {
                if i > 0 {
                    items.push(text(ui, ",", size));
                }
                items.push(child(e, size)?);
            }
            row(
                vec![
                    text(ui, name, size),
                    parens(ui, row(items, size * 0.08), size),
                ],
                size * 0.08,
            )
        }
        Expr::Factorial(e) => {
            let mut operand = child(e, size)?;
            if !matches!(
                e.as_ref(),
                Expr::Number(_) | Expr::Var(_) | Expr::Call(_, _)
            ) {
                operand = parens(ui, operand, size);
            }
            row(vec![operand, text(ui, "!", size)], 0.0)
        }
        Expr::Piecewise(cases) => {
            let mut rows = vec![];
            for (condition, value) in cases {
                let value = child(value, size)?;
                rows.push(if let Some(c) = condition {
                    row(
                        vec![value, text(ui, "if", size), child(c, size)?],
                        size * 0.3,
                    )
                } else {
                    value
                });
            }
            let width = rows.iter().map(|l| l.size.x).fold(0.0, f32::max);
            let height = rows.iter().map(|l| l.size.y + size * 0.15).sum::<f32>();
            let mut out = Layout {
                size: Vec2::new(width + size * 0.6, height),
                baseline: height / 2.0 + size * 0.25,
                marks: vec![],
            };
            let x = size * 0.4;
            let mid = height / 2.0;
            let points = [
                Pos2::new(x, 0.0),
                Pos2::new(x * 0.5, size * 0.2),
                Pos2::new(x * 0.5, mid - size * 0.2),
                Pos2::new(0.0, mid),
                Pos2::new(x * 0.5, mid + size * 0.2),
                Pos2::new(x * 0.5, height - size * 0.2),
                Pos2::new(x, height),
            ];
            out.marks
                .extend(points.windows(2).map(|p| Mark::Line(p[0], p[1])));
            let mut y = 0.0;
            for r in rows {
                let h = r.size.y;
                out.append(r, Vec2::new(size * 0.6, y));
                y += h + size * 0.15;
            }
            out
        }
    })
}
fn parse_layout(ui: &egui::Ui, s: &str, size: f32) -> Option<Layout> {
    parse_layout_at(ui, s, size, 0)
}
fn parse_layout_at(ui: &egui::Ui, s: &str, size: f32, nesting: usize) -> Option<Layout> {
    if nesting > 48 {
        return None;
    }
    if s.parse::<f64>().is_ok() {
        return Some(text(ui, s, size));
    }
    if let Some(rest) = s.strip_prefix('≈') {
        if rest.trim_start().starts_with('≈') {
            return None;
        }
        return Some(row(
            vec![
                text(ui, "≈", size),
                parse_layout_at(ui, rest.trim(), size, nesting + 1)?,
            ],
            size * 0.18,
        ));
    }

    // A trailing graph restriction is presentation, rather than multiplication.
    if s.ends_with('}') && !s.starts_with('{') {
        let mut nesting = 0i32;
        let mut restriction = None;
        for (i, c) in s.char_indices() {
            match c {
                '(' | '{' => {
                    if c == '{' && nesting == 0 {
                        restriction = Some(i);
                    }
                    nesting += 1;
                }
                ')' | '}' => nesting -= 1,
                _ => {}
            }
        }
        if let Some(i) = restriction {
            let expr = Expr::parse(s[..i].trim()).ok()?;
            let condition = Expr::parse(s[i + 1..s.len() - 1].trim()).ok()?;
            return Some(row(
                vec![
                    expression(ui, &expr, size, 0, &mut 512)?,
                    text(ui, "{", size),
                    expression(ui, &condition, size * 0.85, 0, &mut 512)?,
                    text(ui, "}", size),
                ],
                size * 0.15,
            ));
        }
    }

    // Coordinate pairs are intentionally separate from scalar parser syntax.
    if s.starts_with('(') && s.ends_with(')') {
        let inner = &s[1..s.len() - 1];
        let mut depth = 0i32;
        let mut comma = None;
        for (i, c) in inner.char_indices() {
            match c {
                '(' | '{' => depth += 1,
                ')' | '}' => depth -= 1,
                ',' if depth == 0 => {
                    if comma.is_some() {
                        return None;
                    }
                    comma = Some(i)
                }
                _ => {}
            }
        }
        if let Some(i) = comma {
            let a = parse_layout_at(ui, inner[..i].trim(), size, nesting + 1)?;
            let b = parse_layout_at(ui, inner[i + 1..].trim(), size, nesting + 1)?;
            return Some(parens(
                ui,
                row(vec![a, text(ui, ",", size), b], size * 0.12),
                size,
            ));
        }
    }
    let expr = Expr::parse(s).ok()?;
    expression(ui, &expr, size, 0, &mut 512)
}
/// Draw mathematics using STIX when installed; incomplete input remains readable.
pub fn show(ui: &mut egui::Ui, input: &str, size: f32) -> egui::Response {
    let size = if size.is_finite() {
        size.clamp(8.0, 96.0)
    } else {
        18.0
    };
    let bounded: String = input.chars().take(4096).collect();
    let layout = crate::math::normalize_tex(&bounded)
        .ok()
        .and_then(|normalized| parse_layout(ui, normalized.trim(), size))
        .unwrap_or_else(|| text(ui, &bounded, size));
    let (rect, response) = ui.allocate_exact_size(layout.size, egui::Sense::hover());
    let origin = rect.min.to_vec2();
    let color: Color32 = ui.visuals().text_color();
    for mark in layout.marks {
        match mark {
            Mark::Text(p, g) => ui.painter().galley(p + origin, g, color),
            Mark::Line(a, b) => {
                ui.painter()
                    .line_segment([a + origin, b + origin], Stroke::new(1.2_f32, color));
            }
        }
    }
    response
}
