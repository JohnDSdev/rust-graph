use rust_graph::{
    exact::{decimal, display, exact_expr, recognize},
    math::{Env, Expr},
};
fn proof(s: &str) -> Option<String> {
    exact_expr(&Expr::parse(s).unwrap(), &Env::default())
}
#[test]
fn decimal_precision_and_extremes() {
    assert_eq!(decimal(1.23456789123456), "1.23456789123");
    assert_eq!(decimal(0.000000000000123456789123), "1.23456789123e-13");
    assert_eq!(decimal(1e20), "1e20");
    assert_ne!(decimal(f64::from_bits(1)), "0");
    assert_eq!(decimal(-0.0), "0");
}
#[test]
fn numerical_recognition_is_honest_and_simple() {
    for (v, s) in [
        (2f64.sqrt(), "≈ √2"),
        (std::f64::consts::PI / 2.0, "≈ π/2"),
        (2.0 * std::f64::consts::PI / 3.0, "≈ 2π/3"),
        (12f64.sqrt(), "≈ 2√3"),
        (1.0 / 3.0, "≈ 1/3"),
        (-2f64.sqrt(), "≈ -√2"),
    ] {
        assert_eq!(recognize(v).as_deref(), Some(s));
    }
    assert_eq!(recognize(0.123456789), None);
    assert_eq!(recognize(1e-14), None);
    assert_eq!(recognize(f64::INFINITY), None);
    assert_eq!(recognize(2.0).as_deref(), Some("2"));
    assert_eq!(display(2f64.sqrt(), false), decimal(2f64.sqrt()));
    assert_eq!(display(2f64.sqrt(), true), "≈ √2");
}
#[test]
fn symbolic_arithmetic_proves_forms() {
    for (s, want) in [
        ("sqrt(2)", "√2"),
        ("sqrt(8)/2", "√2"),
        ("sqrt(12)", "2√3"),
        ("pi/4", "π/4"),
        ("1/3", "1/3"),
        ("(1/3+1/6)^2", "1/4"),
        ("sqrt(2)*sqrt(8)", "4"),
        ("1/sqrt(2)", "√2/2"),
        ("sqrt(1/2)", "√2/2"),
        ("-2*pi/3", "-2π/3"),
        ("sin(pi/6)", "1/2"),
        ("cos(pi/4)", "√2/2"),
        ("tan(pi/3)", "√3"),
    ] {
        assert_eq!(proof(s).as_deref(), Some(want), "{s}");
    }
}
#[test]
fn unsupported_and_limits_never_claim_proof() {
    for s in [
        "1.4142135623730951",
        "3.141592653589793",
        "sqrt(-1)",
        "1/0",
        "2^10000",
        "sin(1)",
        "pi+1",
        "sqrt(1000000000001)",
    ] {
        assert_eq!(proof(s), None, "{s}");
    }
    let mut env = Env::default();
    env.vars.insert("a".into(), 2f64.sqrt());
    assert_eq!(exact_expr(&Expr::parse("a").unwrap(), &env), None);
    env.functions
        .insert("sqrt".into(), (vec!["x".into()], Expr::Number(9.0)));
    assert_eq!(exact_expr(&Expr::parse("sqrt(2)").unwrap(), &env), None);
    let mut expr = Expr::Number(1.0);
    for _ in 0..100 {
        expr = Expr::Neg(Box::new(expr));
    }
    assert_eq!(exact_expr(&expr, &Env::default()), None);
}

#[test]
fn zero_sign_and_bounded_standard_angles() {
    assert_eq!(recognize(-0.0).as_deref(), Some("0"));
    assert_eq!(proof("sin(100*pi+pi/6)").as_deref(), Some("1/2"));
    assert_eq!(proof("tan(pi/2)"), None);
    assert_eq!(proof("sqrt(2)+sqrt(2)").as_deref(), Some("2√2"));
    assert_eq!(proof("2^-3").as_deref(), Some("1/8"));
    let env = Env {
        degrees: true,
        ..Env::default()
    };
    assert_eq!(
        exact_expr(&Expr::parse("sin(30)").unwrap(), &env).as_deref(),
        Some("1/2")
    );
}
