//! Bounded symbolic proofs and explicitly approximate numerical recognition.
use crate::math::{Env, Expr};

pub fn decimal(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    if !value.is_finite() {
        return value.to_string();
    }
    let scientific = format!("{value:.11e}");
    let (mantissa, exponent) = scientific.split_once('e').unwrap();
    let exponent: i32 = exponent.parse().unwrap();
    if !(-6..12).contains(&exponent) {
        format!("{}e{exponent}", trim(mantissa))
    } else {
        trim(&format!("{:.*}", (11 - exponent).max(0) as usize, value))
    }
}
fn trim(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        s.into()
    }
}
const LIMIT: i128 = 1_000_000_000_000;
#[derive(Clone, Copy, PartialEq, Eq)]
struct Rational {
    n: i128,
    d: i128,
}
fn gcd(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.abs()
}
impl Rational {
    fn new(n: i128, d: i128) -> Option<Self> {
        if d == 0 {
            return None;
        }
        let g = gcd(n, d);
        let n = n / g * d.signum();
        let d = d.abs() / g;
        (n.abs() <= LIMIT && d <= LIMIT).then_some(Self { n, d })
    }
    fn mul(self, b: Self) -> Option<Self> {
        Self::new(self.n.checked_mul(b.n)?, self.d.checked_mul(b.d)?)
    }
    fn div(self, b: Self) -> Option<Self> {
        Self::new(self.n.checked_mul(b.d)?, self.d.checked_mul(b.n)?)
    }
    fn add(self, b: Self) -> Option<Self> {
        Self::new(
            self.n
                .checked_mul(b.d)?
                .checked_add(b.n.checked_mul(self.d)?)?,
            self.d.checked_mul(b.d)?,
        )
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Basis {
    Scalar,
    Pi,
    Root(i128),
}
#[derive(Clone, Copy)]
struct Form {
    coefficient: Rational,
    basis: Basis,
}
impl Form {
    fn scalar(n: i128, d: i128) -> Option<Self> {
        Some(Self {
            coefficient: Rational::new(n, d)?,
            basis: Basis::Scalar,
        })
    }
    fn normalized(self) -> Self {
        if self.coefficient.n == 0 {
            Self {
                basis: Basis::Scalar,
                ..self
            }
        } else {
            self
        }
    }
    fn text(self) -> String {
        let Self {
            coefficient: Rational { n, d },
            basis,
        } = self.normalized();
        let body = match basis {
            Basis::Scalar => n.to_string(),
            _ => {
                let symbol = match basis {
                    Basis::Pi => "π".into(),
                    Basis::Root(r) => format!("√{r}"),
                    _ => unreachable!(),
                };
                match n {
                    1 => symbol,
                    -1 => format!("-{symbol}"),
                    _ => format!("{n}{symbol}"),
                }
            }
        };
        if d == 1 { body } else { format!("{body}/{d}") }
    }
    fn mul(self, b: Self) -> Option<Self> {
        let mut coefficient = self.coefficient.mul(b.coefficient)?;
        let basis = match (self.basis, b.basis) {
            (Basis::Scalar, x) | (x, Basis::Scalar) => x,
            (Basis::Root(a), Basis::Root(b)) => {
                let (outside, inside) = square_factors(a.checked_mul(b)?)?;
                coefficient = coefficient.mul(Rational::new(outside, 1)?)?;
                if inside == 1 {
                    Basis::Scalar
                } else {
                    Basis::Root(inside)
                }
            }
            _ => return None,
        };
        Some(Self { coefficient, basis }.normalized())
    }
    fn inverse(self) -> Option<Self> {
        let mut coefficient = Rational::new(self.coefficient.d, self.coefficient.n)?;
        let basis = match self.basis {
            Basis::Pi => return None,
            Basis::Root(r) => {
                coefficient = coefficient.div(Rational::new(r, 1)?)?;
                Basis::Root(r)
            }
            Basis::Scalar => Basis::Scalar,
        };
        Some(Self { coefficient, basis })
    }
}
fn square_factors(mut n: i128) -> Option<(i128, i128)> {
    if !(1..=1_000_000).contains(&n) {
        return None;
    }
    let mut outside = 1;
    let mut inside = 1;
    let mut p = 2;
    while p * p <= n {
        let mut count = 0;
        while n % p == 0 {
            count += 1;
            n /= p;
        }
        for _ in 0..count / 2 {
            outside *= p;
        }
        if count % 2 == 1 {
            inside *= p;
        }
        p += 1;
    }
    Some((outside, inside * n))
}
fn sqrt_rational(r: Rational) -> Option<Form> {
    if r.n < 0 {
        return None;
    }
    if r.n == 0 {
        return Form::scalar(0, 1);
    }
    let (outside, inside) = square_factors(r.n.checked_mul(r.d)?)?;
    Some(Form {
        coefficient: Rational::new(outside, r.d)?,
        basis: if inside == 1 {
            Basis::Scalar
        } else {
            Basis::Root(inside)
        },
    })
}
/// Guesses only small, familiar forms; every noninteger guess carries `≈`.
pub fn recognize(value: f64) -> Option<String> {
    if value == 0.0 {
        return Some("0".into());
    }
    if !value.is_finite() {
        return None;
    }
    if value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_992.0 {
        return Some(format!("{value:.0}"));
    }
    if value.abs() < 1e-9 || value.abs() > 1000.0 {
        return None;
    }
    let close = |x: f64| (x - value).abs() <= 1e-11 * value.abs().max(1.0);
    let integer = value.round();
    if close(integer) {
        return Some(format!("≈ {integer:.0}"));
    }
    for d in 2..=16 {
        let n = (value * d as f64).round() as i128;
        if n.abs() <= 100 && n != 0 && close(n as f64 / d as f64) {
            return Some(format!("≈ {}", Form::scalar(n, d)?.text()));
        }
    }
    for basis in [Basis::Pi].into_iter().chain(
        (2..=30)
            .filter(|r| square_factors(*r).is_some_and(|(a, _)| a == 1))
            .map(Basis::Root),
    ) {
        let factor = match basis {
            Basis::Pi => std::f64::consts::PI,
            Basis::Root(r) => (r as f64).sqrt(),
            _ => unreachable!(),
        };
        for d in 1..=12 {
            let n = (value / factor * d as f64).round() as i128;
            if n != 0 && n.abs() <= 12 && close(n as f64 / d as f64 * factor) {
                return Some(format!(
                    "≈ {}",
                    Form {
                        coefficient: Rational::new(n, d)?,
                        basis
                    }
                    .text()
                ));
            }
        }
    }
    None
}
pub fn display(value: f64, prefer_exact: bool) -> String {
    if prefer_exact {
        recognize(value).unwrap_or_else(|| decimal(value))
    } else {
        decimal(value)
    }
}
/// Proves a supported expression without treating nearby floating literals as constants.
pub fn exact_expr(expr: &Expr, env: &Env) -> Option<String> {
    prove(expr, env, 0, &mut 512).map(Form::text)
}
fn literal(n: f64) -> Option<Form> {
    if !n.is_finite() || n.abs() > LIMIT as f64 {
        return None;
    }
    // Only small dyadic literals have recoverable exact provenance from f64 alone.
    for power in 0..=16 {
        let d = 1i128 << power;
        let scaled = n * d as f64;
        if scaled.fract() == 0.0 {
            return Form::scalar(scaled as i128, d);
        }
    }
    None
}
fn prove(e: &Expr, env: &Env, depth: usize, budget: &mut usize) -> Option<Form> {
    if depth > 48 || *budget == 0 {
        return None;
    }
    *budget -= 1;
    let mut sub = |e: &Expr| prove(e, env, depth + 1, budget);
    match e {
        Expr::Number(n) => literal(*n),
        Expr::Var(s) if s == "pi" || s == "tau" => Some(Form {
            coefficient: Rational::new(if s == "pi" { 1 } else { 2 }, 1)?,
            basis: Basis::Pi,
        }),
        Expr::Var(_) => None,
        Expr::Neg(e) => {
            let mut f = sub(e)?;
            f.coefficient.n = -f.coefficient.n;
            Some(f)
        }
        Expr::Binary(op, a, b) => {
            let a = sub(a)?;
            let b = sub(b)?;
            match op {
                '+' | '-' => {
                    let mut b = b;
                    if *op == '-' {
                        b.coefficient.n = -b.coefficient.n;
                    }
                    if a.coefficient.n == 0 {
                        return Some(b);
                    }
                    if b.coefficient.n == 0 {
                        return Some(a);
                    }
                    if a.basis != b.basis {
                        return None;
                    }
                    Some(
                        Form {
                            coefficient: a.coefficient.add(b.coefficient)?,
                            basis: a.basis,
                        }
                        .normalized(),
                    )
                }
                '*' => a.mul(b),
                '/' => {
                    if a.basis == b.basis {
                        return Some(Form {
                            coefficient: a.coefficient.div(b.coefficient)?,
                            basis: Basis::Scalar,
                        });
                    }
                    a.mul(b.inverse()?)
                }
                '^' => {
                    if b.basis != Basis::Scalar
                        || b.coefficient.d != 1
                        || b.coefficient.n.abs() > 16
                    {
                        return None;
                    }
                    let n = b.coefficient.n;
                    let a = if n < 0 { a.inverse()? } else { a };
                    let mut out = Form::scalar(1, 1)?;
                    for _ in 0..n.abs() {
                        out = out.mul(a)?;
                    }
                    Some(out)
                }
                _ => None,
            }
        }
        Expr::Call(name, args) if args.len() == 1 && !env.functions.contains_key(name) => {
            let a = sub(&args[0])?;
            match name.as_str() {
                "sqrt" if a.basis == Basis::Scalar => sqrt_rational(a.coefficient),
                "sin" | "cos" | "tan" => trig(name, a, env.degrees),
                "abs" => Some(Form {
                    coefficient: Rational {
                        n: a.coefficient.n.abs(),
                        ..a.coefficient
                    },
                    ..a
                }),
                _ => None,
            }
        }
        _ => None,
    }
}
fn trig(name: &str, a: Form, degrees: bool) -> Option<Form> {
    let r = if degrees {
        if a.basis != Basis::Scalar {
            return None;
        }
        a.coefficient.div(Rational::new(180, 1)?)?
    } else {
        match a.basis {
            Basis::Pi => a.coefficient,
            Basis::Scalar if a.coefficient.n == 0 => Rational::new(0, 1)?,
            _ => return None,
        }
    };
    let twelfths = r.mul(Rational::new(12, 1)?)?;
    if twelfths.d != 1 {
        return None;
    }
    let sin = |k: i128| -> Option<Form> {
        let k = k.rem_euclid(24);
        let (k, sign) = if k > 12 { (k - 12, -1) } else { (k, 1) };
        let k = if k > 6 { 12 - k } else { k };
        match k {
            0 => Form::scalar(0, 1),
            2 => Form::scalar(sign, 2),
            3 => Some(Form {
                coefficient: Rational::new(sign, 2)?,
                basis: Basis::Root(2),
            }),
            4 => Some(Form {
                coefficient: Rational::new(sign, 2)?,
                basis: Basis::Root(3),
            }),
            6 => Form::scalar(sign, 1),
            _ => None,
        }
    };
    let k = twelfths.n;
    match name {
        "sin" => sin(k),
        "cos" => sin(k + 6),
        "tan" => sin(k)?.mul(sin(k + 6)?.inverse()?),
        _ => None,
    }
}
