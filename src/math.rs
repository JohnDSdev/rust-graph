//! Small, bounded mathematical expression parser. No scripting or side effects.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default)]
pub struct Env {
    pub vars: BTreeMap<String, f64>,
    pub functions: BTreeMap<String, (Vec<String>, Expr)>,
    pub degrees: bool,
}

#[derive(Clone, Debug)]
pub enum Expr {
    Number(f64),
    Var(String),
    Neg(Box<Expr>),
    Binary(char, Box<Expr>, Box<Expr>),
    Compare(Vec<Expr>, Vec<String>),
    Call(String, Vec<Expr>),
    Factorial(Box<Expr>),
    Piecewise(Vec<(Option<Expr>, Expr)>),
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    Name(String),
    Op(String),
    End,
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    if input.len() > 4096 {
        return Err("Expression is too long (4096 bytes maximum)".into());
    }
    let normalized = input
        .replace('π', "pi")
        .replace('θ', "theta")
        .replace('τ', "tau")
        .replace(['×', '·'], "*")
        .replace('÷', "/")
        .replace('−', "-")
        .replace('≤', "<=")
        .replace('≥', ">=")
        .replace('≠', "!=")
        .replace('√', "sqrt");
    let chars: Vec<char> = normalized.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || c == '.' {
            let start = i;
            let mut dots = 0;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                if chars[i] == '.' {
                    dots += 1;
                }
                i += 1;
            }
            if dots > 1 {
                return Err("Invalid number".into());
            }
            // Scientific notation is recognized only with an explicit signed exponent.
            if i + 2 < chars.len()
                && matches!(chars[i], 'e' | 'E')
                && matches!(chars[i + 1], '+' | '-')
                && chars[i + 2].is_ascii_digit()
            {
                i += 2;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let s: String = chars[start..i].iter().collect();
            let n = s.parse::<f64>().map_err(|_| "Invalid number")?;
            if !n.is_finite() {
                return Err("Number is out of range".into());
            }
            out.push(Token::Number(n));
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            i += 1;
            while i < chars.len()
                && (chars[i].is_ascii_alphabetic() || chars[i] == '_' || chars[i].is_ascii_digit())
            {
                i += 1;
            }
            out.push(Token::Name(chars[start..i].iter().collect()));
            continue;
        }
        if "+-*/%^!(),{}:<>=&|".contains(c) {
            let mut s = c.to_string();
            i += 1;
            if i < chars.len()
                && (("<>=!".contains(c) && chars[i] == '=')
                    || (matches!(c, '&' | '|') && chars[i] == c))
            {
                s.push(chars[i]);
                i += 1;
            }
            out.push(Token::Op(s));
        } else {
            return Err(format!("Unexpected character '{c}'"));
        }
    }
    if out.len() > 1024 {
        return Err("Expression has too many terms".into());
    }
    out.push(Token::End);
    Ok(out)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    depth: usize,
}
impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }
    fn is(&self, s: &str) -> bool {
        matches!(self.peek(),Token::Op(t) if t==s)
    }
    fn take(&mut self, s: &str) -> bool {
        if self.is(s) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, s: &str) -> Result<(), String> {
        if self.take(s) {
            Ok(())
        } else {
            Err(format!("Expected '{s}'"))
        }
    }
    fn expression(&mut self, min: u8) -> Result<Expr, String> {
        self.depth += 1;
        if self.depth > 64 {
            return Err("Expression is nested too deeply".into());
        }
        let token = self.peek().clone();
        self.pos += 1;
        let mut lhs = match token {
            Token::Number(n) => Expr::Number(n),
            Token::Name(name) => {
                if self.is("(")
                    && !matches!(
                        name.as_str(),
                        "x" | "y" | "t" | "theta" | "pi" | "e" | "tau"
                    )
                {
                    self.pos += 1;
                    let mut args = Vec::new();
                    if !self.is(")") {
                        loop {
                            args.push(self.expression(0)?);
                            if !self.take(",") {
                                break;
                            }
                        }
                    }
                    self.expect(")")?;
                    Expr::Call(name, args)
                } else {
                    Expr::Var(name)
                }
            }
            Token::Op(s) if s == "-" => Expr::Neg(Box::new(self.expression(6)?)),
            Token::Op(s) if s == "+" => self.expression(6)?,
            Token::Op(s) if s == "(" => {
                let e = self.expression(0)?;
                self.expect(")")?;
                e
            }
            Token::Op(s) if s == "{" => {
                let mut cases = Vec::new();
                loop {
                    let e = self.expression(0)?;
                    if self.take(":") {
                        cases.push((Some(e), self.expression(0)?));
                    } else {
                        cases.push((None, e));
                    }
                    if !self.take(",") {
                        break;
                    }
                }
                self.expect("}")?;
                Expr::Piecewise(cases)
            }
            _ => return Err("Expected a number, variable or function".into()),
        };
        loop {
            if self.is("!") && 9 >= min {
                self.pos += 1;
                lhs = Expr::Factorial(Box::new(lhs));
                continue;
            }
            if let Token::Op(op) = self.peek()
                && ["<", ">", "<=", ">=", "=", "==", "!="].contains(&op.as_str())
                && min <= 2
            {
                let mut values = vec![lhs];
                let mut ops = Vec::new();
                loop {
                    let Token::Op(op) = self.peek() else {
                        break;
                    };
                    if !["<", ">", "<=", ">=", "=", "==", "!="].contains(&op.as_str()) {
                        break;
                    }
                    ops.push(op.clone());
                    self.pos += 1;
                    values.push(self.expression(3)?);
                }
                lhs = Expr::Compare(values, ops);
                continue;
            }
            let (op, left, right, consume) = match self.peek() {
                Token::Op(s) => match s.as_str() {
                    "||" => ('|', 0, 1, true),
                    "&&" => ('&', 1, 2, true),
                    "+" => ('+', 3, 4, true),
                    "-" => ('-', 3, 4, true),
                    "*" => ('*', 5, 6, true),
                    "/" => ('/', 5, 6, true),
                    "%" => ('%', 5, 6, true),
                    "^" => ('^', 7, 7, true),
                    "(" => ('*', 5, 6, false),
                    _ => break,
                },
                Token::Name(_) => ('*', 5, 6, false),
                _ => break,
            };
            if left < min {
                break;
            }
            if consume {
                self.pos += 1;
            }
            let rhs = self.expression(right)?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        self.depth -= 1;
        Ok(lhs)
    }
}

impl Expr {
    pub fn parse(input: &str) -> Result<Self, String> {
        let mut p = Parser {
            tokens: tokenize(input)?,
            pos: 0,
            depth: 0,
        };
        let result = p.expression(0)?;
        if *p.peek() != Token::End {
            return Err("Unexpected input after expression".into());
        }
        Ok(result)
    }
    pub fn eval(&self, env: &Env) -> Result<f64, String> {
        let mut budget = 10000;
        self.eval_inner(env, &BTreeMap::new(), 0, &mut budget)
    }
    fn eval_inner(
        &self,
        env: &Env,
        locals: &BTreeMap<String, f64>,
        depth: usize,
        budget: &mut usize,
    ) -> Result<f64, String> {
        if depth > 48 || *budget == 0 {
            return Err("Calculation limit exceeded (check recursive functions)".into());
        }
        *budget -= 1;
        let mut get = |e: &Expr| e.eval_inner(env, locals, depth + 1, budget);
        let value = match self {
            Self::Number(n) => *n,
            Self::Var(s) => match s.as_str() {
                "pi" => std::f64::consts::PI,
                "e" => std::f64::consts::E,
                "tau" => std::f64::consts::TAU,
                _ => *locals
                    .get(s)
                    .or_else(|| env.vars.get(s))
                    .ok_or_else(|| format!("Undefined variable: {s}"))?,
            },
            Self::Neg(e) => -get(e)?,
            Self::Factorial(e) => {
                let n = get(e)?;
                if !(0.0..=170.0).contains(&n) || n.fract() != 0.0 {
                    return Err("Factorial needs an integer from 0 to 170".into());
                }
                (1..=n as u32).fold(1.0, |a, b| a * f64::from(b))
            }
            Self::Binary(op, a, b) => {
                let a = get(a)?;
                if *op == '&' && a == 0.0 {
                    return Ok(0.0);
                }
                if *op == '|' && a != 0.0 {
                    return Ok(1.0);
                }
                let b = get(b)?;
                match op {
                    '+' => a + b,
                    '-' => a - b,
                    '*' => a * b,
                    '/' => a / b,
                    '%' => a % b,
                    '^' => a.powf(b),
                    '&' => f64::from(b != 0.0),
                    '|' => f64::from(b != 0.0),
                    _ => unreachable!(),
                }
            }
            Self::Compare(values, ops) => {
                let mut a = get(&values[0])?;
                let mut valid = true;
                for (b, op) in values[1..].iter().zip(ops) {
                    let b = get(b)?;
                    valid &= match op.as_str() {
                        "<" => a < b,
                        ">" => a > b,
                        "<=" => a <= b,
                        ">=" => a >= b,
                        "=" | "==" => a == b,
                        "!=" => a != b,
                        _ => false,
                    };
                    a = b;
                }
                f64::from(valid)
            }
            Self::Piecewise(cases) => {
                let mut result = None;
                for (condition, value) in cases {
                    if condition
                        .as_ref()
                        .map(&mut get)
                        .transpose()?
                        .is_none_or(|v| v != 0.0)
                    {
                        result = Some(get(value)?);
                        break;
                    }
                }
                result.ok_or("Outside piecewise domain")?
            }
            Self::Call(name, args) => {
                let values = args.iter().map(&mut get).collect::<Result<Vec<_>, _>>()?;
                if let Some((params, body)) = env.functions.get(name) {
                    if params.len() != values.len() {
                        return Err(format!("{name} expects {} arguments", params.len()));
                    }
                    let mut child = BTreeMap::new();
                    for (p, v) in params.iter().zip(values) {
                        child.insert(p.clone(), v);
                    }
                    body.eval_inner(env, &child, depth + 1, budget)?
                } else {
                    builtin(name, &values, env.degrees)?
                }
            }
        };
        if value.is_finite() {
            Ok(value)
        } else {
            Err("Undefined value (division by zero or outside function domain)".into())
        }
    }
    /// Validate every branch and function dependency before sampling a graph.
    pub fn validate(&self, env: &Env, locals: &BTreeSet<String>) -> Result<(), String> {
        self.validate_inner(env, locals, &mut Vec::new(), &mut BTreeSet::new())
    }
    fn validate_inner(
        &self,
        env: &Env,
        locals: &BTreeSet<String>,
        stack: &mut Vec<String>,
        checked: &mut BTreeSet<String>,
    ) -> Result<(), String> {
        if stack.len() > 48 {
            return Err("Function dependency limit exceeded".into());
        }
        match self {
            Self::Number(_) => {}
            Self::Var(name) => {
                if !locals.contains(name)
                    && !env.vars.contains_key(name)
                    && !matches!(name.as_str(), "pi" | "e" | "tau")
                {
                    return Err(format!("Undefined variable: {name}"));
                }
            }
            Self::Neg(e) | Self::Factorial(e) => e.validate_inner(env, locals, stack, checked)?,
            Self::Binary(_, a, b) => {
                a.validate_inner(env, locals, stack, checked)?;
                b.validate_inner(env, locals, stack, checked)?;
            }
            Self::Compare(values, _) => {
                for e in values {
                    e.validate_inner(env, locals, stack, checked)?;
                }
            }
            Self::Piecewise(cases) => {
                for (condition, e) in cases {
                    if let Some(c) = condition {
                        c.validate_inner(env, locals, stack, checked)?;
                    }
                    e.validate_inner(env, locals, stack, checked)?;
                }
            }
            Self::Call(name, args) => {
                for e in args {
                    e.validate_inner(env, locals, stack, checked)?;
                }
                if let Some((params, body)) = env.functions.get(name) {
                    if params.len() != args.len() {
                        return Err(format!("{name} expects {} arguments", params.len()));
                    }
                    if stack.contains(name) {
                        return Err(format!("Circular function definition: {name}"));
                    }
                    if checked.contains(name) {
                        return Ok(());
                    }
                    stack.push(name.clone());
                    let result =
                        body.validate_inner(env, &params.iter().cloned().collect(), stack, checked);
                    stack.pop();
                    result?;
                    checked.insert(name.clone());
                } else {
                    let valid = match name.as_str() {
                        "min" | "max" => !args.is_empty(),
                        "atan2" | "mod" | "pow" | "root" => args.len() == 2,
                        "log" => matches!(args.len(), 1 | 2),
                        "sin" | "cos" | "tan" | "sec" | "csc" | "cot" | "asin" | "acos"
                        | "atan" | "arcsin" | "arccos" | "arctan" | "sinh" | "cosh" | "tanh"
                        | "sqrt" | "cbrt" | "abs" | "exp" | "ln" | "log10" | "log2" | "floor"
                        | "ceil" | "round" | "sign" | "signum" => args.len() == 1,
                        _ => return Err(format!("Unknown function: {name}")),
                    };
                    if !valid {
                        return Err(format!("{name}: incorrect number of arguments"));
                    }
                }
            }
        }
        Ok(())
    }
    pub fn variables(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        self.visit_vars(&mut out);
        out
    }
    fn visit_vars(&self, out: &mut BTreeSet<String>) {
        match self {
            Self::Var(v) => {
                out.insert(v.clone());
            }
            Self::Neg(e) | Self::Factorial(e) => e.visit_vars(out),
            Self::Binary(_, a, b) => {
                a.visit_vars(out);
                b.visit_vars(out);
            }
            Self::Compare(v, _) | Self::Call(_, v) => {
                for e in v {
                    e.visit_vars(out);
                }
            }
            Self::Piecewise(v) => {
                for (c, e) in v {
                    if let Some(c) = c {
                        c.visit_vars(out);
                    }
                    e.visit_vars(out);
                }
            }
            Self::Number(_) => {}
        }
    }
}

fn builtin(name: &str, a: &[f64], degrees: bool) -> Result<f64, String> {
    if matches!(name, "min" | "max") {
        if a.is_empty() {
            return Err(format!("{name} needs arguments"));
        }
        return Ok(a
            .iter()
            .copied()
            .reduce(if name == "min" { f64::min } else { f64::max })
            .unwrap());
    }
    if matches!(name, "atan2" | "mod" | "pow" | "root" | "log") && a.len() == 2 {
        return Ok(match name {
            "atan2" => {
                let v = a[0].atan2(a[1]);
                if degrees { v.to_degrees() } else { v }
            }
            "mod" => a[0].rem_euclid(a[1]),
            "pow" => a[0].powf(a[1]),
            "root" => {
                if a[0] < 0.0 && a[1].fract() == 0.0 && (a[1] as i64) % 2 != 0 {
                    -(-a[0]).powf(1.0 / a[1])
                } else {
                    a[0].powf(1.0 / a[1])
                }
            }
            _ => a[0].log(a[1]),
        });
    }
    if a.len() != 1 {
        return Err(format!("{name} expects one argument"));
    }
    let x = a[0];
    let angle = if degrees { x.to_radians() } else { x };
    let inverse = |v: f64| if degrees { v.to_degrees() } else { v };
    let value = match name {
        "sin" => angle.sin(),
        "cos" => angle.cos(),
        "tan" => {
            if angle.cos().abs() < 1e-14 {
                f64::NAN
            } else {
                angle.tan()
            }
        }
        "sec" => 1.0 / angle.cos(),
        "csc" => 1.0 / angle.sin(),
        "cot" => 1.0 / angle.tan(),
        "asin" | "arcsin" => inverse(x.asin()),
        "acos" | "arccos" => inverse(x.acos()),
        "atan" | "arctan" => inverse(x.atan()),
        "sinh" => x.sinh(),
        "cosh" => x.cosh(),
        "tanh" => x.tanh(),
        "sqrt" => x.sqrt(),
        "cbrt" => x.cbrt(),
        "abs" => x.abs(),
        "exp" => x.exp(),
        "ln" => x.ln(),
        "log" | "log10" => x.log10(),
        "log2" => x.log2(),
        "floor" => x.floor(),
        "ceil" => x.ceil(),
        "round" => x.round(),
        "sign" | "signum" => x.signum(),
        _ => return Err(format!("Unknown function: {name}")),
    };
    Ok(value)
}
