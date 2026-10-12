use rust_graph::{
    analysis::{FeatureKind, features},
    document::{Document, Row},
    graph::Viewport,
};
fn check(rows: &[&str], kind: FeatureKind, p: [f64; 2]) {
    let d = Document {
        rows: rows
            .iter()
            .enumerate()
            .map(|(i, s)| Row::expression(s, i))
            .collect(),
        ..Document::default()
    };
    let s = d.compile();
    let v = Viewport::default();
    let f = features(&s, v, &s.geometry(v, 600, 400));
    assert!(
        f.iter().any(|q| q.kind == kind
            && (q.position[0] - p[0]).abs() < 1e-7
            && (q.position[1] - p[1]).abs() < 1e-7),
        "missing {p:?}: {f:?}"
    );
}
#[test]
fn refined_features() {
    check(&["y=x^2"], FeatureKind::Minimum, [0., 0.]);
    check(&["y=(x-2)^2-3"], FeatureKind::Minimum, [2., -3.]);
    check(
        &["y=sin(x)"],
        FeatureKind::Maximum,
        [std::f64::consts::FRAC_PI_2, 1.],
    );
    for x in [-2f64.sqrt(), 2f64.sqrt()] {
        check(&["y=x^2", "y=2"], FeatureKind::Intersection, [x, 2.]);
        check(&["x^2+y^2=4", "y=x"], FeatureKind::Intersection, [x, x]);
    }
    check(&["y=x^2", "y=0"], FeatureKind::Intersection, [0., 0.]);
    check(
        &["y=sin(x)"],
        FeatureKind::XIntercept,
        [std::f64::consts::PI, 0.],
    );
}
#[test]
fn poles_and_domains() {
    for rows in [vec!["y=1/x"], vec!["y=1/x", "y=0"], vec!["y=x^2 {x>1}"]] {
        let d = Document {
            rows: rows
                .iter()
                .enumerate()
                .map(|(i, s)| Row::expression(s, i))
                .collect(),
            ..Document::default()
        };
        let s = d.compile();
        let f = features(&s, d.viewport, &Default::default());
        assert!(f.iter().all(|p| p.position.iter().all(|n| n.is_finite())));
        assert!(!f.iter().any(|p| matches!(
            p.kind,
            FeatureKind::XIntercept | FeatureKind::Intersection | FeatureKind::Minimum
        ) && p.rows.contains(&0)
            && p.position[0].abs() < 1e-7
            && p.position[1].abs() < 1e-7));
    }
}
#[test]
fn circle_apex_and_bounded_many_plots() {
    check(&["x^2+y^2=4"], FeatureKind::Maximum, [0., 2.]);
    check(&["x^2+y^2=4"], FeatureKind::Minimum, [0., -2.]);
    let d = Document {
        rows: (0..128)
            .map(|i| Row::expression(&format!("y=sin(x)+{}", i as f64 / 100.), i))
            .collect(),
        ..Document::default()
    };
    let s = d.compile();
    let f = features(&s, d.viewport, &Default::default());
    assert!(f.len() <= 512);
}
#[test]
fn hidden_and_discontinuous() {
    let mut d = Document {
        rows: vec![
            Row::expression("y=x^2", 0),
            Row::expression("y=1/(x-0.123)", 1),
            Row::expression("y=0", 2),
        ],
        ..Document::default()
    };
    d.rows[0].visible = false;
    let s = d.compile();
    let f = features(&s, d.viewport, &Default::default());
    assert!(f.iter().all(|q| !q.rows.contains(&0)));
    assert!(!f.iter().any(|q| q.kind == FeatureKind::Intersection));
}
#[test]
fn parametric_intersections_and_precision() {
    check(
        &["(2cos(t),sin(t))", "y=0"],
        FeatureKind::Intersection,
        [2., 0.],
    );
    check(&["(t,t)", "(t,-t)"], FeatureKind::Intersection, [0., 0.]);
    check(&["r=2", "y=0"], FeatureKind::Intersection, [2., 0.]);
    let d = Document {
        rows: vec![Row::expression("x^2+y^2=4", 0), Row::expression("x-y=0", 1)],
        ..Document::default()
    };
    let s = d.compile();
    let f = features(&s, d.viewport, &Default::default());
    assert!(f.iter().any(|q| q.kind == FeatureKind::Intersection
        && (q.position[0] - 2f64.sqrt()).abs() < 1e-10
        && (q.position[1] - 2f64.sqrt()).abs() < 1e-10));
    check(&["(1,1)", "y=x"], FeatureKind::Intersection, [1., 1.]);
}
#[test]
fn tiny_pole_residual_is_not_a_root() {
    let d = Document {
        rows: vec![
            Row::expression("y=0.000000000001/(x-0.123)", 0),
            Row::expression("y=0", 1),
        ],
        ..Document::default()
    };
    let s = d.compile();
    let f = features(&s, d.viewport, &Default::default());
    assert!(!f.iter().any(|q| q.kind == FeatureKind::Intersection
        || q.kind == FeatureKind::XIntercept && q.rows == [0]));
}
#[test]
fn implicit_solver_is_scale_invariant_and_rejects_coincidence() {
    for rows in [
        vec!["1e-15*(x^2+y^2-1)=0", "1e-15*((x-4)^2+y^2-1)=0"],
        vec!["x^2+y^2=4", "x^2+y^2=4"],
    ] {
        let d = Document {
            rows: rows
                .iter()
                .enumerate()
                .map(|(i, r)| Row::expression(r, i))
                .collect(),
            ..Document::default()
        };
        let s = d.compile();
        assert!(s.errors.iter().all(Option::is_none), "{:?}", s.errors);
        let f = features(&s, d.viewport, &Default::default());
        assert!(
            !f.iter().any(|p| p.kind == FeatureKind::Intersection),
            "{f:?}"
        );
    }
    for scale in ["1e-15", "1", "1e15"] {
        let d = Document {
            rows: vec![
                Row::expression(&format!("{scale}*(x^2+y^2-4)=0"), 0),
                Row::expression(&format!("{scale}*(x-y)=0"), 1),
            ],
            ..Document::default()
        };
        let s = d.compile();
        let f = features(&s, d.viewport, &Default::default());
        for x in [-2f64.sqrt(), 2f64.sqrt()] {
            assert!(
                f.iter().any(|p| p.kind == FeatureKind::Intersection
                    && (p.position[0] - x).abs() < 1e-10
                    && (p.position[1] - x).abs() < 1e-10),
                "{scale}: {f:?}"
            );
        }
    }
}
