//! Bounded least-squares fitting and numerical tables.
use crate::graph::{PlotKind, Scene};
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RegressionModel {
    Linear,
    Quadratic,
    Cubic,
    Polynomial(u8),
    Exponential,
    Logarithmic,
    Power,
    Sinusoidal,
}
impl RegressionModel {
    pub fn label(&self) -> &str {
        match self {
            Self::Linear => "Linear",
            Self::Quadratic => "Quadratic",
            Self::Cubic => "Cubic",
            Self::Polynomial(_) => "Polynomial",
            Self::Exponential => "Exponential",
            Self::Logarithmic => "Logarithmic",
            Self::Power => "Power",
            Self::Sinusoidal => "Sinusoidal",
        }
    }
}
pub struct Fit {
    pub equation: String,
    pub r_squared: f64,
    pub rmse: f64,
    pub parameters: Vec<f64>,
}
fn number(x: f64) -> String {
    format!("{x:.16e}").replace("e", "e+").replace("e+-", "e-")
}
// Modified Gram-Schmidt with a second orthogonalization pass and column scaling.
fn solve(mut a: Vec<Vec<f64>>, y: &[f64]) -> Result<Vec<f64>, String> {
    let n = a.len();
    let mut r = vec![vec![0.; n]; n];
    let mut scales = vec![0.; n];
    let mut rhs = vec![0.; n];
    for j in 0..n {
        scales[j] = a[j].iter().map(|v| v * v).sum::<f64>().sqrt();
        if scales[j] == 0. || !scales[j].is_finite() {
            return Err("Rank-deficient data".into());
        }
        for v in &mut a[j] {
            *v /= scales[j];
        }
        for _ in 0..2 {
            for i in 0..j {
                let d = a[i].iter().zip(&a[j]).map(|(x, y)| x * y).sum::<f64>();
                r[i][j] += d;
                let (previous, current) = a.split_at_mut(j);
                for (value, basis) in current[0].iter_mut().zip(&previous[i]) {
                    *value -= d * basis;
                }
            }
        }
        r[j][j] = a[j].iter().map(|v| v * v).sum::<f64>().sqrt();
        if r[j][j] < 1e-11 {
            return Err("Rank-deficient data".into());
        }
        for v in &mut a[j] {
            *v /= r[j][j];
        }
        rhs[j] = a[j].iter().zip(y).map(|(x, y)| x * y).sum();
    }
    let mut b = vec![0.; n];
    for i in (0..n).rev() {
        b[i] = (rhs[i] - (i + 1..n).map(|j| r[i][j] * b[j]).sum::<f64>()) / r[i][i];
    }
    for i in 0..n {
        b[i] /= scales[i];
    }
    if b.iter().any(|v| !v.is_finite()) {
        return Err("Fit overflow".into());
    }
    Ok(b)
}
pub fn fit(points: &[[f64; 2]], model: RegressionModel) -> Result<Fit, String> {
    if points.len() < 2 || points.len() > 2048 || points.iter().flatten().any(|v| !v.is_finite()) {
        return Err("Provide 2–2048 finite points".into());
    }
    let x: Vec<f64> = points.iter().map(|p| p[0]).collect();
    let y: Vec<f64> = points.iter().map(|p| p[1]).collect();
    let lo = x.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let center = lo / 2. + hi / 2.;
    let scale = (hi - lo) / 2.;
    if !scale.is_finite() || scale <= 0. {
        return Err("Distinct finite x values required".into());
    }
    let mut predict = Vec::new();
    let parameters;
    let expression;
    match model {
        RegressionModel::Linear
        | RegressionModel::Quadratic
        | RegressionModel::Cubic
        | RegressionModel::Polynomial(_) => {
            let d = match model {
                RegressionModel::Linear => 1,
                RegressionModel::Quadratic => 2,
                RegressionModel::Cubic => 3,
                RegressionModel::Polynomial(d) => d,
                _ => unreachable!(),
            };
            if !(1..=6).contains(&d) || points.len() <= d as usize {
                return Err("Polynomial degree must be 1–6 with enough points".into());
            }
            let z: Vec<f64> = x.iter().map(|x| (x - center) / scale).collect();
            parameters = solve(
                (0..=d)
                    .map(|i| z.iter().map(|v| v.powi(i as i32)).collect())
                    .collect(),
                &y,
            )?;
            expression = parameters
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    format!(
                        "({})*((x-({}))/({}))^{}",
                        number(*c),
                        number(center),
                        number(scale),
                        i
                    )
                })
                .collect::<Vec<_>>()
                .join("+");
            for z in z {
                predict.push(parameters.iter().rev().fold(0., |v, c| v * z + c));
            }
        }
        RegressionModel::Sinusoidal => {
            if points.len() < 8 {
                return Err("Sinusoidal fit requires at least eight points".into());
            }
            let span = hi - lo;
            let min = std::f64::consts::TAU / span * 0.25;
            let max = std::f64::consts::PI / span * (points.len().min(256) - 1) as f64;
            let eval = |w: f64| -> Option<(f64, Vec<f64>)> {
                let b = solve(
                    vec![
                        vec![1.; x.len()],
                        x.iter().map(|v| (w * (v - center)).sin()).collect(),
                        x.iter().map(|v| (w * (v - center)).cos()).collect(),
                    ],
                    &y,
                )
                .ok()?;
                let error = x
                    .iter()
                    .zip(&y)
                    .map(|(x, y)| {
                        let r = b[0]
                            + b[1] * (w * (x - center)).sin()
                            + b[2] * (w * (x - center)).cos()
                            - y;
                        r * r
                    })
                    .sum();
                Some((error, b))
            };
            let step = (max - min) / 1024.;
            let mut best = (f64::INFINITY, min, Vec::new());
            for i in 0..=1024 {
                let w = min + step * i as f64;
                if let Some((e, b)) = eval(w)
                    && e < best.0
                {
                    best = (e, w, b);
                }
            }
            let mut a = (best.1 - step).max(min);
            let mut b = (best.1 + step).min(max);
            for _ in 0..80 {
                let l = a + (b - a) / 3.;
                let r = b - (b - a) / 3.;
                if eval(l).map_or(f64::INFINITY, |v| v.0) < eval(r).map_or(f64::INFINITY, |v| v.0) {
                    b = r
                } else {
                    a = l
                }
            }
            let w = (a + b) / 2.;
            let (_, coeff) = eval(w).ok_or("Sinusoidal fit failed")?;
            parameters = vec![coeff[0], coeff[1], coeff[2], w, center];
            expression = format!(
                "({})+({})*sin(({})*(x-({})))+({})*cos(({})*(x-({})))",
                number(coeff[0]),
                number(coeff[1]),
                number(w),
                number(center),
                number(coeff[2]),
                number(w),
                number(center)
            );
            predict = x
                .iter()
                .map(|v| {
                    coeff[0]
                        + coeff[1] * (w * (v - center)).sin()
                        + coeff[2] * (w * (v - center)).cos()
                })
                .collect();
        }
        _ => {
            let logx = matches!(model, RegressionModel::Logarithmic | RegressionModel::Power);
            let logy = matches!(model, RegressionModel::Exponential | RegressionModel::Power);
            if points
                .iter()
                .any(|p| (logx && p[0] <= 0.) || (logy && p[1] <= 0.))
            {
                return Err("Model requires positive values".into());
            }
            let tx: Vec<f64> = x.iter().map(|v| if logx { v.ln() } else { *v }).collect();
            let ty: Vec<f64> = y.iter().map(|v| if logy { v.ln() } else { *v }).collect();
            let c = tx.iter().sum::<f64>() / tx.len() as f64;
            let s = tx.iter().map(|v| (v - c).abs()).fold(0., f64::max);
            if s <= 0. {
                return Err("Rank-deficient data".into());
            }
            let b = solve(
                vec![vec![1.; x.len()], tx.iter().map(|v| (v - c) / s).collect()],
                &ty,
            )?;
            let slope = b[1] / s;
            // Transformed models retain the centered log intercept, slope and
            // transformed-x center. Avoid exponentiating an extrapolated
            // intercept: it can underflow even when all fitted y are ordinary.
            parameters = vec![b[0], slope, c];
            expression = match model {
                RegressionModel::Exponential => format!(
                    "exp(({})+({})*(x-({})))",
                    number(b[0]),
                    number(slope),
                    number(c)
                ),
                RegressionModel::Power => format!(
                    "exp(({})+({})*(ln(x)-({})))",
                    number(b[0]),
                    number(slope),
                    number(c)
                ),
                _ => format!(
                    "({})+({})*(ln(x)-({}))",
                    number(b[0]),
                    number(slope),
                    number(c)
                ),
            };
            predict = tx
                .iter()
                .map(|v| {
                    let q = b[0] + b[1] * (v - c) / s;
                    if logy { q.exp() } else { q }
                })
                .collect();
        }
    }
    // Compute diagnostics from the actual serialized equation, so coefficients
    // and parser arithmetic must remain usable at every training point.
    let emitted = crate::math::Expr::parse(&expression)?;
    let mut env = crate::math::Env::default();
    predict.clear();
    for &value in &x {
        env.vars.insert("x".into(), value);
        predict.push(
            emitted
                .eval(&env)
                .map_err(|_| "Emitted fit is undefined at a training point")?,
        );
    }
    let mean = y.iter().sum::<f64>() / y.len() as f64;
    let sse = predict
        .iter()
        .zip(&y)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>();
    let sst = y.iter().map(|v| (v - mean).powi(2)).sum::<f64>();
    if !sse.is_finite() || !sst.is_finite() || parameters.iter().any(|v| !v.is_finite()) {
        return Err("Fit overflow".into());
    }
    Ok(Fit {
        equation: format!("y={expression}"),
        r_squared: if sst == 0. {
            if sse < 1e-24 { 1. } else { 0. }
        } else {
            1. - sse / sst
        },
        rmse: (sse / y.len() as f64).sqrt(),
        parameters,
    })
}
pub fn generate_table(
    scene: &Scene,
    row: usize,
    start: f64,
    end: f64,
    step: f64,
) -> Result<String, String> {
    let span = end - start;
    if ![start, end, step, span].iter().all(|v| v.is_finite()) || span <= 0. || step <= 0. {
        return Err("Use finite increasing bounds and a positive step".into());
    }
    let count = (span / step).floor();
    if !count.is_finite() || count >= 2048. {
        return Err("Table exceeds 2048 attempted rows".into());
    }
    let plot = scene
        .plots
        .iter()
        .find(|p| p.row == row)
        .ok_or("No equation in this row")?;
    if !matches!(
        plot.kind,
        PlotKind::Y(_) | PlotKind::X(_) | PlotKind::Polar(..) | PlotKind::Parametric(..)
    ) {
        return Err("Table requires an explicit, polar or parametric equation".into());
    }
    let mut rows = Vec::new();
    for i in 0..=count as usize {
        let u = start + i as f64 * step;
        let mut env = scene.env.clone();
        let p = match &plot.kind {
            PlotKind::Y(e) => {
                env.vars.insert("x".into(), u);
                e.eval(&env).ok().map(|v| [u, v])
            }
            PlotKind::X(e) => {
                env.vars.insert("y".into(), u);
                e.eval(&env).ok().map(|v| [v, u])
            }
            PlotKind::Polar(e, a, b) => {
                if u < *a || u > *b {
                    continue;
                }
                env.vars.insert("theta".into(), u);
                let t = if env.degrees { u.to_radians() } else { u };
                e.eval(&env).ok().map(|r| [r * t.cos(), r * t.sin()])
            }
            PlotKind::Parametric(a, b, lo, hi) => {
                if u < *lo || u > *hi {
                    continue;
                }
                env.vars.insert("t".into(), u);
                a.eval(&env)
                    .ok()
                    .zip(b.eval(&env).ok())
                    .map(|(x, y)| [x, y])
            }
            _ => None,
        };
        if let Some(p) = p {
            env.vars.insert("x".into(), p[0]);
            env.vars.insert("y".into(), p[1]);
            if p.iter().all(|v| v.is_finite())
                && plot
                    .restrictions
                    .iter()
                    .all(|e| e.eval(&env).is_ok_and(|v| v != 0.))
            {
                rows.push(format!("{},{}", number(p[0]), number(p[1])));
            }
        }
    }
    if rows.is_empty() {
        return Err("No defined points in this range".into());
    }
    Ok(rows.join("\n"))
}
