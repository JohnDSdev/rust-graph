//! Export the same sampled geometry used by the desktop canvas.
use crate::{
    document::Document,
    graph::{COLORS, Scene, format_number, grid_step},
};
use std::fmt::Write;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn color(index: usize) -> String {
    let [r, g, b] = COLORS[index % COLORS.len()];
    format!("#{r:02x}{g:02x}{b:02x}")
}
pub fn svg(doc: &Document, scene: &Scene, width: u32, height: u32) -> String {
    let width = width.clamp(100, 4096);
    let height = height.clamp(100, 4096);
    let w = f64::from(width);
    let h = f64::from(height);
    let v = doc.viewport;
    let geometry = scene.geometry(v, width as usize, height as usize);
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><title>{}</title><defs><clipPath id=\"canvas\"><rect width=\"{width}\" height=\"{height}\"/></clipPath></defs><rect width=\"100%\" height=\"100%\" fill=\"white\"/><g clip-path=\"url(#canvas)\">",
        escape(&doc.title)
    );
    let xs = grid_step(v.xmax - v.xmin, w);
    let ys = grid_step(v.ymax - v.ymin, h);
    if v.grid {
        for (vertical, step, min, max) in [(true, xs, v.xmin, v.xmax), (false, ys, v.ymin, v.ymax)]
        {
            let minor = step / 5.0;
            let start = (min / minor).ceil() as i64;
            let end = (max / minor).floor() as i64;
            for n in start..=end {
                let value = n as f64 * minor;
                let major = n % 5 == 0;
                let shade = if major { "#c7cbd0" } else { "#e9ebed" };
                let p = v.to_screen(if vertical { [value, 0.0] } else { [0.0, value] }, w, h);
                if vertical {
                    let _ = write!(
                        out,
                        "<path d=\"M{:.2} 0V{height}\" stroke=\"{shade}\"/>",
                        p[0]
                    );
                } else {
                    let _ = write!(
                        out,
                        "<path d=\"M0 {:.2}H{width}\" stroke=\"{shade}\"/>",
                        p[1]
                    );
                }
            }
        }
    }
    for (c, [x, y, dx, dy]) in geometry.fills {
        let a = v.to_screen([x, y + dy], w, h);
        let b = v.to_screen([x + dx, y], w, h);
        let _ = write!(
            out,
            "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\" fill-opacity=\"0.16\"/>",
            a[0],
            a[1],
            b[0] - a[0] + 0.3,
            b[1] - a[1] + 0.3,
            color(c)
        );
    }
    if v.axes {
        let origin = v.to_screen([0.0, 0.0], w, h);
        let _ = write!(
            out,
            "<path d=\"M{:.2} 0V{height}M0 {:.2}H{width}\" stroke=\"#454b52\" stroke-width=\"1.5\"/>",
            origin[0], origin[1]
        );
        for (vertical, step, min, max) in [(true, xs, v.xmin, v.xmax), (false, ys, v.ymin, v.ymax)]
        {
            let start = (min / step).ceil() as i64;
            let end = (max / step).floor() as i64;
            for n in start..=end {
                if n == 0 {
                    continue;
                }
                let value = n as f64 * step;
                let p = v.to_screen(if vertical { [value, 0.0] } else { [0.0, value] }, w, h);
                let (x, y) = if vertical {
                    (p[0] + 3.0, origin[1].clamp(8.0, h - 22.0) + 17.0)
                } else {
                    (origin[0].clamp(25.0, w - 30.0) - 7.0, p[1] - 5.0)
                };
                let _ = write!(
                    out,
                    "<text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"DejaVu Sans,sans-serif\" font-size=\"12\" fill=\"#5c626b\">{}</text>",
                    format_number(value)
                );
            }
        }
    }
    for (c, points) in geometry.lines {
        out.push_str("<polyline points=\"");
        for p in points {
            let p = v.to_screen(p, w, h);
            let _ = write!(out, "{:.2},{:.2} ", p[0], p[1]);
        }
        let _ = write!(
            out,
            "\" fill=\"none\" stroke=\"{}\" stroke-width=\"2.5\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
            color(c)
        );
    }
    for (c, p) in geometry.points {
        let p = v.to_screen(p, w, h);
        let _ = write!(
            out,
            "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"5\" fill=\"{}\" stroke=\"white\" stroke-width=\"1.5\"/>",
            p[0],
            p[1],
            color(c)
        );
    }
    out.push_str("</g></svg>");
    out
}
pub fn png(svg: &str) -> Result<Vec<u8>, String> {
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
    let size = tree.size().to_int_size();
    if size.width() > 4096 || size.height() > 4096 {
        return Err("Export dimensions exceed 4096 pixels".into());
    }
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or("Unable to allocate image")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|e| e.to_string())
}
