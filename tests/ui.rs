use eframe::egui::{self, Event, Key, Modifiers, RawInput};
use rust_graph::{
    app::GraphApp,
    document::{Document, Row},
    export,
};
fn draw(app: &mut GraphApp, ctx: &egui::Context, events: Vec<Event>) {
    let input = RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 800.0),
        )),
        events,
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| app.ui(ctx));
}
#[test]
fn editing_expression_replots_and_undoes() {
    let mut app = GraphApp::from_document(Document {
        rows: vec![Row::expression("", 0)],
        ..Default::default()
    });
    let ctx = egui::Context::default();
    draw(&mut app, &ctx, vec![]);
    ctx.memory_mut(|m| m.request_focus(egui::Id::new(("expression", 0usize))));
    draw(&mut app, &ctx, vec![Event::Text("y=x^2".into())]);
    assert_eq!(app.document.rows[0].text, "y=x^2");
    assert!(
        !app.document
            .compile()
            .geometry(app.document.viewport, 600, 400)
            .lines
            .is_empty()
    );
    app.undo();
    assert_eq!(app.document.rows[0].text, "");
    app.redo();
    assert_eq!(app.document.rows[0].text, "y=x^2");
}
#[test]
fn enter_adds_expression() {
    let mut app = GraphApp::from_document(Document {
        rows: vec![Row::expression("y=x", 0)],
        ..Default::default()
    });
    let ctx = egui::Context::default();
    draw(&mut app, &ctx, vec![]);
    ctx.memory_mut(|m| m.request_focus(egui::Id::new(("expression", 0usize))));
    draw(
        &mut app,
        &ctx,
        vec![Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        }],
    );
    assert_eq!(app.document.rows.len(), 2);
}
#[test]
fn svg_and_png_are_real_graph_exports() {
    let d = Document {
        rows: vec![Row::expression("y=x^2", 0), Row::expression("(2,3)", 1)],
        ..Default::default()
    };
    let s = d.compile();
    let svg = export::svg(&d, &s, 800, 600);
    assert!(svg.contains("<polyline"));
    assert!(svg.contains("<circle"));
    assert!(svg.contains("viewBox=\"0 0 800 600\""));
    let bytes = export::png(&svg).unwrap();
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
}
#[test]
fn pan_in_canvas_changes_bounds() {
    let mut app = GraphApp::default();
    let ctx = egui::Context::default();
    draw(&mut app, &ctx, vec![]);
    let old = app.document.viewport;
    draw(
        &mut app,
        &ctx,
        vec![
            Event::PointerMoved(egui::pos2(800.0, 400.0)),
            Event::PointerButton {
                pos: egui::pos2(800.0, 400.0),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::default(),
            },
        ],
    );
    draw(
        &mut app,
        &ctx,
        vec![Event::PointerMoved(egui::pos2(850.0, 400.0))],
    );
    assert!(app.document.viewport.xmin < old.xmin);
}

#[test]
fn app_regression_and_generated_tables_are_graphable() {
    use rust_graph::regression::RegressionModel;
    let mut table = Row::expression("", 0);
    table.kind = "table".into();
    table.table = "-2,7\n-1,4\n0,3\n1,4\n2,7".into();
    let mut app = GraphApp::from_document(Document {
        rows: vec![table],
        ..Default::default()
    });
    app.fit_table(0, RegressionModel::Quadratic).unwrap();
    assert_eq!(app.document.rows.len(), 2);
    let scene = app.document.compile();
    assert!(scene.errors.iter().all(Option::is_none));
    app.generate_from_row(1, -2.0, 2.0, 1.0).unwrap();
    assert_eq!(app.document.rows.len(), 3);
    let points = rust_graph::document::table_points(&app.document.rows[2].table).unwrap();
    assert_eq!(points.len(), 5);
    assert!((points[2][1] - 3.0).abs() < 1e-10);
}

#[test]
fn generated_table_at_sample_limit_can_be_saved() {
    let mut app = GraphApp::from_document(Document {
        rows: vec![Row::expression("y=x", 0)],
        ..Default::default()
    });
    app.generate_from_row(0, 0.0, 2047.0, 1.0).unwrap();
    assert!(
        app.document.validate().is_ok(),
        "generated table exceeds file limits"
    );
    assert!(Document::decode(&serde_json::to_string(&app.document).unwrap()).is_ok());
}
