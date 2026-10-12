use rust_graph::math::{Env, Expr};
fn eval(s: &str) -> f64 {
    Expr::parse(s).unwrap().eval(&Env::default()).unwrap()
}
#[test]
fn precedence_and_powers() {
    assert_eq!(eval("-2^2 + 3*4"), 8.0);
    assert_eq!(eval("2^3^2"), 512.0);
}
#[test]
fn implicit_multiplication() {
    let mut e = Env::default();
    e.vars.insert("x".into(), 3.0);
    assert_eq!(Expr::parse("2x(x+1)").unwrap().eval(&e).unwrap(), 24.0);
    assert_eq!(eval("2(3+1)"), 8.0);
}
#[test]
fn scientific_functions() {
    assert!((eval("sin(pi/2)+ln(e)+sqrt(9)+abs(-2)") - 7.0).abs() < 1e-10);
    assert_eq!(eval("5!"), 120.0);
    assert_eq!(eval("max(2,3,1)"), 3.0);
}
#[test]
fn degree_mode() {
    let e = Env {
        degrees: true,
        ..Default::default()
    };
    assert!((Expr::parse("sin(30)").unwrap().eval(&e).unwrap() - 0.5).abs() < 1e-10);
}
#[test]
fn restrictions_and_piecewise() {
    assert_eq!(eval("{1<0:7,2<3:9,4}"), 9.0);
    assert!(
        Expr::parse("sqrt(-1)")
            .unwrap()
            .eval(&Env::default())
            .is_err()
    );
}
#[test]
fn comparisons() {
    assert_eq!(eval("-1<x<2".replace('x', "0").as_str()), 1.0);
    assert_eq!(eval("2>=3"), 0.0);
}
#[test]
fn malformed_inputs_are_rejected() {
    for s in ["", "1+", "sin(", "1..2", "x;2", "(1,2)", "2**3"] {
        assert!(Expr::parse(s).is_err(), "{s}");
    }
}
#[test]
fn user_functions_and_recursion_guard() {
    let mut e = Env::default();
    e.functions.insert(
        "f".into(),
        (vec!["u".into()], Expr::parse("u^2+1").unwrap()),
    );
    assert_eq!(Expr::parse("f(3)").unwrap().eval(&e).unwrap(), 10.0);
    e.functions
        .insert("f".into(), (vec!["u".into()], Expr::parse("f(u)").unwrap()));
    assert!(Expr::parse("f(3)").unwrap().eval(&e).is_err());
}
#[test]
fn singularities_are_errors() {
    for s in ["1/0", "tan(pi/2)", "log(-1)", "(-1)^0.5"] {
        assert!(
            Expr::parse(s).unwrap().eval(&Env::default()).is_err(),
            "{s}"
        );
    }
}

#[test]
fn functions_use_definition_scope() {
    let mut e = Env::default();
    e.vars.insert("a".into(), 10.0);
    e.functions
        .insert("f".into(), (vec!["x".into()], Expr::parse("a+x").unwrap()));
    e.functions
        .insert("g".into(), (vec!["a".into()], Expr::parse("f(1)").unwrap()));
    assert_eq!(Expr::parse("g(2)").unwrap().eval(&e).unwrap(), 11.0);
}

#[test]
fn expanding_function_dependencies_validate_without_expansion() {
    use std::collections::BTreeSet;
    let mut e = Env::default();
    e.functions
        .insert("f0".into(), (vec!["u".into()], Expr::parse("u").unwrap()));
    for n in 1..=32 {
        let prev = n - 1;
        e.functions.insert(
            format!("f{n}"),
            (
                vec!["u".into()],
                Expr::parse(&format!("f{prev}(u)+f{prev}(u)")).unwrap(),
            ),
        );
    }
    let expr = Expr::parse("f32(1)").unwrap();
    assert!(expr.validate(&e, &BTreeSet::new()).is_ok());
    assert!(expr.eval(&e).is_err());
}

#[test]
fn latex_radicals_fractions_and_exponents() {
    assert!((eval(r"\frac{\sqrt{8}}{2}") - 2.0_f64.sqrt()).abs() < 1e-12);
    assert_eq!(eval(r"2^{3}+\sqrt[3]{27}"), 11.0);
    assert!((eval(r"\sin\left(\frac{\pi}{2}\right)") - 1.0).abs() < 1e-12);
    assert_eq!(eval("√(9)+π-π"), 3.0);
}
#[test]
fn invalid_latex_is_a_visible_parse_error() {
    for s in [r"\frac{1}", r"\sqrt{", r"\unknown{2}", r"x^{2"] {
        assert!(Expr::parse(s).is_err(), "{s}");
    }
}

#[test]
fn scientific_notation_accepts_unsigned_exponents() {
    assert_eq!(eval("1e3"), 1000.0);
    assert_eq!(eval("1E-3"), 0.001);
    assert!((eval("2e") - 2.0 * std::f64::consts::E).abs() < 1e-12);
}
