//! View transforms and bounded screen-resolution sampling.
use crate::math::{Env, Expr};
use serde::{Deserialize, Serialize};

pub const COLORS: [[u8; 3]; 6] = [
    [203, 65, 75],
    [47, 117, 200],
    [50, 150, 112],
    [148, 87, 189],
    [223, 146, 44],
    [53, 58, 71],
];
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Viewport {
    pub xmin: f64,
    pub xmax: f64,
    pub ymin: f64,
    pub ymax: f64,
    pub grid: bool,
    pub axes: bool,
}
impl Default for Viewport {
    fn default() -> Self {
        Self {
            xmin: -10.0,
            xmax: 10.0,
            ymin: -8.0,
            ymax: 8.0,
            grid: true,
            axes: true,
        }
    }
}
impl Viewport {
    pub fn valid(&self) -> bool {
        [self.xmin, self.xmax, self.ymin, self.ymax]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1e12)
            && (1e-9..=2e12).contains(&(self.xmax - self.xmin))
            && (1e-9..=2e12).contains(&(self.ymax - self.ymin))
    }
    pub fn to_screen(&self, [x, y]: [f64; 2], w: f64, h: f64) -> [f64; 2] {
        [
            (x - self.xmin) / (self.xmax - self.xmin) * w,
            (self.ymax - y) / (self.ymax - self.ymin) * h,
        ]
    }
    pub fn to_world(&self, [x, y]: [f64; 2], w: f64, h: f64) -> [f64; 2] {
        [
            self.xmin + x / w * (self.xmax - self.xmin),
            self.ymax - y / h * (self.ymax - self.ymin),
        ]
    }
    pub fn zoom(&mut self, factor: f64, [x, y]: [f64; 2]) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let proposed = Self {
            xmin: x + (self.xmin - x) * factor,
            xmax: x + (self.xmax - x) * factor,
            ymin: y + (self.ymin - y) * factor,
            ymax: y + (self.ymax - y) * factor,
            ..*self
        };
        if proposed.valid() {
            *self = proposed;
        }
    }
    pub fn pan(&mut self, dx: f64, dy: f64) {
        let proposed = Self {
            xmin: self.xmin + dx,
            xmax: self.xmax + dx,
            ymin: self.ymin + dy,
            ymax: self.ymax + dy,
            ..*self
        };
        if proposed.valid() {
            *self = proposed;
        }
    }
    pub fn square(&mut self, w: f64, h: f64) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let center = (self.ymin + self.ymax) / 2.0;
        let half = (self.xmax - self.xmin) * h / w / 2.0;
        let v = Self {
            ymin: center - half,
            ymax: center + half,
            ..*self
        };
        if v.valid() {
            *self = v;
        }
    }
}
#[derive(Default)]
pub struct Scene {
    pub env: Env,
    pub plots: Vec<Plot>,
    pub errors: Vec<Option<String>>,
    pub values: Vec<Option<f64>>,
}
pub struct Plot {
    pub row: usize,
    pub color: usize,
    pub kind: PlotKind,
    pub restrictions: Vec<Expr>,
}
pub enum PlotKind {
    Y(Expr),
    X(Expr),
    Implicit(Expr),
    Region(Expr),
    Polar(Expr, f64, f64),
    Parametric(Expr, Expr, f64, f64),
    Points(Vec<[f64; 2]>),
}
#[derive(Default, Clone)]
pub struct Geometry {
    pub lines: Vec<(usize, Vec<[f64; 2]>)>,
    pub points: Vec<(usize, [f64; 2])>,
    pub fills: Vec<(usize, [f64; 4])>,
}

impl Plot {
    fn permitted(&self, env: &Env) -> bool {
        self.restrictions
            .iter()
            .all(|e| e.eval(env).is_ok_and(|v| v != 0.0))
    }
    fn at(&self, u: f64, env: &mut Env) -> Option<[f64; 2]> {
        let p = match &self.kind {
            PlotKind::Y(e) => {
                env.vars.insert("x".into(), u);
                [u, e.eval(env).ok()?]
            }
            PlotKind::X(e) => {
                env.vars.insert("y".into(), u);
                [e.eval(env).ok()?, u]
            }
            PlotKind::Polar(e, _, _) => {
                env.vars.insert("theta".into(), u);
                let r = e.eval(env).ok()?;
                let a = if env.degrees { u.to_radians() } else { u };
                [r * a.cos(), r * a.sin()]
            }
            PlotKind::Parametric(a, b, _, _) => {
                env.vars.insert("t".into(), u);
                [a.eval(env).ok()?, b.eval(env).ok()?]
            }
            _ => return None,
        };
        env.vars.insert("x".into(), p[0]);
        env.vars.insert("y".into(), p[1]);
        if self.permitted(env) && p.iter().all(|v| v.is_finite()) {
            Some(p)
        } else {
            None
        }
    }
}

impl Scene {
    pub fn geometry(&self, v: Viewport, width: usize, height: usize) -> Geometry {
        let mut out = Geometry::default();
        if !v.valid() {
            return out;
        }
        let w = width.clamp(100, 2400);
        let h = height.clamp(100, 1600);
        for plot in &self.plots {
            let mut env = self.env.clone();
            match &plot.kind {
                PlotKind::Points(points) => {
                    for &point in points {
                        env.vars.insert("x".into(), point[0]);
                        env.vars.insert("y".into(), point[1]);
                        if plot.permitted(&env)
                            && point.iter().all(|n| n.is_finite())
                            && point[0] >= v.xmin
                            && point[0] <= v.xmax
                            && point[1] >= v.ymin
                            && point[1] <= v.ymax
                        {
                            out.points.push((plot.color, point));
                        }
                    }
                }
                PlotKind::Implicit(e) => {
                    implicit(plot, e, None, &mut env, v, (w, h), &mut out);
                }
                PlotKind::Region(predicate) => {
                    let mut boundaries = Vec::new();
                    comparison_boundaries(predicate, &mut boundaries);
                    for (i, boundary) in boundaries.iter().enumerate() {
                        implicit(
                            plot,
                            boundary,
                            if i == 0 { Some(predicate) } else { None },
                            &mut env,
                            v,
                            (w, h),
                            &mut out,
                        );
                    }
                }
                kind => {
                    let (a, b) = match kind {
                        PlotKind::X(_) => (v.ymin, v.ymax),
                        PlotKind::Polar(_, a, b) => (*a, *b),
                        PlotKind::Parametric(_, _, a, b) => (*a, *b),
                        _ => (v.xmin, v.xmax),
                    };
                    let count = (w * 2).clamp(400, 4000);
                    let mut line = Vec::new();
                    let mut previous_u = a;
                    let mut sampler = CurveSampler {
                        plot,
                        env: &mut env,
                        view: v,
                        size: (w, h),
                        remaining: 12000,
                    };
                    for i in 0..=count {
                        let u = a + (b - a) * i as f64 / count as f64;
                        if let Some(point) = sampler.point(u) {
                            if let Some(&last) = line.last() {
                                sampler.append(
                                    (previous_u, last),
                                    (u, point),
                                    0,
                                    &mut line,
                                    &mut out,
                                );
                            } else {
                                line.push(point);
                            }
                        } else {
                            flush(&mut line, plot.color, &mut out);
                        }
                        previous_u = u;
                    }
                    flush(&mut line, plot.color, &mut out);
                }
            }
        }
        out
    }
    /// Find the closest sampled graph location for coordinate tracing.
    pub fn trace(
        &self,
        geometry: &Geometry,
        v: Viewport,
        p: [f64; 2],
        w: f64,
        h: f64,
    ) -> Option<(usize, [f64; 2])> {
        let mut best = None;
        let mut distance = 18.0;
        for (color, points) in &geometry.lines {
            for pair in points.windows(2) {
                let a = v.to_screen(pair[0], w, h);
                let b = v.to_screen(pair[1], w, h);
                let dx = b[0] - a[0];
                let dy = b[1] - a[1];
                let denominator = dx * dx + dy * dy;
                if denominator < 1e-12 {
                    continue;
                }
                let t = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / denominator;
                let t = t.clamp(0.0, 1.0);
                let screen = [a[0] + dx * t, a[1] + dy * t];
                let d = (screen[0] - p[0]).hypot(screen[1] - p[1]);
                if d < distance {
                    distance = d;
                    best = Some((
                        *color,
                        [
                            pair[0][0] + (pair[1][0] - pair[0][0]) * t,
                            pair[0][1] + (pair[1][1] - pair[0][1]) * t,
                        ],
                    ));
                }
            }
        }
        for &(color, point) in &geometry.points {
            let s = v.to_screen(point, w, h);
            let d = (s[0] - p[0]).hypot(s[1] - p[1]);
            if d < distance {
                distance = d;
                best = Some((color, point));
            }
        }
        best
    }
}
fn flush(line: &mut Vec<[f64; 2]>, color: usize, out: &mut Geometry) {
    if line.len() > 1 {
        out.lines.push((color, std::mem::take(line)));
    } else {
        line.clear();
    }
}

fn implicit(
    plot: &Plot,
    e: &Expr,
    predicate: Option<&Expr>,
    env: &mut Env,
    v: Viewport,
    (w, h): (usize, usize),
    out: &mut Geometry,
) {
    let nx = (w / 5).clamp(40, 220);
    let ny = (h / 5).clamp(40, 180);
    let dx = (v.xmax - v.xmin) / nx as f64;
    let dy = (v.ymax - v.ymin) / ny as f64;
    let mut values = vec![None; (nx + 1) * (ny + 1)];
    for j in 0..=ny {
        for i in 0..=nx {
            env.vars.insert("x".into(), v.xmin + i as f64 * dx);
            env.vars.insert("y".into(), v.ymin + j as f64 * dy);
            if plot.permitted(env) {
                values[j * (nx + 1) + i] = e.eval(env).ok();
            }
        }
    }
    for j in 0..ny {
        for i in 0..nx {
            let x = v.xmin + i as f64 * dx;
            let y = v.ymin + j as f64 * dy;
            let p = [[x, y], [x + dx, y], [x + dx, y + dy], [x, y + dy]];
            let z = [
                values[j * (nx + 1) + i],
                values[j * (nx + 1) + i + 1],
                values[(j + 1) * (nx + 1) + i + 1],
                values[(j + 1) * (nx + 1) + i],
            ];
            if let Some(predicate) = predicate {
                env.vars.insert("x".into(), x + dx / 2.0);
                env.vars.insert("y".into(), y + dy / 2.0);
                if plot.permitted(env) && predicate.eval(env).is_ok_and(|value| value != 0.0) {
                    out.fills.push((plot.color, [x, y, dx, dy]));
                }
            }
            let mut crossings = Vec::new();
            for k in 0..4 {
                let n = (k + 1) % 4;
                if let (Some(a), Some(b)) = (z[k], z[n])
                    && ((a < 0.0) != (b < 0.0) || a == 0.0)
                {
                    let denominator = a - b;
                    if denominator.abs() < 1e-20 {
                        continue;
                    }
                    let t = (a / denominator).clamp(0.0, 1.0);
                    let point = [
                        p[k][0] + t * (p[n][0] - p[k][0]),
                        p[k][1] + t * (p[n][1] - p[k][1]),
                    ];
                    // A sign change through an asymptote is not a contour.
                    env.vars.insert("x".into(), point[0]);
                    env.vars.insert("y".into(), point[1]);
                    if e.eval(env)
                        .is_ok_and(|value| value.abs() <= 0.2 * (a.abs() + b.abs()).max(1e-10))
                    {
                        crossings.push(point);
                    }
                }
            }
            for pair in crossings.chunks_exact(2) {
                out.lines.push((plot.color, pair.to_vec()));
            }
        }
    }
}

pub fn grid_step(span: f64, pixels: f64) -> f64 {
    let rough = span / (pixels / 85.0).max(2.0);
    let base = 10.0_f64.powf(rough.log10().floor());
    let n = rough / base;
    base * if n <= 1.0 {
        1.0
    } else if n <= 2.0 {
        2.0
    } else if n <= 5.0 {
        5.0
    } else {
        10.0
    }
}
pub fn format_number(n: f64) -> String {
    if n.abs() < 1e-10 {
        "0".into()
    } else if n.abs() >= 1e6 || n.abs() < 1e-4 {
        format!("{n:.3e}")
    } else {
        format!("{n:.5}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .into()
    }
}

fn comparison_boundaries(e: &Expr, out: &mut Vec<Expr>) {
    match e {
        Expr::Compare(values, _) => {
            for pair in values.windows(2) {
                if out.len() < 32 {
                    out.push(Expr::Binary(
                        '-',
                        Box::new(pair[0].clone()),
                        Box::new(pair[1].clone()),
                    ));
                }
            }
        }
        Expr::Binary('&' | '|', a, b) => {
            comparison_boundaries(a, out);
            comparison_boundaries(b, out);
        }
        _ => {}
    }
}

struct CurveSampler<'a> {
    plot: &'a Plot,
    env: &'a mut Env,
    view: Viewport,
    size: (usize, usize),
    remaining: usize,
}
impl CurveSampler<'_> {
    fn point(&mut self, u: f64) -> Option<[f64; 2]> {
        let point = self.plot.at(u, self.env)?;
        let (w, h) = (self.size.0 as f64, self.size.1 as f64);
        let screen = self.view.to_screen(point, w, h);
        if screen.iter().all(|n| n.is_finite())
            && screen[0] > -w * 4.0
            && screen[0] < w * 5.0
            && screen[1] > -h * 4.0
            && screen[1] < h * 5.0
        {
            Some(point)
        } else {
            None
        }
    }
    fn append(
        &mut self,
        a: (f64, [f64; 2]),
        b: (f64, [f64; 2]),
        depth: u8,
        line: &mut Vec<[f64; 2]>,
        out: &mut Geometry,
    ) {
        if self.remaining == 0 {
            flush(line, self.plot.color, out);
            line.push(b.1);
            return;
        }
        self.remaining -= 1;
        let u = (a.0 + b.0) / 2.0;
        let Some(mid) = self.point(u) else {
            flush(line, self.plot.color, out);
            line.push(b.1);
            return;
        };
        let w = self.size.0 as f64;
        let h = self.size.1 as f64;
        let sa = self.view.to_screen(a.1, w, h);
        let sb = self.view.to_screen(b.1, w, h);
        let sm = self.view.to_screen(mid, w, h);
        let deviation = (sm[0] - (sa[0] + sb[0]) / 2.0).hypot(sm[1] - (sa[1] + sb[1]) / 2.0);
        // Relative midpoint excursions detect poles even when their amplitude is subpixel.
        let excursion = (0..2).any(|i| {
            let low = a.1[i].min(b.1[i]);
            let high = a.1[i].max(b.1[i]);
            let tolerance = ((high - low).abs() + low.abs() + high.abs()).max(1e-300) * 1e-12;
            mid[i] < low - tolerance || mid[i] > high + tolerance
        });
        if deviation > 0.7 || excursion {
            if depth >= 10 {
                flush(line, self.plot.color, out);
                line.push(b.1);
            } else {
                self.append(a, (u, mid), depth + 1, line, out);
                self.append((u, mid), b, depth + 1, line, out);
            }
        } else {
            line.push(b.1);
        }
    }
}
