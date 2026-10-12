use rust_graph::{
    graph::{Plot, PlotKind, Scene},
    math::{Env, Expr},
    regression::{RegressionModel as M, fit, generate_table},
};
fn check(m: M, p: Vec<[f64; 2]>) {
    let f = fit(&p, m).unwrap();
    assert!(f.rmse < 1e-7, "{} {}", f.equation, f.rmse);
    let e = Expr::parse(f.equation.strip_prefix("y=").unwrap()).unwrap();
    for [x, y] in p {
        let mut env = Env::default();
        env.vars.insert("x".into(), x);
        assert!((e.eval(&env).unwrap() - y).abs() < 1e-5);
    }
}
#[test]
fn models() {
    for m in [M::Quadratic, M::Cubic] {
        check(
            m,
            (-8..9)
                .map(|x| {
                    let x = x as f64;
                    [
                        x,
                        2. + 3. * x - x * x + if m == M::Cubic { 0.2 * x * x * x } else { 0. },
                    ]
                })
                .collect(),
        );
    }
    check(
        M::Quadratic,
        (0..20)
            .map(|i| {
                let t = i as f64;
                [1e9 + t, 3. + 2. * t + t * t]
            })
            .collect(),
    );
    for m in [M::Exponential, M::Logarithmic, M::Power] {
        check(
            m,
            (1..20)
                .map(|i| {
                    let x = i as f64 / 3.;
                    [
                        x,
                        match m {
                            M::Exponential => 2. * (0.2 * x).exp(),
                            M::Logarithmic => 2. + 3. * x.ln(),
                            _ => 2. * x.powf(1.3),
                        },
                    ]
                })
                .collect(),
        );
    }
    check(
        M::Sinusoidal,
        (0..100)
            .map(|i| {
                let x = i as f64 / 10.;
                [x, 3. + 2. * (1.7 * x + 0.3).sin()]
            })
            .collect(),
    );
}
#[test]
fn invalid() {
    assert!(fit(&[[1., 2.], [1., 3.]], M::Linear).is_err());
    assert!(fit(&[[0., 1.], [1., 2.]], M::Power).is_err());
    assert!(fit(&[[1., -1.], [2., 2.]], M::Exponential).is_err());
}
fn scene(s: &str) -> Scene {
    Scene {
        plots: vec![Plot {
            row: 0,
            color: 0,
            kind: PlotKind::Y(Expr::parse(s).unwrap()),
            restrictions: vec![],
        }],
        ..Scene::default()
    }
}
#[test]
fn tables() {
    let s = scene("x^2");
    let t = generate_table(&s, 0, -2., 2., 1.).unwrap();
    let p = rust_graph::document::table_points(&t).unwrap();
    assert_eq!(p, vec![[-2., 4.], [-1., 1.], [0., 0.], [1., 1.], [2., 4.]]);
    assert_eq!(
        rust_graph::document::table_points(
            &generate_table(&scene("sqrt(x)"), 0, -2., 2., 1.).unwrap()
        )
        .unwrap()
        .len(),
        3
    );
    assert_eq!(
        rust_graph::document::table_points(&generate_table(&scene("1/x"), 0, -2., 2., 1.).unwrap())
            .unwrap()
            .len(),
        4
    );
    assert!(generate_table(&s, 0, 0., 3000., 1.).is_err());
    assert!(generate_table(&s, 0, 2., 1., 1.).is_err());
    assert!(generate_table(&s, 0, 0., 1., 0.).is_err());
}
#[test]
fn residuals_and_parameters() {
    let p = vec![[-2., -2.9], [-1., -1.1], [0., 1.], [1., 3.1], [2., 4.9]];
    let f = fit(&p, M::Linear).unwrap();
    assert!((f.rmse - (0.0072f64).sqrt()).abs() < 1e-12);
    assert!(f.r_squared > 0.998);
    let mut s = scene("a*x^2");
    s.env.vars.insert("a".into(), 2.);
    s.plots[0].restrictions.push(Expr::parse("x>0").unwrap());
    let p =
        rust_graph::document::table_points(&generate_table(&s, 0, -2., 2., 1.).unwrap()).unwrap();
    assert_eq!(p, vec![[1., 2.], [2., 8.]]);
    assert_eq!(
        rust_graph::document::table_points(&generate_table(&s, 0, 0., 2., 0.75).unwrap()).unwrap(),
        vec![[0.75, 1.125], [1.5, 4.5]]
    );
}
#[test]
fn shifted_transformed_models() {
    check(
        M::Exponential,
        vec![
            [1000., 1.],
            [1001., std::f64::consts::E],
            [1002., std::f64::consts::E.powi(2)],
        ],
    );
    check(M::Power, vec![[1e100, 1.], [2e100, 4.], [3e100, 9.]]);
}
