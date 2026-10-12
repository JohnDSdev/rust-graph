//! Bounded numerical feature discovery. Coordinates are estimates, not symbolic proofs.
use crate::{
    graph::{Geometry, Plot, PlotKind, Scene, Viewport},
    math::Env,
};
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FeatureKind {
    Maximum,
    Minimum,
    XIntercept,
    YIntercept,
    Intersection,
}
#[derive(Clone, Debug)]
pub struct FeaturePoint {
    pub position: [f64; 2],
    pub kind: FeatureKind,
    pub rows: Vec<usize>,
}
const LIMIT: usize = 512;
fn permitted(p: &Plot, e: &Env) -> bool {
    p.restrictions
        .iter()
        .all(|r| r.eval(e).is_ok_and(|v| v != 0.))
}
fn point(p: &Plot, u: f64, env: &Env) -> Option<[f64; 2]> {
    let mut e = env.clone();
    let q = match &p.kind {
        PlotKind::Y(a) => {
            e.vars.insert("x".into(), u);
            [u, a.eval(&e).ok()?]
        }
        PlotKind::X(a) => {
            e.vars.insert("y".into(), u);
            [a.eval(&e).ok()?, u]
        }
        PlotKind::Polar(a, _, _) => {
            e.vars.insert("theta".into(), u);
            let r = a.eval(&e).ok()?;
            let t = if e.degrees { u.to_radians() } else { u };
            [r * t.cos(), r * t.sin()]
        }
        PlotKind::Parametric(a, b, _, _) => {
            e.vars.insert("t".into(), u);
            [a.eval(&e).ok()?, b.eval(&e).ok()?]
        }
        _ => return None,
    };
    e.vars.insert("x".into(), q[0]);
    e.vars.insert("y".into(), q[1]);
    (q.iter().all(|v| v.is_finite()) && permitted(p, &e)).then_some(q)
}
fn residual(p: &Plot, q: [f64; 2], env: &Env) -> Option<f64> {
    let mut e = env.clone();
    e.vars.insert("x".into(), q[0]);
    e.vars.insert("y".into(), q[1]);
    if !permitted(p, &e) {
        return None;
    }
    let v = match &p.kind {
        PlotKind::Y(a) => q[1] - a.eval(&e).ok()?,
        PlotKind::X(a) => q[0] - a.eval(&e).ok()?,
        PlotKind::Implicit(a) => a.eval(&e).ok()?,
        _ => return None,
    };
    v.is_finite().then_some(v)
}
fn roots(f: &impl Fn(f64) -> Option<f64>, a: f64, b: f64) -> Vec<f64> {
    let n = 512;
    let h = (b - a) / n as f64;
    let vals: Vec<_> = (0..=n).map(|i| f(a + h * i as f64)).collect();
    let mut out = Vec::new();
    for i in 0..n {
        let (Some(fa), Some(fb)) = (vals[i], vals[i + 1]) else {
            continue;
        };
        if fa == 0.0 && (fb != 0.0 || (i > 0 && vals[i - 1].is_some_and(|v| v != 0.0))) {
            out.push(a + h * i as f64);
        }
        if fa.signum() != fb.signum() {
            let (mut lo, mut hi, mut fl) = (a + h * i as f64, a + h * (i + 1) as f64, fa);
            for _ in 0..60 {
                let m = (lo + hi) / 2.;
                let Some(fm) = f(m) else { break };
                if fm == 0.0 {
                    lo = m;
                    hi = m;
                    break;
                }
                if fm.signum() == fl.signum() {
                    lo = m;
                    fl = fm
                } else {
                    hi = m
                }
            }
            let x = (lo + hi) / 2.;
            if f(x).is_some_and(|v| v.abs() <= 1e-10 * fa.abs().max(fb.abs()).min(1.0)) {
                out.push(x);
            }
        }
    }
    // Tangencies do not change sign: refine local minima of the absolute residual.
    for i in 1..n {
        let (Some(l), Some(c), Some(r)) = (vals[i - 1], vals[i], vals[i + 1]) else {
            continue;
        };
        if c.abs() < l.abs() && c.abs() < r.abs() {
            let (mut lo, mut hi) = (a + h * (i - 1) as f64, a + h * (i + 1) as f64);
            for _ in 0..64 {
                let x = lo + (hi - lo) / 3.;
                let y = hi - (hi - lo) / 3.;
                match (f(x), f(y)) {
                    (Some(fx), Some(fy)) => {
                        if fx.abs() < fy.abs() {
                            hi = y
                        } else {
                            lo = x
                        }
                    }
                    _ => break,
                }
            }
            let x = (lo + hi) / 2.;
            if f(x).is_some_and(|v| v.abs() <= 1e-10 * l.abs().max(r.abs()).min(1.0)) {
                out.push(x)
            }
        }
    }
    out
}
fn derivative(f: &impl Fn(f64) -> Option<f64>, u: f64) -> Option<f64> {
    let h = 1e-5 * (1. + u.abs());
    Some((f(u + h)? - f(u - h)?) / (2. * h))
}
fn add(out: &mut Vec<FeaturePoint>, v: Viewport, q: [f64; 2], kind: FeatureKind, rows: Vec<usize>) {
    if out.len() >= LIMIT
        || !q.iter().all(|x| x.is_finite())
        || q[0] < v.xmin
        || q[0] > v.xmax
        || q[1] < v.ymin
        || q[1] > v.ymax
    {
        return;
    }
    if !out.iter().any(|p| {
        p.kind == kind
            && p.rows == rows
            && (p.position[0] - q[0]).abs() < 1e-7 * (1. + q[0].abs())
            && (p.position[1] - q[1]).abs() < 1e-7 * (1. + q[1].abs())
    }) {
        out.push(FeaturePoint {
            position: q,
            kind,
            rows,
        });
    }
}
fn solve2(
    f: &impl Fn([f64; 2]) -> Option<[f64; 2]>,
    mut q: [f64; 2],
    v: Viewport,
) -> Option<[f64; 2]> {
    for _ in 0..32 {
        let z = f(q)?;
        let hx = 1e-5 * (1. + q[0].abs());
        let hy = 1e-5 * (1. + q[1].abs());
        let a = f([q[0] + hx, q[1]])?;
        let b = f([q[0] - hx, q[1]])?;
        let c = f([q[0], q[1] + hy])?;
        let d = f([q[0], q[1] - hy])?;
        let mut j = [
            (a[0] - b[0]) / (2. * hx),
            (c[0] - d[0]) / (2. * hy),
            (a[1] - b[1]) / (2. * hx),
            (c[1] - d[1]) / (2. * hy),
        ];
        // Normalize each equation by its gradient before checking rank or
        // convergence: multiplying an equation by any nonzero scale must not
        // change its zero set or turn arbitrary seed positions into roots.
        let norms = [j[0].hypot(j[1]), j[2].hypot(j[3])];
        if norms.iter().any(|n| !n.is_finite() || *n == 0.0) {
            return None;
        }
        j[0] /= norms[0];
        j[1] /= norms[0];
        j[2] /= norms[1];
        j[3] /= norms[1];
        let r = [z[0] / norms[0], z[1] / norms[1]];
        let det = j[0] * j[3] - j[1] * j[2];
        // Coincident curves and poorly conditioned crossings do not define
        // a reliably isolated point.
        if !det.is_finite() || det.abs() < 1e-10 {
            return None;
        }
        let dx = (r[0] * j[3] - r[1] * j[1]) / det;
        let dy = (j[0] * r[1] - j[2] * r[0]) / det;
        if !dx.is_finite() || !dy.is_finite() || r.iter().any(|n| !n.is_finite()) {
            return None;
        }
        let tolerance = 1e-12 * (1.0 + q[0].abs().max(q[1].abs()));
        if dx.abs().max(dy.abs()) <= tolerance && r[0].abs().max(r[1].abs()) <= tolerance {
            return Some(q);
        }
        let scale = (dx.abs() / (v.xmax - v.xmin) * 4.)
            .max(dy.abs() / (v.ymax - v.ymin) * 4.)
            .max(1.);
        q[0] -= dx / scale;
        q[1] -= dy / scale;
        if q[0] < v.xmin || q[0] > v.xmax || q[1] < v.ymin || q[1] > v.ymax {
            return None;
        }
    }
    None
}
pub fn features(scene: &Scene, view: Viewport, _geometry: &Geometry) -> Vec<FeaturePoint> {
    let mut out = Vec::new();
    if !view.valid() {
        return out;
    }
    let plots: Vec<_> = scene.plots.iter().take(64).collect();
    for p in &plots {
        if out.len() >= LIMIT {
            return out;
        }
        let range = match p.kind {
            PlotKind::Y(_) => (view.xmin, view.xmax),
            PlotKind::X(_) => (view.ymin, view.ymax),
            PlotKind::Polar(_, a, b) | PlotKind::Parametric(_, _, a, b) => (a, b),
            _ => continue,
        };
        for axis in 0..2 {
            let f = |u| point(p, u, &scene.env).map(|q| q[axis]);
            for u in roots(&f, range.0, range.1) {
                if let Some(q) = point(p, u, &scene.env) {
                    add(
                        &mut out,
                        view,
                        q,
                        if axis == 0 {
                            FeatureKind::YIntercept
                        } else {
                            FeatureKind::XIntercept
                        },
                        vec![p.row],
                    );
                }
            }
        }
        let axis = if matches!(p.kind, PlotKind::X(_)) {
            0
        } else {
            1
        };
        let f = |u| point(p, u, &scene.env).map(|q| q[axis]);
        let df = |u| derivative(&f, u);
        for u in roots(&df, range.0, range.1) {
            let h = (range.1 - range.0) * 1e-4;
            if let (Some(l), Some(c), Some(r), Some(q)) =
                (f(u - h), f(u), f(u + h), point(p, u, &scene.env))
            {
                if c < l && c < r {
                    add(&mut out, view, q, FeatureKind::Minimum, vec![p.row]);
                } else if c > l && c > r {
                    add(&mut out, view, q, FeatureKind::Maximum, vec![p.row]);
                }
            }
        }
    }
    // Implicit intercepts and vertical apexes use residual/gradient equations.
    for p in &plots {
        if !matches!(p.kind, PlotKind::Implicit(_)) {
            continue;
        }
        for axis in 0..2 {
            let (a, b) = if axis == 0 {
                (view.ymin, view.ymax)
            } else {
                (view.xmin, view.xmax)
            };
            let f = |u| residual(p, if axis == 0 { [0., u] } else { [u, 0.] }, &scene.env);
            for u in roots(&f, a, b) {
                add(
                    &mut out,
                    view,
                    if axis == 0 { [0., u] } else { [u, 0.] },
                    if axis == 0 {
                        FeatureKind::YIntercept
                    } else {
                        FeatureKind::XIntercept
                    },
                    vec![p.row],
                );
            }
        }
        let f = |q: [f64; 2]| {
            let g = |x| residual(p, [x, q[1]], &scene.env);
            Some([residual(p, q, &scene.env)?, derivative(&g, q[0])?])
        };
        for i in 0..9 {
            for j in 0..9 {
                let seed = [
                    view.xmin + (view.xmax - view.xmin) * i as f64 / 8.,
                    view.ymin + (view.ymax - view.ymin) * j as f64 / 8.,
                ];
                if let Some(q) = solve2(&f, seed, view) {
                    let h = 1e-4 * (1. + q[1].abs());
                    let top = residual(p, [q[0], q[1] + h], &scene.env);
                    let bottom = residual(p, [q[0], q[1] - h], &scene.env);
                    let side = residual(p, [q[0] + h, q[1]], &scene.env);
                    if let (Some(t), Some(b), Some(s)) = (top, bottom, side) {
                        let kind = if t * s > 0. && b * s < 0. {
                            Some(FeatureKind::Maximum)
                        } else if b * s > 0. && t * s < 0. {
                            Some(FeatureKind::Minimum)
                        } else {
                            None
                        };
                        if let Some(k) = kind {
                            add(&mut out, view, q, k, vec![p.row]);
                        }
                    }
                }
            }
        }
    }
    let mut pairs = 0;
    for (i, p) in plots.iter().enumerate() {
        for q in plots.iter().skip(i + 1) {
            if pairs >= 64 || out.len() >= LIMIT {
                return out;
            }
            pairs += 1;
            let rows = vec![p.row, q.row];
            let pr = parameter_range(p, view);
            let qr = parameter_range(q, view);
            if let PlotKind::Points(points) = &p.kind {
                point_intersections(&mut out, points, p, q, scene, view, &rows);
                continue;
            }
            if let PlotKind::Points(points) = &q.kind {
                point_intersections(&mut out, points, q, p, scene, view, &rows);
                continue;
            }
            let curve = if pr.is_some() && has_residual(q) {
                Some(*p)
            } else if qr.is_some() && has_residual(p) {
                Some(*q)
            } else {
                None
            };
            if let Some(c) = curve {
                let other = if std::ptr::eq(c, *p) { *q } else { *p };
                let (a, b) = parameter_range(c, view).unwrap();
                let f = |u| residual(other, point(c, u, &scene.env)?, &scene.env);
                for u in roots(&f, a, b) {
                    if let Some(pos) = point(c, u, &scene.env) {
                        add(&mut out, view, pos, FeatureKind::Intersection, rows.clone());
                    }
                }
            } else if let (Some((a, b)), Some((c, d))) = (pr, qr) {
                let bounds = Viewport {
                    xmin: a,
                    xmax: b,
                    ymin: c,
                    ymax: d,
                    ..view
                };
                let f = |uv: [f64; 2]| {
                    let x = point(p, uv[0], &scene.env)?;
                    let y = point(q, uv[1], &scene.env)?;
                    Some([x[0] - y[0], x[1] - y[1]])
                };
                for i in 0..5 {
                    for j in 0..5 {
                        let seed = [a + (b - a) * i as f64 / 4., c + (d - c) * j as f64 / 4.];
                        if let Some(uv) = solve2(&f, seed, bounds)
                            && let Some(pos) = point(p, uv[0], &scene.env)
                        {
                            add(&mut out, view, pos, FeatureKind::Intersection, rows.clone());
                        }
                    }
                }
            } else if matches!(p.kind, PlotKind::Implicit(_))
                && matches!(q.kind, PlotKind::Implicit(_))
            {
                let f = |x| Some([residual(p, x, &scene.env)?, residual(q, x, &scene.env)?]);
                for i in 0..7 {
                    for j in 0..7 {
                        let seed = [
                            view.xmin + (view.xmax - view.xmin) * i as f64 / 6.,
                            view.ymin + (view.ymax - view.ymin) * j as f64 / 6.,
                        ];
                        if let Some(pos) = solve2(&f, seed, view) {
                            add(&mut out, view, pos, FeatureKind::Intersection, rows.clone());
                        }
                    }
                }
            }
        }
    }
    out
}

fn has_residual(p: &Plot) -> bool {
    matches!(
        p.kind,
        PlotKind::Y(_) | PlotKind::X(_) | PlotKind::Implicit(_)
    )
}
fn parameter_range(p: &Plot, v: Viewport) -> Option<(f64, f64)> {
    match p.kind {
        PlotKind::Y(_) => Some((v.xmin, v.xmax)),
        PlotKind::X(_) => Some((v.ymin, v.ymax)),
        PlotKind::Polar(_, a, b) | PlotKind::Parametric(_, _, a, b) => Some((a, b)),
        _ => None,
    }
}
fn point_intersections(
    out: &mut Vec<FeaturePoint>,
    points: &[[f64; 2]],
    source: &Plot,
    other: &Plot,
    scene: &Scene,
    view: Viewport,
    rows: &[usize],
) {
    for &pos in points.iter().take(128) {
        let mut env = scene.env.clone();
        env.vars.insert("x".into(), pos[0]);
        env.vars.insert("y".into(), pos[1]);
        if !pos.iter().all(|v| v.is_finite()) || !permitted(source, &env) {
            continue;
        }
        let hit = if has_residual(other) {
            residual(other, pos, &scene.env).is_some_and(|r| r.abs() < 1e-12)
        } else if let PlotKind::Points(q) = &other.kind {
            q.iter()
                .take(128)
                .any(|q| (q[0] - pos[0]).hypot(q[1] - pos[1]) < 1e-10)
                && permitted(other, &env)
        } else if let Some((a, b)) = parameter_range(other, view) {
            let f = |u| {
                let q = point(other, u, &scene.env)?;
                Some((q[0] - pos[0]).hypot(q[1] - pos[1]))
            };
            !roots(&f, a, b).is_empty()
        } else {
            false
        };
        if hit {
            add(out, view, pos, FeatureKind::Intersection, rows.to_vec());
        }
    }
}
