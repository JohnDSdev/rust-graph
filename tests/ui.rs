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
