use rust_graph::{
    document::{Document, Row},
    graph::Viewport,
};
fn doc(rows: &[&str]) -> Document {
    Document {
        rows: rows
            .iter()
            .enumerate()
            .map(|(i, s)| Row::expression(s, i))
            .collect(),
        ..Default::default()
    }
}
#[test]
fn explicit_curve() {
    let g = doc(&["y=x^2"])
        .compile()
        .geometry(Viewport::default(), 600, 400);
    assert!(!g.lines.is_empty());
    assert!(
        g.lines
            .iter()
            .flat_map(|(_, p)| p)
            .any(|p| p[0].abs() < 0.05 && p[1].abs() < 0.01)
    );
}
#[test]
fn implicit_circle() {
    let g = doc(&["x^2+y^2=4"])
        .compile()
        .geometry(Viewport::default(), 600, 400);
    assert!(g.lines.len() > 10);
    for (_, points) in g.lines {
        for [x, y] in points {
            assert!((x * x + y * y - 4.0).abs() < 0.08);
        }
    }
}
#[test]
fn poles_do_not_connect() {
    let g = doc(&["y=1/x"])
        .compile()
        .geometry(Viewport::default(), 600, 400);
    assert!(g.lines.len() >= 2);
    for (_, p) in g.lines {
        for pair in p.windows(2) {
            assert!(pair[0][0] * pair[1][0] >= 0.0);
        }
    }
}
#[test]
fn restrictions() {
    let g = doc(&["y=x {-2<x<2}"])
        .compile()
        .geometry(Viewport::default(), 600, 400);
    assert!(!g.lines.is_empty());
    assert!(
        g.lines
            .iter()
            .flat_map(|(_, p)| p)
            .all(|p| p[0] > -2.0 && p[0] < 2.0)
    );
}
#[test]
fn inequality_fill() {
    let g = doc(&["y<x"])
        .compile()
        .geometry(Viewport::default(), 600, 400);
    assert!(!g.fills.is_empty());
    assert!(g.fills.iter().all(|(_, r)| r[1] < r[0] + 0.5));
}
#[test]
fn parameters_and_forward_references() {
    let s = doc(&["y=a*x", "a=b+1", "b=2"]).compile();
    assert!(s.errors.iter().all(Option::is_none));
    let g = s.geometry(Viewport::default(), 600, 400);
    assert!(!g.lines.is_empty());
    assert!(
        g.lines
            .iter()
            .flat_map(|(_, p)| p)
            .all(|p| (p[1] - 3.0 * p[0]).abs() < 1e-9)
    );
}
#[test]
fn cyclic_parameters_report_errors() {
    let s = doc(&["a=b+1", "b=a+1", "y=a*x"]).compile();
    assert!(s.errors.iter().any(Option::is_some));
}
#[test]
fn named_functions() {
    let s = doc(&["f(u)=u^2", "y=f(x)"]).compile();
    assert!(s.errors.iter().all(Option::is_none));
    assert!(!s.geometry(Viewport::default(), 600, 400).lines.is_empty());
}
#[test]
fn polar_parametric_and_points() {
    let g = doc(&["r=2", "(cos(t),sin(t))", "(2,3)"])
        .compile()
        .geometry(Viewport::default(), 600, 400);
    assert!(g.lines.len() >= 2);
    assert_eq!(g.points, vec![(2, [2.0, 3.0])]);
}
#[test]
fn table_points() {
    let mut d = doc(&[]);
    let mut r = Row::expression("", 0);
    r.kind = "table".into();
    r.table = "1,2\n2,4\n3,6".into();
    d.rows.push(r);
    let g = d.compile().geometry(Viewport::default(), 600, 400);
    assert_eq!(g.points.len(), 3);
}
#[test]
fn transform_and_zoom_anchor() {
    let mut v = Viewport::default();
    assert_eq!(v.to_screen([0.0, 0.0], 600.0, 400.0), [300.0, 200.0]);
    let anchor = v.to_world([120.0, 100.0], 600.0, 400.0);
    v.zoom(0.5, anchor);
    let p = v.to_screen(anchor, 600.0, 400.0);
    assert!((p[0] - 120.0).abs() < 1e-9 && (p[1] - 100.0).abs() < 1e-9);
    assert!(v.valid());
}
#[test]
fn file_validation() {
    let d = doc(&["y=sin(x)"]);
    let text = serde_json::to_string(&d).unwrap();
    assert_eq!(Document::decode(&text).unwrap().rows.len(), 1);
    assert!(Document::decode("{}").is_err());
    let mut bad = d.clone();
    bad.version = 99;
    assert!(Document::decode(&serde_json::to_string(&bad).unwrap()).is_err());
    bad.version = 1;
    bad.viewport.xmax = bad.viewport.xmin;
    assert!(Document::decode(&serde_json::to_string(&bad).unwrap()).is_err());
}

#[test]
fn small_residue_poles_do_not_connect() {
    for formula in [
        "y=0.001/(x-0.008)",
        "y=0.00001/(x-0.008)",
        "y=1e-10/(x-0.008)",
        "y=1+0.00001/(x-0.008)",
    ] {
        let g = doc(&[formula])
            .compile()
            .geometry(Viewport::default(), 600, 400);
        assert!(!g.lines.is_empty());
        for (_, points) in g.lines {
            for p in points.windows(2) {
                assert!(
                    (p[0][0] - 0.008) * (p[1][0] - 0.008) >= 0.0,
                    "{formula} connected across a pole: {p:?}"
                );
            }
        }
    }
}

#[test]
fn far_offscreen_points_are_culled() {
    let g = doc(&["(1e+300,0)", "(2,3)"])
        .compile()
        .geometry(Viewport::default(), 600, 400);
    assert_eq!(g.points, vec![(1, [2.0, 3.0])]);
}

#[test]
fn invalid_function_definitions_have_row_errors() {
    for rows in [
        &["f(x)=missing(x)"][..],
        &["f(x)=f(x)"][..],
        &["f(x)=g(x)", "g(x)=f(x)"][..],
    ] {
        let s = doc(rows).compile();
        assert!(
            s.errors.iter().all(Option::is_some),
            "missing errors for {rows:?}"
        );
    }
}
#[test]
fn undefined_names_in_inactive_branches_are_reported() {
    let s = doc(&["y={x<0:missing,x}"]).compile();
    assert!(s.errors[0].is_some());
}

#[test]
fn chained_inequalities_shade_only_the_strip() {
    let g = doc(&["-1<x<1"])
        .compile()
        .geometry(Viewport::default(), 600, 400);
    assert!(!g.fills.is_empty());
    assert!(g.fills.iter().all(|(_, r)| r[0] >= -1.2 && r[0] <= 1.0));
}
#[test]
fn overflowing_parameter_ranges_are_rejected() {
    let mut d = doc(&["(t,t)"]);
    d.rows[0].min = -1e308;
    d.rows[0].max = 1e308;
    assert!(d.validate().is_err());
}

#[test]
fn polar_uses_the_displayed_parameter_range() {
    let mut d = doc(&["r=theta"]);
    d.rows[0].min = -10.0;
    d.rows[0].max = 10.0;
    let g = d.compile().geometry(Viewport::default(), 600, 400);
    assert!(
        g.lines
            .iter()
            .flat_map(|(_, p)| p)
            .any(|p| p[0].hypot(p[1]) > 9.9)
    );
}

#[test]
fn latex_equations_compile_as_graphs() {
    let s = doc(&[r"a=\frac{1}{2}", r"y=a\sqrt{x}"]).compile();
    assert!(s.errors.iter().all(Option::is_none), "{:?}", s.errors);
    assert!(!s.geometry(Viewport::default(), 600, 400).lines.is_empty());
}
#[test]
fn old_files_get_math_display_defaults_and_settings_round_trip() {
    let mut value = serde_json::to_value(doc(&["y=x"])).unwrap();
    value.as_object_mut().unwrap().remove("pretty_math");
    value.as_object_mut().unwrap().remove("show_features");
    value.as_object_mut().unwrap().remove("exact_values");
    let restored = Document::decode(&value.to_string()).unwrap();
    let defaults = serde_json::to_value(restored).unwrap();
    assert_eq!(defaults["pretty_math"], true);
    assert_eq!(defaults["show_features"], true);
    value["exact_values"] = serde_json::Value::Bool(true);
    let restored = Document::decode(&value.to_string()).unwrap();
    assert_eq!(
        serde_json::to_value(restored).unwrap()["exact_values"],
        true
    );
}
