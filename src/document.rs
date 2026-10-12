//! Versioned graph files and compilation of expression rows into a scene.
use crate::{
    graph::{Plot, PlotKind, Scene, Viewport},
    math::{Env, Expr},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};

#[derive(Clone, Serialize, Deserialize)]
pub struct Document {
    pub rows: Vec<Row>,
    pub viewport: Viewport,
    pub degrees: bool,
    pub title: String,
    pub version: u32,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Row {
    pub text: String,
    pub visible: bool,
    pub color: usize,
    pub kind: String,
    pub table: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub animate: bool,
}
impl Row {
    pub fn expression(text: &str, color: usize) -> Self {
        let angular = text.trim_start().starts_with("r=")
            || text.trim_start().starts_with("r =")
            || (text.trim_start().starts_with('(') && text.contains('t'));
        Self {
            text: text.into(),
            visible: true,
            color: color % 6,
            kind: "expression".into(),
            table: String::new(),
            min: if angular { 0.0 } else { -10.0 },
            max: if angular { std::f64::consts::TAU } else { 10.0 },
            step: 0.1,
            animate: false,
        }
    }
}
impl Default for Document {
    fn default() -> Self {
        Self {
            rows: vec![
                Row::expression("y=sin(x)", 0),
                Row::expression("y=0.15x^2-2", 1),
            ],
            viewport: Viewport::default(),
            degrees: false,
            title: "Untitled Graph".into(),
            version: 1,
        }
    }
}

pub fn identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
/// Split a delimiter only outside parentheses and braces.
pub fn split_top(s: &str, delimiter: char) -> Vec<&str> {
    let mut depth = 0i32;
    let mut start = 0;
    let mut out = Vec::new();
    for (i, c) in s.char_indices() {
        if matches!(c, '(' | '{') {
            depth += 1;
        }
        if matches!(c, ')' | '}') {
            depth -= 1;
        }
        if depth == 0 && c == delimiter {
            out.push(&s[start..i]);
            start = i + c.len_utf8();
        }
    }
    out.push(&s[start..]);
    out
}
fn relation(s: &str) -> Option<(&str, &str, &str)> {
    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' | '{' => depth += 1,
            ')' | '}' => depth -= 1,
            _ => {}
        }
        if depth == 0 && matches!(c, '=' | '<' | '>') {
            let end = i + c.len_utf8() + usize::from(s[i + c.len_utf8()..].starts_with('='));
            return Some((s[..i].trim(), &s[i..end], s[end..].trim()));
        }
    }
    None
}
fn restrictions(s: &str) -> Result<(&str, Vec<Expr>), String> {
    let mut depth = 0;
    for (i, c) in s.char_indices() {
        if c == '(' {
            depth += 1;
        }
        if c == ')' {
            depth -= 1;
        }
        if c == '{'
            && depth == 0
            && i > 0
            && s[..i]
                .trim_end()
                .chars()
                .last()
                .is_some_and(|c| !"=+-*/^,:".contains(c))
        {
            let base = s[..i].trim();
            let mut rest = s[i..].trim();
            let mut conditions = Vec::new();
            while !rest.is_empty() {
                if !rest.starts_with('{') {
                    return Err("Expected domain restriction in braces".into());
                }
                let end = rest.find('}').ok_or("Unclosed domain restriction")?;
                for part in split_top(&rest[1..end], ',') {
                    conditions.push(Expr::parse(part)?);
                }
                rest = rest[end + 1..].trim();
            }
            return Ok((base, conditions));
        }
    }
    Ok((s, Vec::new()))
}
fn definition(text: &str) -> Option<(String, Expr)> {
    let (a, op, b) = relation(text)?;
    if op != "="
        || !identifier(a)
        || matches!(a, "x" | "y" | "r" | "pi" | "e" | "tau" | "t" | "theta")
    {
        return None;
    }
    Some((a.into(), Expr::parse(b).ok()?))
}
pub fn parameter(text: &str) -> Option<(String, f64)> {
    let (name, expr) = definition(text)?;
    if !expr.variables().is_empty() {
        return None;
    }
    let value = expr.eval(&Env::default()).ok()?;
    Some((name, value))
}

impl Document {
    pub fn decode(text: &str) -> Result<Self, String> {
        if text.len() > 4 * 1024 * 1024 {
            return Err("Graph file is too large (4 MB maximum)".into());
        }
        let doc: Self =
            serde_json::from_str(text).map_err(|e| format!("Invalid graph file: {e}"))?;
        doc.validate()?;
        Ok(doc)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!("Unsupported graph format version {}", self.version));
        }
        if !self.viewport.valid() {
            return Err("Graph bounds must be finite and increasing".into());
        }
        if self.rows.len() > 128 || self.title.len() > 256 {
            return Err("Graph exceeds document limits".into());
        }
        for row in &self.rows {
            if row.text.len() > 4096
                || row.table.len() > 32768
                || row.color >= 6
                || !matches!(row.kind.as_str(), "expression" | "table" | "note")
            {
                return Err("Invalid expression row".into());
            }
            if !valid_range(row.min, row.max, row.step) {
                return Err("Invalid slider range or step".into());
            }
        }
        Ok(())
    }
    pub fn read(path: &Path) -> Result<Self, String> {
        if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 4 * 1024 * 1024 {
            return Err("Graph file exceeds 4 MB".into());
        }
        Self::decode(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)
    }
    pub fn write(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let data = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("rustgraph.tmp");
        std::fs::write(&tmp, data).map_err(|e| e.to_string())?;
        std::fs::rename(tmp, path).map_err(|e| e.to_string())
    }
    pub fn compile(&self) -> Scene {
        let mut scene = Scene {
            env: Env {
                degrees: self.degrees,
                ..Default::default()
            },
            errors: vec![None; self.rows.len()],
            values: vec![None; self.rows.len()],
            ..Default::default()
        };
        let mut definitions = Vec::new();
        let mut function_rows = Vec::new();
        let mut used = BTreeSet::new();
        for (i, row) in self.rows.iter().enumerate() {
            if row.kind != "expression" {
                continue;
            }
            let text = row.text.trim();
            if let Some((a, "=", b)) = relation(text) {
                if let Some(open) = a.find('(') {
                    let name = a[..open].trim();
                    if identifier(name) && a.ends_with(')') {
                        let params: Vec<String> = a[open + 1..a.len() - 1]
                            .split(',')
                            .map(|s| s.trim().into())
                            .collect();
                        if params.iter().any(|p| !identifier(p))
                            || params.iter().collect::<BTreeSet<_>>().len() != params.len()
                        {
                            scene.errors[i] =
                                Some("Function arguments must be unique variable names".into());
                            continue;
                        }
                        match Expr::parse(b) {
                            Ok(e) => {
                                if used.insert(name.to_string()) {
                                    scene.env.functions.insert(name.into(), (params, e));
                                    function_rows.push((i, name.to_string()));
                                } else {
                                    scene.errors[i] = Some(format!("Duplicate definition: {name}"));
                                }
                            }
                            Err(e) => scene.errors[i] = Some(e),
                        }
                        continue;
                    }
                }
                if let Some((name, expr)) = definition(text) {
                    if used.insert(name.clone()) {
                        definitions.push((i, name, expr));
                    } else {
                        scene.errors[i] = Some("Duplicate definition".into());
                    }
                }
            }
        }
        for _ in 0..=definitions.len() {
            let mut changed = false;
            for (i, name, expr) in &definitions {
                if scene.env.vars.contains_key(name) {
                    continue;
                }
                if let Ok(value) = expr.eval(&scene.env) {
                    scene.env.vars.insert(name.clone(), value);
                    scene.values[*i] = Some(value);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for (i, name, expr) in definitions {
            if !scene.env.vars.contains_key(&name) {
                scene.errors[i] = Some(
                    expr.eval(&scene.env)
                        .err()
                        .unwrap_or("Circular definition".into()),
                );
            }
        }
        for (i, name) in function_rows {
            if let Some((params, body)) = scene.env.functions.get(&name) {
                scene.errors[i] = body
                    .validate(&scene.env, &params.iter().cloned().collect())
                    .err();
            }
        }
        for (i, row) in self.rows.iter().enumerate() {
            if scene.errors[i].is_some()
                || row.kind == "note"
                || row.text.trim().is_empty() && row.kind != "table"
            {
                continue;
            }
            if row.kind == "table" {
                match table_points(&row.table) {
                    Ok(points) => {
                        if row.visible {
                            scene.plots.push(Plot {
                                row: i,
                                color: row.color,
                                kind: PlotKind::Points(points),
                                restrictions: Vec::new(),
                            });
                        }
                    }
                    Err(e) => scene.errors[i] = Some(e),
                }
                continue;
            }
            if definition(row.text.trim()).is_some() {
                continue;
            }
            if relation(row.text.trim()).is_some_and(|(a, op, _)| {
                op == "=" && a.contains('(') && identifier(a.split('(').next().unwrap_or(""))
            }) {
                continue;
            }
            let result = compile_plot(row, i, &scene.env);
            match result {
                Ok((Some(plot), value)) => {
                    scene.values[i] = value;
                    if row.visible {
                        scene.plots.push(plot);
                    }
                }
                Ok((None, value)) => scene.values[i] = value,
                Err(e) => scene.errors[i] = Some(e),
            }
        }
        scene
    }
}

fn compile_plot(row: &Row, i: usize, env: &Env) -> Result<(Option<Plot>, Option<f64>), String> {
    let text = row
        .text
        .trim()
        .replace('θ', "theta")
        .replace('≤', "<=")
        .replace('≥', ">=");
    let (base, conditions) = restrictions(&text)?;
    if !valid_range(row.min, row.max, row.step) {
        return Err("Invalid parameter range".into());
    }
    let locals: BTreeSet<String> = ["x", "y", "t", "theta"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for condition in &conditions {
        condition.validate(env, &locals)?;
    }
    let kind = if base.starts_with('(')
        && base.ends_with(')')
        && split_top(&base[1..base.len() - 1], ',').len() == 2
    {
        let parts = split_top(&base[1..base.len() - 1], ',');
        let a = Expr::parse(parts[0])?;
        let b = Expr::parse(parts[1])?;
        a.validate(env, &locals)?;
        b.validate(env, &locals)?;
        if a.variables().contains("t") || b.variables().contains("t") {
            PlotKind::Parametric(a, b, row.min, row.max)
        } else {
            PlotKind::Points(vec![[a.eval(env)?, b.eval(env)?]])
        }
    } else if let Some((a, op, b)) = relation(base) {
        let lhs = Expr::parse(a)?;
        let rhs = Expr::parse(b)?;
        if op == "=" {
            match a {
                "y" if !rhs.variables().contains("y") => PlotKind::Y(rhs),
                "x" if !rhs.variables().contains("x") => PlotKind::X(rhs),
                "r" => PlotKind::Polar(rhs, row.min, row.max),
                _ => PlotKind::Implicit(Expr::Binary('-', Box::new(lhs), Box::new(rhs))),
            }
        } else {
            PlotKind::Region(Expr::parse(base)?)
        }
    } else {
        let e = Expr::parse(base)?;
        e.validate(env, &locals)?;
        if !e.variables().contains("x") && !e.variables().contains("y") {
            return Ok((None, Some(e.eval(env)?)));
        }
        PlotKind::Y(e)
    };
    // Probe catches undefined names without rejecting legitimate domain holes.
    let mut probe = env.clone();
    for name in ["x", "y", "t", "theta"] {
        probe.vars.insert(name.into(), 0.37);
    }
    let expressions: Vec<&Expr> = match &kind {
        PlotKind::Y(e)
        | PlotKind::X(e)
        | PlotKind::Polar(e, _, _)
        | PlotKind::Implicit(e)
        | PlotKind::Region(e) => {
            vec![e]
        }
        PlotKind::Parametric(a, b, _, _) => vec![a, b],
        PlotKind::Points(_) => Vec::new(),
    };
    for e in expressions.into_iter().chain(conditions.iter()) {
        e.validate(env, &locals)?;
        if let Err(error) = e.eval(&probe)
            && (error.starts_with("Undefined variable")
                || error.starts_with("Unknown function")
                || error.contains("expects")
                || error.contains("limit exceeded"))
        {
            return Err(error);
        }
    }
    Ok((
        Some(Plot {
            row: i,
            color: row.color,
            kind,
            restrictions: conditions,
        }),
        None,
    ))
}

pub fn table_points(text: &str) -> Result<Vec<[f64; 2]>, String> {
    let mut points = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if points.len() >= 2048 {
            return Err("Table is limited to 2048 points".into());
        }
        let cells: Vec<&str> = line
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .collect();
        if cells.len() != 2 {
            return Err(format!("Table line {} needs x, y", i + 1));
        }
        let a = Expr::parse(cells[0])?.eval(&Env::default())?;
        let b = Expr::parse(cells[1])?.eval(&Env::default())?;
        points.push([a, b]);
    }
    Ok(points)
}

pub fn linear_fit(points: &[[f64; 2]]) -> Result<(f64, f64), String> {
    if points.len() < 2 {
        return Err("Add at least two points".into());
    }
    let n = points.len() as f64;
    let mx = points.iter().map(|p| p[0]).sum::<f64>() / n;
    let my = points.iter().map(|p| p[1]).sum::<f64>() / n;
    let denominator = points.iter().map(|p| (p[0] - mx).powi(2)).sum::<f64>();
    if denominator < 1e-20 {
        return Err("A line needs distinct x coordinates".into());
    }
    let slope = points
        .iter()
        .map(|p| (p[0] - mx) * (p[1] - my))
        .sum::<f64>()
        / denominator;
    let intercept = my - slope * mx;
    if slope.is_finite() && intercept.is_finite() {
        Ok((slope, intercept))
    } else {
        Err("Table values are too large to fit".into())
    }
}

pub fn valid_range(min: f64, max: f64, step: f64) -> bool {
    min.is_finite()
        && max.is_finite()
        && step.is_finite()
        && min.abs() <= 1e12
        && max.abs() <= 1e12
        && (1e-9..=2e12).contains(&(max - min))
        && step > 0.0
        && step <= max - min
}
