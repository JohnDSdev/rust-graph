use eframe::egui::{self, epaint::Shape};
use rust_graph::typeset;
fn render(text: &str) -> (egui::Vec2, Vec<egui::epaint::ClippedShape>) {
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "stix".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/STIXTwoMath-Regular.ttf"
        ))),
    );
    fonts
        .families
        .insert(egui::FontFamily::Name("math".into()), vec!["stix".into()]);
    ctx.set_fonts(fonts);
    let mut size = egui::Vec2::ZERO;
    let out = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            size = typeset::show(ui, text, 24.0).rect.size();
        });
    });
    (size, out.shapes)
}
#[test]
fn fractions_have_native_stacked_layout() {
    let (plain, _) = render("1+2");
    let (fraction, shapes) = render("1/2");
    assert!(fraction.y > plain.y * 1.4);
    assert!(
        shapes
            .iter()
            .any(|s| matches!(&s.shape, Shape::LineSegment { .. }))
    );
}
#[test]
fn radicals_have_vector_overbars() {
    let (_, shapes) = render("sqrt(2)");
    assert!(
        shapes
            .iter()
            .filter(|s| matches!(&s.shape, Shape::LineSegment { .. }))
            .count()
            >= 4
    );
}
#[test]
fn powers_raise_smaller_text() {
    let (_, shapes) = render("x^2");
    let texts: Vec<_> = shapes
        .iter()
        .filter_map(|s| match &s.shape {
            Shape::Text(t) => Some(t),
            _ => None,
        })
        .collect();
    let x = texts.iter().find(|t| t.galley.job.text == "𝑥").unwrap();
    let two = texts.iter().find(|t| t.galley.job.text == "2").unwrap();
    assert!(two.galley.size().y < x.galley.size().y);
    assert!(two.pos.y < x.pos.y);
}
#[test]
fn pairs_and_incomplete_input_are_readable() {
    let (pair, shapes) = render("(pi/2,sqrt(2))");
    assert!(pair.x > 0.0 && pair.y > 24.0);
    assert!(
        shapes
            .iter()
            .any(|s| matches!(&s.shape, Shape::LineSegment { .. }))
    );
    let (fallback, shapes) = render("sqrt(");
    assert!(fallback.x > 0.0);
    assert!(shapes.iter().any(|s| matches!(&s.shape, Shape::Text(_))));
}
#[test]
fn piecewise_and_domain_restrictions_have_native_parts() {
    let (plain, _) = render("x+1");
    let (piece, shapes) = render("{x>0:x^2,-x}");
    assert!(piece.y > plain.y * 1.5);
    assert!(
        shapes
            .iter()
            .filter(|s| matches!(&s.shape, Shape::LineSegment { .. }))
            .count()
            >= 6
    );
    let (_, shapes) = render("sqrt(x){x>0}");
    assert!(
        shapes
            .iter()
            .filter(|s| matches!(&s.shape, Shape::LineSegment { .. }))
            .count()
            >= 4
    );
}
#[test]
fn latex_normalization_keeps_native_math() {
    let (_, shapes) = render(r"\frac{\sqrt{2}}{3}");
    assert!(
        shapes
            .iter()
            .filter(|s| matches!(&s.shape, Shape::LineSegment { .. }))
            .count()
            >= 5
    );
    let (_, shapes) = render(r"\sqrt[3]{x}");
    assert!(
        shapes
            .iter()
            .filter(|s| matches!(&s.shape, Shape::LineSegment { .. }))
            .count()
            >= 4
    );
}
#[test]
fn approximate_exact_forms_retain_native_radicals() {
    for input in ["≈ √2", "≈ (√2, 2)", "2√3", "y=√2/2"] {
        let (_, shapes) = render(input);
        assert!(
            shapes
                .iter()
                .filter(|s| matches!(&s.shape, Shape::LineSegment { .. }))
                .count()
                >= 4,
            "{input}"
        );
    }
}
#[test]
fn scientific_decimal_labels_do_not_turn_into_variable_products() {
    let (_, shapes) = render("1e20");
    assert!(
        shapes
            .iter()
            .any(|s| matches!(&s.shape,Shape::Text(t) if t.galley.job.text=="1e20"))
    );
}
#[test]
fn scientific_coordinates_remain_numbers() {
    let (_, shapes) = render("(1e20,2)");
    assert!(
        shapes
            .iter()
            .any(|s| matches!(&s.shape,Shape::Text(t) if t.galley.job.text=="1e20"))
    );
}
#[test]
fn variables_are_italic_and_functions_and_constants_upright() {
    let (_, shapes) = render("h+x+A+e+pi+theta+sin(t)");
    let glyphs: Vec<_> = shapes
        .iter()
        .filter_map(|s| match &s.shape {
            Shape::Text(t) => Some(t.galley.job.text.as_str()),
            _ => None,
        })
        .collect();
    for expected in ["ℎ", "𝑥", "𝐴", "𝑡", "e", "π", "θ", "sin"] {
        assert!(glyphs.contains(&expected), "missing {expected}: {glyphs:?}");
    }
}
#[test]
fn factorial_and_power_preserve_compound_operand_grouping() {
    for input in [
        "(1+2)!",
        "(-2)!",
        "(1<2)!",
        "({x>0:2,3})!",
        "(-x)^2",
        "(x^2)^3",
        "(1<2)^3",
        "({x>0:2,3})^2",
        "(x!)!",
    ] {
        let (_, shapes) = render(input);
        let glyphs: Vec<_> = shapes
            .iter()
            .filter_map(|s| match &s.shape {
                Shape::Text(t) => Some(t.galley.job.text.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            glyphs.contains(&"(") && glyphs.contains(&")"),
            "grouping lost for {input}: {glyphs:?}"
        );
        if input.ends_with('!') {
            assert_eq!(glyphs.last(), Some(&"!"));
            let close = glyphs.iter().rposition(|g| *g == ")").unwrap();
            assert_eq!(
                close + 1,
                glyphs.len() - 1,
                "factorial must follow closing group: {input}"
            );
        }
    }
}
