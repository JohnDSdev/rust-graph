//! Native desktop interaction and local file workflows.
use crate::{
    analysis::{self, FeatureKind, FeaturePoint},
    document::{Document, Row, parameter, table_points, valid_range},
    exact, export,
    graph::{COLORS, Geometry, Scene, Viewport, format_number, grid_step},
    math::Expr,
    regression::{self, RegressionModel},
    typeset,
};
use eframe::egui::{
    self, Align, Color32, FontId, Id, Key, Layout, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const INK: Color32 = Color32::from_rgb(38, 47, 52);
const GREEN: Color32 = Color32::from_rgb(31, 103, 83);
fn color(i: usize) -> Color32 {
    let [r, g, b] = COLORS[i % 6];
    Color32::from_rgb(r, g, b)
}

pub struct GraphApp {
    pub document: Document,
    pub scene: Scene,
    pub history: Vec<Document>,
    pub future: Vec<Document>,
    pub path: Option<PathBuf>,
    pub status: String,
    geometry: Geometry,
    cache_view: Option<(Viewport, usize, usize)>,
    settings: bool,
    help: bool,
    keypad: bool,
    trace: bool,
    focused: usize,
    focus_next: Option<usize>,
    skip_history: bool,
    saved: String,
    autosave: Option<PathBuf>,
    last_autosave: Instant,
    last_tick: Instant,
    styled: bool,
    feature_points: Vec<FeaturePoint>,
    feature_view: Option<(Viewport, usize, usize)>,
    feature_changed: Instant,
    feature_updated: Instant,
    point_inspector: bool,
    selected_point: Option<FeaturePoint>,
    table_dialog: Option<usize>,
    table_start: f64,
    table_end: f64,
    table_step: f64,
}
impl Default for GraphApp {
    fn default() -> Self {
        let document = Document::default();
        let scene = document.compile();
        Self {
            saved: serde_json::to_string(&document).unwrap_or_default(),
            document,
            scene,
            history: Vec::new(),
            future: Vec::new(),
            path: None,
            status: "Ready · all calculations stay on your computer".into(),
            geometry: Geometry::default(),
            cache_view: None,
            settings: false,
            help: false,
            keypad: false,
            trace: true,
            focused: 0,
            focus_next: None,
            skip_history: false,
            autosave: None,
            last_autosave: Instant::now(),
            last_tick: Instant::now(),
            styled: false,
            feature_points: Vec::new(),
            feature_view: None,
            feature_changed: Instant::now(),
            feature_updated: Instant::now(),
            point_inspector: false,
            selected_point: None,
            table_dialog: None,
            table_start: -5.0,
            table_end: 5.0,
            table_step: 1.0,
        }
    }
}
impl GraphApp {
    pub fn fit_table(&mut self, index: usize, model: RegressionModel) -> Result<(), String> {
        if self.document.rows.len() >= 128 {
            return Err("A graph can contain at most 128 rows".into());
        }
        let source = self
            .document
            .rows
            .get(index)
            .ok_or("Table no longer exists")?;
        if source.kind != "table" {
            return Err("Choose a table row".into());
        }
        let points = table_points(&source.table)?;
        let fit = regression::fit(&points, model)?;
        let mut row = Row::expression(&fit.equation, source.color);
        if self.document.degrees && model == RegressionModel::Sinusoidal {
            row.text = row
                .text
                .replace("sin(", "sin(180/pi*")
                .replace("cos(", "cos(180/pi*");
        }
        row.annotation = format!(
            "{} regression · R² = {} · RMSE = {}",
            model.label(),
            exact::decimal(fit.r_squared),
            exact::decimal(fit.rmse)
        );
        self.status = row.annotation.clone();
        self.document.rows.push(row);
        self.recompile();
        Ok(())
    }
    pub fn generate_from_row(
        &mut self,
        index: usize,
        start: f64,
        end: f64,
        step: f64,
    ) -> Result<(), String> {
        if self.document.rows.len() >= 128 {
            return Err("A graph can contain at most 128 rows".into());
        }
        let source = self
            .document
            .rows
            .get(index)
            .ok_or("Expression no longer exists")?;
        let mut visible = self.document.clone();
        visible.rows[index].visible = true;
        let data = regression::generate_table(&visible.compile(), index, start, end, step)?;
        let mut row = Row::expression("", source.color);
        row.kind = "table".into();
        row.table = data;
        row.annotation = format!(
            "Generated from expression {} · {} to {} by {}",
            index + 1,
            exact::decimal(start),
            exact::decimal(end),
            exact::decimal(step)
        );
        self.document.rows.push(row);
        self.status = "Generated table with full-precision values".into();
        self.recompile();
        Ok(())
    }
    pub fn from_document(document: Document) -> Self {
        let mut app = Self {
            document,
            ..Self::default()
        };
        app.recompile();
        app.saved = app.snapshot();
        app
    }

    pub fn new(cc: &eframe::CreationContext<'_>, file: Option<&Path>) -> Self {
        Self::style(&cc.egui_ctx);
        let mut app = Self::default();
        let data = data_dir();
        if std::fs::create_dir_all(&data).is_ok() {
            app.autosave = Some(data.join("session.rustgraph"));
        }
        if let Some(file) = file {
            app.open_path(file);
        } else if let Some(path) = app.autosave.clone().filter(|p| p.exists()) {
            match Document::read(&path) {
                Ok(d) => {
                    app.document = d;
                    app.status = "Restored your last session".into();
                    app.recompile();
                }
                Err(e) => app.status = format!("Could not restore session: {e}"),
            }
        }
        app.saved = app.snapshot();
        app
    }
    pub fn style(ctx: &egui::Context) {
        ctx.set_visuals(egui::Visuals::light());
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "STIX Two Math".into(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../assets/STIXTwoMath-Regular.ttf"
            ))),
        );
        fonts.families.insert(
            egui::FontFamily::Name("math".into()),
            vec!["STIX Two Math".into()],
        );
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push("STIX Two Math".into());
        }
        ctx.set_fonts(fonts);
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(10.0, 7.0);
        style.visuals.selection.bg_fill = GREEN.gamma_multiply(0.22);
        style.visuals.selection.stroke = Stroke::new(1.0_f32, GREEN);
        style.visuals.widgets.active.bg_fill = Color32::from_rgb(224, 239, 231);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(235, 240, 237);
        style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, INK);
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, FontId::proportional(22.0));
        ctx.set_style(style);
    }
    fn snapshot(&self) -> String {
        serde_json::to_string(&self.document).unwrap_or_default()
    }
    fn recompile(&mut self) {
        self.scene = self.document.compile();
        self.cache_view = None;
        self.feature_view = None;
        self.feature_points.clear();
        self.selected_point = None;
        self.feature_changed = Instant::now();
    }
    pub fn undo(&mut self) {
        if let Some(d) = self.history.pop() {
            self.future.push(self.document.clone());
            self.document = d;
            self.skip_history = true;
            self.recompile();
        }
    }
    pub fn redo(&mut self) {
        if let Some(d) = self.future.pop() {
            self.history.push(self.document.clone());
            self.document = d;
            self.skip_history = true;
            self.recompile();
        }
    }
    fn remember(&mut self, before: Document) {
        if self.skip_history {
            self.skip_history = false;
            return;
        }
        if serde_json::to_string(&before).unwrap_or_default() != self.snapshot() {
            let math_changed = before.degrees != self.document.degrees
                || serde_json::to_string(&before.rows).unwrap_or_default()
                    != serde_json::to_string(&self.document.rows).unwrap_or_default();
            self.history.push(before);
            if self.history.len() > 100 {
                self.history.remove(0);
            }
            self.future.clear();
            if math_changed {
                self.recompile();
            }
        }
    }
    fn add_row(&mut self, kind: &str) {
        if self.document.rows.len() >= 128 {
            self.status = "A graph can contain at most 128 rows".into();
            return;
        }
        let i = self.document.rows.len();
        let mut row = Row::expression("", i);
        row.kind = kind.into();
        if kind == "table" {
            row.table = "-2, 4\n-1, 1\n0, 0\n1, 1\n2, 4".into();
        }
        self.document.rows.push(row);
        self.focused = i;
        self.focus_next = Some(i);
    }
    pub fn open_path(&mut self, path: &Path) {
        match Document::read(path) {
            Ok(d) => {
                self.document = d;
                self.path = Some(path.into());
                self.saved = self.snapshot();
                self.status = format!("Opened {}", path.display());
                self.recompile();
            }
            Err(e) => self.status = e,
        }
    }
    fn open_dialog(&mut self) {
        if let Some(file) = rfd::FileDialog::new()
            .add_filter("Rust Graph", &["rustgraph", "json"])
            .pick_file()
        {
            self.open_path(&file);
        }
    }
    fn save_dialog(&mut self, save_as: bool) {
        let path = if save_as || self.path.is_none() {
            rfd::FileDialog::new()
                .add_filter("Rust Graph", &["rustgraph"])
                .set_file_name("graph.rustgraph")
                .save_file()
        } else {
            self.path.clone()
        };
        if let Some(path) = path {
            match self.document.write(&path) {
                Ok(()) => {
                    self.path = Some(path.clone());
                    self.saved = self.snapshot();
                    self.status = format!("Saved {}", path.display());
                }
                Err(e) => self.status = format!("Save failed: {e}"),
            }
        }
    }
    fn export_dialog(&mut self, format: &str) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(format, &[format])
            .set_file_name(format!("graph.{format}"))
            .save_file()
        {
            let svg = export::svg(&self.document, &self.document.compile(), 1600, 1000);
            let result = if format == "png" {
                export::png(&svg)
                    .and_then(|bytes| std::fs::write(&path, bytes).map_err(|e| e.to_string()))
            } else {
                std::fs::write(&path, svg).map_err(|e| e.to_string())
            };
            self.status = match result {
                Ok(()) => format!("Exported {}", path.display()),
                Err(e) => format!("Export failed: {e}"),
            };
        }
    }
    fn example(&mut self, name: &str) {
        let rows: Vec<&str> = match name {
            "Waves & sliders" => vec!["a=1", "b=1", "y=a*sin(b*x)", "y=cos(x)"],
            "Implicit curves" => vec!["x^2+y^2=25", "(x^2+y^2-1)^3=x^2*y^3", "y>x^2-3"],
            "Polar roses" => vec!["r=4*cos(3*theta)", "r=2"],
            "Parametric" => vec!["(3*cos(t),2*sin(t))", "(t*cos(t),t*sin(t))"],
            "Piecewise" => vec!["y={x<0:-x,x^2}", "y=2*sin(x) {-4<x<4}", "(2,3)"],
            _ => vec![""],
        };
        self.document = Document {
            rows: rows
                .iter()
                .enumerate()
                .map(|(i, s)| Row::expression(s, i))
                .collect(),
            title: name.into(),
            ..Document::default()
        };
        if name == "Parametric" {
            for row in &mut self.document.rows {
                row.min = 0.0;
                row.max = std::f64::consts::TAU;
            }
        }
        self.path = None;
        self.status = format!("Loaded {name}");
        self.recompile();
    }
    fn animate(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64().min(0.15);
        self.last_tick = now;
        let mut changed = false;
        for row in &mut self.document.rows {
            if row.animate
                && let Some((name, value)) = parameter(&row.text)
            {
                let mut value = value + dt * (row.max - row.min) / 6.0;
                if value > row.max {
                    value = row.min;
                }
                row.text = format!("{name}={value:.8}");
                changed = true;
            }
        }
        if changed {
            self.recompile();
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let (open, save, save_as, undo, redo) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::CTRL, Key::O),
                i.consume_key(egui::Modifiers::CTRL, Key::S),
                i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, Key::S),
                i.consume_key(egui::Modifiers::CTRL, Key::Z),
                i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, Key::Z)
                    || i.consume_key(egui::Modifiers::CTRL, Key::Y),
            )
        });
        if open {
            self.open_dialog();
        }
        if save || save_as {
            self.save_dialog(save_as);
        }
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
    }
    pub fn ui(&mut self, ctx: &egui::Context) {
        if !self.styled {
            Self::style(ctx);
            self.styled = true;
        }
        self.animate(ctx);
        let before = self.document.clone();
        self.shortcuts(ctx);
        self.header(ctx);
        self.sidebar(ctx);
        self.footer(ctx);
        self.canvas(ctx);
        if self.settings {
            self.settings_window(ctx);
        }
        if self.help {
            self.help_window(ctx);
        }
        if self.point_inspector {
            self.points_window(ctx);
        }
        if self.table_dialog.is_some() {
            self.table_window(ctx);
        }
        self.remember(before);
        if self.last_autosave.elapsed() > Duration::from_secs(3) {
            if let Some(path) = &self.autosave
                && let Err(e) = self.document.write(path)
            {
                self.status = format!("Autosave failed: {e}");
            }
            self.last_autosave = Instant::now();
        }
        ctx.request_repaint_after(Duration::from_secs(1));
    }
    fn header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .exact_height(58.0)
            .frame(
                egui::Frame::new()
                    .fill(INK)
                    .inner_margin(egui::Margin::symmetric(16, 9)),
            )
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(Color32::WHITE);
                ui.visuals_mut().popup_shadow = egui::epaint::Shadow::NONE;
                ui.visuals_mut().window_fill = INK;
                let widgets = &mut ui.visuals_mut().widgets;
                for widgets in [
                    &mut widgets.inactive,
                    &mut widgets.hovered,
                    &mut widgets.active,
                ] {
                    widgets.bg_fill = Color32::from_rgb(53, 64, 69);
                    widgets.weak_bg_fill = Color32::from_rgb(53, 64, 69);
                    widgets.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
                }
                ui.horizontal_centered(|ui| {
                    brand_icon(ui);
                    ui.label(
                        RichText::new("RUST GRAPH")
                            .size(16.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.add_space(12.0);
                    ui.menu_button("File", |ui| {
                        if ui.button("New graph").clicked() {
                            self.example("Untitled Graph");
                            ui.close();
                        }
                        if ui.button("Open…                  Ctrl+O").clicked() {
                            self.open_dialog();
                            ui.close();
                        }
                        if ui.button("Save                     Ctrl+S").clicked() {
                            self.save_dialog(false);
                            ui.close();
                        }
                        if ui.button("Save as…").clicked() {
                            self.save_dialog(true);
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Export PNG…").clicked() {
                            self.export_dialog("png");
                            ui.close();
                        }
                        if ui.button("Export SVG…").clicked() {
                            self.export_dialog("svg");
                            ui.close();
                        }
                    });
                    ui.add(
                        egui::TextEdit::singleline(&mut self.document.title)
                            .desired_width(220.0)
                            .font(FontId::proportional(15.0))
                            .text_color(Color32::WHITE)
                            .frame(false)
                            .char_limit(256),
                    );
                    if self.snapshot() != self.saved {
                        ui.label(RichText::new("•").color(Color32::from_rgb(155, 227, 204)));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Help").clicked() {
                            self.help = true;
                        }
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Save graph").color(Color32::WHITE),
                                )
                                .fill(GREEN),
                            )
                            .clicked()
                        {
                            self.save_dialog(false);
                        }
                        ui.menu_button("Examples", |ui| {
                            for name in [
                                "Waves & sliders",
                                "Implicit curves",
                                "Polar roses",
                                "Parametric",
                                "Piecewise",
                            ] {
                                if ui.button(name).clicked() {
                                    self.example(name);
                                    ui.close();
                                }
                            }
                        });
                        ui.label(
                            RichText::new("OFFLINE")
                                .size(10.0)
                                .color(Color32::from_rgb(155, 185, 177)),
                        );
                    });
                });
            });
    }
    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("expressions")
            .default_width(360.0)
            .min_width(290.0)
            .max_width(600.0)
            .resizable(true)
            .frame(egui::Frame::new().fill(Color32::from_rgb(247, 248, 249)))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::WHITE)
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.menu_button(RichText::new("+ Add").color(GREEN).strong(), |ui| {
                                for (label, kind) in [
                                    ("Expression", "expression"),
                                    ("Table", "table"),
                                    ("Note", "note"),
                                ] {
                                    if ui.button(label).clicked() {
                                        self.add_row(kind);
                                        ui.close();
                                    }
                                }
                            });
                            if ui
                                .add_enabled(!self.history.is_empty(), egui::Button::new("Undo"))
                                .on_hover_text("Undo · Ctrl+Z")
                                .clicked()
                            {
                                self.undo();
                            }
                            if ui
                                .add_enabled(!self.future.is_empty(), egui::Button::new("Redo"))
                                .on_hover_text("Redo · Ctrl+Shift+Z")
                                .clicked()
                            {
                                self.redo();
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.selectable_label(self.keypad, "Keypad").clicked() {
                                    self.keypad = !self.keypad;
                                }
                                ui.label(
                                    RichText::new(format!("{}", self.document.rows.len()))
                                        .small()
                                        .weak(),
                                );
                            });
                        });
                    });
                ui.add_space(8.0);
                if self.keypad {
                    self.keypad_ui(ui);
                    ui.add_space(8.0);
                }
                let pretty_math = self.document.pretty_math;
                let exact_values = self.document.exact_values;
                let mut table_source = None;
                let mut remove = None;
                let mut insert = None;
                let mut fit = None;
                let mut move_row = None;
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let row_count = self.document.rows.len();
                        for i in 0..row_count {
                            let row = &mut self.document.rows[i];
                            let error = self.scene.errors.get(i).and_then(|e| e.as_deref());
                            let frame = egui::Frame::new()
                                .fill(Color32::WHITE)
                                .stroke(Stroke::new(
                                    1.0_f32,
                                    if error.is_some() {
                                        Color32::from_rgb(232, 182, 182)
                                    } else {
                                        Color32::from_rgb(226, 229, 231)
                                    },
                                ))
                                .inner_margin(12)
                                .corner_radius(6);
                            ui.push_id(i, |ui| {
                                frame.show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let response = color_dot(ui, row.color, row.visible);
                                        if response.clicked() {
                                            row.visible = !row.visible;
                                        }
                                        response.context_menu(|ui| {
                                            ui.label("Graph color");
                                            ui.horizontal(|ui| {
                                                for c in 0..6 {
                                                    if color_dot(ui, c, true).clicked() {
                                                        row.color = c;
                                                        ui.close();
                                                    }
                                                }
                                            });
                                        });
                                        ui.label(
                                            RichText::new(format!("{}", i + 1)).small().weak(),
                                        );
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                if ui
                                                    .small_button("×")
                                                    .on_hover_text("Delete row")
                                                    .clicked()
                                                {
                                                    remove = Some(i);
                                                }
                                                ui.menu_button("...", |ui| {
                                                    if ui.button("Move up").clicked() && i > 0 {
                                                        move_row = Some((i, i - 1));
                                                        ui.close();
                                                    }
                                                    if ui.button("Move down").clicked()
                                                        && i + 1 < row_count
                                                    {
                                                        move_row = Some((i, i + 1));
                                                        ui.close();
                                                    }
                                                    if row.kind == "expression"
                                                        && ui.button("Generate table…").clicked()
                                                    {
                                                        table_source = Some(i);
                                                        ui.close();
                                                    }
                                                    if ui.button("Duplicate").clicked() {
                                                        insert = Some(i);
                                                        ui.close();
                                                    }
                                                });
                                                if row.kind != "expression" {
                                                    ui.label(
                                                        RichText::new(row.kind.to_uppercase())
                                                            .size(10.0)
                                                            .weak(),
                                                    );
                                                }
                                            },
                                        );
                                    });
                                    match row.kind.as_str() {
                                        "note" => {
                                            ui.add(
                                                egui::TextEdit::multiline(&mut row.text)
                                                    .desired_width(f32::INFINITY)
                                                    .desired_rows(3)
                                                    .hint_text("Write a note…")
                                                    .char_limit(4096),
                                            );
                                        }
                                        "table" => {
                                            if let Ok(points) = table_points(&row.table) {
                                                egui::ScrollArea::vertical()
                                                    .id_salt(("table_preview", i))
                                                    .max_height(190.0)
                                                    .show(ui, |ui| {
                                                        egui::Grid::new(("data_grid", i))
                                                            .striped(true)
                                                            .spacing([14.0, 5.0])
                                                            .show(ui, |ui| {
                                                                ui.strong("x");
                                                                ui.strong("y");
                                                                ui.end_row();
                                                                for point in points.iter().take(200)
                                                                {
                                                                    for value in point {
                                                                        typeset::show(
                                                                            ui,
                                                                            &exact::display(
                                                                                *value,
                                                                                exact_values,
                                                                            ),
                                                                            16.0,
                                                                        );
                                                                    }
                                                                    ui.end_row();
                                                                }
                                                            });
                                                    });
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{} points{}",
                                                        points.len(),
                                                        if points.len() > 200 {
                                                            " · first 200 shown"
                                                        } else {
                                                            ""
                                                        }
                                                    ))
                                                    .small()
                                                    .weak(),
                                                );
                                            }
                                            ui.collapsing("Edit table data (CSV)", |ui| {
                                                ui.add(
                                                    egui::TextEdit::multiline(&mut row.table)
                                                        .font(egui::TextStyle::Monospace)
                                                        .desired_width(f32::INFINITY)
                                                        .desired_rows(5)
                                                        .char_limit(131072),
                                                );
                                            });
                                            ui.horizontal(|ui| {
                                                egui::ComboBox::from_id_salt(("regression", i))
                                                    .width(140.0)
                                                    .selected_text(model_label(&row.regression))
                                                    .show_ui(ui, |ui| {
                                                        for (key, label) in REGRESSION_CHOICES {
                                                            ui.selectable_value(
                                                                &mut row.regression,
                                                                key.to_string(),
                                                                *label,
                                                            );
                                                        }
                                                    });
                                                if ui.button("Fit").clicked() {
                                                    fit =
                                                        Some((i, model_from_key(&row.regression)));
                                                }
                                            });
                                        }
                                        _ => {
                                            let id = Id::new(("expression", i));
                                            if self.focus_next == Some(i) {
                                                ui.memory_mut(|m| m.request_focus(id));
                                                self.focus_next = None;
                                            }
                                            let editing = row.text.trim().is_empty()
                                                || ui.memory(|m| m.has_focus(id))
                                                || self.focus_next == Some(i);
                                            let response = if pretty_math && !editing {
                                                let content = egui::ScrollArea::horizontal()
                                                    .id_salt(("math_row", i))
                                                    .auto_shrink([false, true])
                                                    .max_width(ui.available_width())
                                                    .show(ui, |ui| {
                                                        typeset::show(ui, &row.text, 24.0)
                                                    });
                                                let math = content.inner;
                                                let click = ui
                                                    .interact(
                                                        math.rect.intersect(ui.clip_rect()),
                                                        id.with("rendered"),
                                                        Sense::click(),
                                                    )
                                                    .on_hover_text(
                                                        "Click to edit · plain math or LaTeX",
                                                    );
                                                if click.clicked() {
                                                    self.focus_next = Some(i);
                                                    self.focused = i;
                                                }
                                                click
                                            } else {
                                                ui.add(
                                                    egui::TextEdit::singleline(&mut row.text)
                                                        .id(id)
                                                        .font(FontId::monospace(18.0))
                                                        .desired_width(f32::INFINITY)
                                                        .hint_text("Type math or LaTeX…")
                                                        .char_limit(4096),
                                                )
                                            };
                                            if pretty_math
                                                && response.has_focus()
                                                && !row.text.trim().is_empty()
                                            {
                                                egui::ScrollArea::horizontal()
                                                    .id_salt(("math_preview", i))
                                                    .auto_shrink([false, true])
                                                    .max_width(ui.available_width())
                                                    .show(ui, |ui| {
                                                        typeset::show(ui, &row.text, 23.0)
                                                    });
                                            }
                                            if response.has_focus() {
                                                self.focused = i;
                                            }
                                            if response.lost_focus()
                                                && ui.input(|input| input.key_pressed(Key::Enter))
                                            {
                                                insert = Some(usize::MAX);
                                            }
                                            if let Some((name, mut value)) = parameter(&row.text) {
                                                ui.horizontal(|ui| {
                                                    if ui
                                                        .small_button(if row.animate {
                                                            "Pause"
                                                        } else {
                                                            "Play"
                                                        })
                                                        .clicked()
                                                    {
                                                        row.animate = !row.animate;
                                                    }
                                                    let slider = egui::Slider::new(
                                                        &mut value,
                                                        row.min..=row.max,
                                                    )
                                                    .step_by(row.step)
                                                    .show_value(true);
                                                    if ui.add(slider).changed() {
                                                        row.text = format!("{name}={value:.8}");
                                                    }
                                                });
                                                slider_range(ui, row);
                                            } else if row.text.trim_start().starts_with("r=")
                                                || row.text.trim_start().starts_with("r =")
                                                || row.text.contains('t')
                                                    && row.text.trim_start().starts_with('(')
                                            {
                                                ui.label(
                                                    RichText::new("Parameter range")
                                                        .size(11.0)
                                                        .weak(),
                                                );
                                                slider_range(ui, row);
                                            } else if let Some(value) =
                                                self.scene.values.get(i).and_then(|v| *v)
                                            {
                                                let label = if exact_values {
                                                    let normalized =
                                                        crate::math::normalize_tex(&row.text)
                                                            .unwrap_or_else(|_| row.text.clone());
                                                    let expression = Expr::parse(&normalized).ok();
                                                    expression
                                                        .as_ref()
                                                        .and_then(|e| {
                                                            let body = match e {
                                                                Expr::Compare(values, ops)
                                                                    if ops.len() == 1
                                                                        && ops[0] == "=" =>
                                                                {
                                                                    &values[1]
                                                                }
                                                                _ => e,
                                                            };
                                                            exact::exact_expr(body, &self.scene.env)
                                                        })
                                                        .unwrap_or_else(|| {
                                                            exact::display(value, true)
                                                        })
                                                } else {
                                                    exact::decimal(value)
                                                };
                                                ui.horizontal(|ui| {
                                                    ui.label("=");
                                                    typeset::show(ui, &label, 20.0);
                                                });
                                            }
                                        }
                                    }
                                    if !row.annotation.is_empty() {
                                        ui.label(
                                            RichText::new(&row.annotation).size(11.0).color(GREEN),
                                        );
                                    }
                                    if let Some(error) = error {
                                        ui.label(
                                            RichText::new(error)
                                                .size(12.0)
                                                .color(Color32::from_rgb(178, 48, 59)),
                                        );
                                    }
                                });
                            });
                            ui.add_space(8.0);
                        }
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            if ui
                                .button(RichText::new("+ Expression").color(GREEN))
                                .clicked()
                            {
                                insert = Some(usize::MAX);
                            }
                        });
                        ui.add_space(24.0);
                    });
                if let Some(i) = remove {
                    self.document.rows.remove(i);
                    self.focused = self.focused.min(self.document.rows.len().saturating_sub(1));
                }
                if let Some((a, b)) = move_row
                    && b < self.document.rows.len()
                {
                    self.document.rows.swap(a, b);
                }
                if let Some(i) = insert {
                    if i == usize::MAX {
                        self.add_row("expression");
                    } else if self.document.rows.len() < 128 {
                        self.document
                            .rows
                            .insert(i + 1, self.document.rows[i].clone());
                    }
                }
                if let Some((i, model)) = fit
                    && let Err(e) = self.fit_table(i, model)
                {
                    self.status = e;
                }
                if let Some(i) = table_source {
                    self.table_dialog = Some(i);
                    self.table_start = -5.0;
                    self.table_end = 5.0;
                    self.table_step = 1.0;
                }
            });
    }
    fn keypad_ui(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(Color32::from_rgb(235, 239, 237))
            .inner_margin(10)
            .show(ui, |ui| {
                egui::Grid::new("keypad")
                    .spacing(Vec2::splat(5.0))
                    .show(ui, |ui| {
                        for row in [
                            ["x", "y", "(", ")", "^"],
                            ["7", "8", "9", "/", "sqrt("],
                            ["4", "5", "6", "*", "sin("],
                            ["1", "2", "3", "-", "cos("],
                            ["0", ".", "pi", "+", "="],
                            ["{", "}", "<", ">", "abs("],
                        ] {
                            for text in row {
                                if ui
                                    .add_sized([48.0, 28.0], egui::Button::new(key_label(text)))
                                    .clicked()
                                    && let Some(row) = self.document.rows.get_mut(self.focused)
                                {
                                    row.text.push_str(text);
                                    self.focus_next = Some(self.focused);
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
    }
    fn footer(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("footer")
            .exact_height(34.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(247, 248, 249))
                    .inner_margin(egui::Margin::symmetric(14, 6)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.status).size(11.0).weak());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.selectable_label(self.document.degrees, "DEG").clicked() {
                            self.document.degrees = true;
                        }
                        if ui.selectable_label(!self.document.degrees, "RAD").clicked() {
                            self.document.degrees = false;
                        }
                        ui.checkbox(&mut self.trace, "Trace");
                        ui.checkbox(&mut self.document.show_features, "Notable points");
                        if ui.button("Point list").clicked() {
                            self.point_inspector = true;
                        }
                    });
                });
            });
    }
    fn canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::WHITE))
            .show(ctx, |ui| {
                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
                let rect = response.rect;
                let w = rect.width() as f64;
                let h = rect.height() as f64;
                if response.dragged() {
                    let delta = ctx.input(|i| i.pointer.delta());
                    let v = self.document.viewport;
                    self.document.viewport.pan(
                        -f64::from(delta.x) / w * (v.xmax - v.xmin),
                        f64::from(delta.y) / h * (v.ymax - v.ymin),
                    );
                }
                if response.hovered() {
                    let (scroll, pinch) = ctx.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
                    if (scroll.abs() > 0.1 || (pinch - 1.0).abs() > 0.001)
                        && let Some(pos) = response.hover_pos()
                    {
                        let anchor = self.document.viewport.to_world(
                            [
                                f64::from(pos.x - rect.left()),
                                f64::from(pos.y - rect.top()),
                            ],
                            w,
                            h,
                        );
                        let factor = (-f64::from(scroll) * 0.003).exp() / f64::from(pinch);
                        self.document.viewport.zoom(factor, anchor);
                    }
                }
                if response.double_clicked() {
                    self.document.viewport = Viewport::default();
                    self.document.viewport.square(w, h);
                }
                let v = self.document.viewport;
                let key = (v, w as usize, h as usize);
                if self.cache_view != Some(key) {
                    self.geometry = self.scene.geometry(v, w as usize, h as usize);
                    self.cache_view = Some(key);
                    self.feature_changed = Instant::now();
                }
                if self.document.show_features && self.feature_view != Some(key) {
                    ctx.request_repaint_after(Duration::from_millis(120));
                }
                if self.document.show_features
                    && self.feature_view != Some(key)
                    && !response.dragged()
                    && (self.feature_changed.elapsed() > Duration::from_millis(120)
                        || self.feature_updated.elapsed() > Duration::from_millis(350))
                {
                    self.feature_points = analysis::features(&self.scene, v, &self.geometry);
                    self.feature_view = Some(key);
                    self.feature_updated = Instant::now();
                }
                paint_grid(&painter, rect, v);
                for &(c, [x, y, dx, dy]) in &self.geometry.fills {
                    let a = screen(v, [x, y + dy], rect);
                    let b = screen(v, [x + dx, y], rect);
                    painter.rect_filled(
                        Rect::from_min_max(a, b).expand(0.2),
                        0.0,
                        color(c).gamma_multiply(0.14),
                    );
                }
                for (c, points) in &self.geometry.lines {
                    let points: Vec<Pos2> = points.iter().map(|p| screen(v, *p, rect)).collect();
                    painter.add(egui::Shape::line(points, Stroke::new(2.6_f32, color(*c))));
                }
                for &(c, p) in &self.geometry.points {
                    let p = screen(v, p, rect);
                    painter.circle_filled(p, 5.5, color(c));
                    painter.circle_stroke(p, 5.5, Stroke::new(1.5_f32, Color32::WHITE));
                }
                if self.document.show_features {
                    for point in &self.feature_points {
                        let center = screen(v, point.position, rect);
                        let shade = if point.kind == FeatureKind::Intersection {
                            INK
                        } else {
                            point
                                .rows
                                .first()
                                .and_then(|row| self.document.rows.get(*row))
                                .map(|row| color(row.color))
                                .unwrap_or(INK)
                        };
                        painter.circle_filled(center, 4.5, Color32::WHITE);
                        painter.circle_stroke(center, 4.5, Stroke::new(1.8_f32, shade));
                        painter.circle_filled(center, 2.2, shade);
                    }
                    if let Some(pos) = response.hover_pos().filter(|_| !response.dragged())
                        && let Some(point) = nearest_feature(&self.feature_points, v, rect, pos)
                    {
                        let center = screen(v, point.position, rect);
                        painter.circle_stroke(center, 7.0, Stroke::new(2.0_f32, GREEN));
                        let label = format!(
                            "{}  {}",
                            feature_name(point.kind),
                            coordinate_text(point.position, self.document.exact_values)
                        );
                        let galley = painter.layout_no_wrap(label, math_font(ctx, 15.0), INK);
                        let size = galley.size() + Vec2::new(18.0, 14.0);
                        let location = Pos2::new(
                            (center.x + 16.0)
                                .min(rect.right() - size.x - 8.0)
                                .max(rect.left() + 8.0),
                            (center.y - size.y - 12.0).max(rect.top() + 8.0),
                        );
                        let badge = Rect::from_min_size(location, size);
                        painter.rect_filled(badge, 6.0, Color32::WHITE);
                        painter.rect_stroke(
                            badge,
                            6.0,
                            Stroke::new(1.0_f32, Color32::from_gray(200)),
                            egui::StrokeKind::Inside,
                        );
                        painter.galley(location + Vec2::new(9.0, 7.0), galley, INK);
                        if response.clicked() {
                            self.selected_point = Some(point.clone());
                            self.point_inspector = true;
                        }
                    }
                }
                if self.trace
                    && let Some(pos) = response.hover_pos().filter(|pos| {
                        !response.dragged()
                            && (!self.document.show_features
                                || nearest_feature(&self.feature_points, v, rect, *pos).is_none())
                    })
                {
                    let local = [
                        f64::from(pos.x - rect.left()),
                        f64::from(pos.y - rect.top()),
                    ];
                    if let Some((c, p)) = self.scene.trace(&self.geometry, v, local, w, h) {
                        let point = screen(v, p, rect);
                        painter.circle_filled(point, 5.0, color(c));
                        painter.circle_stroke(point, 6.0, Stroke::new(2.0_f32, Color32::WHITE));
                        let text = coordinate_text(p, self.document.exact_values);
                        let galley = painter.layout_no_wrap(text, math_font(ctx, 14.0), INK);
                        let size = galley.size() + Vec2::new(16.0, 12.0);
                        let location = Pos2::new(
                            (point.x + 14.0).min(rect.right() - size.x - 8.0),
                            (point.y - size.y - 12.0).max(rect.top() + 8.0),
                        );
                        painter.rect_filled(
                            Rect::from_min_size(location, size),
                            5.0,
                            Color32::WHITE,
                        );
                        painter.rect_stroke(
                            Rect::from_min_size(location, size),
                            5.0,
                            Stroke::new(1.0_f32, Color32::from_gray(205)),
                            egui::StrokeKind::Inside,
                        );
                        painter.galley(location + Vec2::new(8.0, 6.0), galley, INK);
                    }
                }
                painter.text(
                    rect.left_bottom() + Vec2::new(16.0, -14.0),
                    egui::Align2::LEFT_BOTTOM,
                    "Drag to pan  ·  Scroll to zoom  ·  Double-click to reset",
                    FontId::proportional(11.0),
                    Color32::from_gray(130),
                );
                egui::Area::new(Id::new("graph_controls"))
                    .fixed_pos(rect.right_top() + Vec2::new(-62.0, 16.0))
                    .show(ctx, |ui| {
                        egui::Frame::new()
                            .fill(Color32::WHITE)
                            .stroke(Stroke::new(1.0_f32, Color32::from_gray(220)))
                            .inner_margin(5)
                            .corner_radius(7)
                            .show(ui, |ui| {
                                ui.set_width(37.0);
                                if graph_button(ui, GraphIcon::Settings).clicked() {
                                    self.settings = !self.settings;
                                }
                                ui.separator();
                                if graph_button(ui, GraphIcon::ZoomIn).clicked() {
                                    self.zoom_center(0.8);
                                }
                                if graph_button(ui, GraphIcon::ZoomOut).clicked() {
                                    self.zoom_center(1.25);
                                }
                                if graph_button(ui, GraphIcon::Home).clicked() {
                                    self.document.viewport = Viewport::default();
                                    self.document.viewport.square(w, h);
                                }
                            });
                    });
            });
    }
    fn zoom_center(&mut self, factor: f64) {
        let v = self.document.viewport;
        self.document
            .viewport
            .zoom(factor, [(v.xmin + v.xmax) / 2.0, (v.ymin + v.ymax) / 2.0]);
    }
    fn settings_window(&mut self, ctx: &egui::Context) {
        let mut open = self.settings;
        egui::Window::new("Graph settings")
            .open(&mut open)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.checkbox(&mut self.document.viewport.grid, "Show grid");
                ui.checkbox(&mut self.document.viewport.axes, "Show axes and labels");
                ui.checkbox(&mut self.trace, "Trace coordinates on hover");
                ui.checkbox(&mut self.document.show_features,"Mark turning points and intersections");
                ui.checkbox(&mut self.document.pretty_math,"Render mathematical notation");
                ui.separator();ui.label("Value display");
                ui.radio_value(&mut self.document.exact_values,false,"Decimal approximations (12 significant digits)");
                ui.radio_value(&mut self.document.exact_values,true,"Exact forms when available: π, roots, fractions");
                ui.small("Supported constant expressions can be exact. Numerically recognized forms and feature coordinates are marked ≈.");
                if ui.button("Inspect notable points").clicked(){self.point_inspector=true;}

                ui.separator();
                let mut v = self.document.viewport;
                egui::Grid::new("bounds").show(ui, |ui| {
                    for (label, value) in [
                        ("x minimum", &mut v.xmin),
                        ("x maximum", &mut v.xmax),
                        ("y minimum", &mut v.ymin),
                        ("y maximum", &mut v.ymax),
                    ] {
                        ui.label(label);
                        ui.add(egui::DragValue::new(value).speed(0.1));
                        ui.end_row();
                    }
                });
                if v.valid() {
                    self.document.viewport = v;
                } else {
                    ui.colored_label(
                        Color32::from_rgb(178, 48, 59),
                        "Bounds must increase and stay finite.",
                    );
                }
                if ui.button("Equal axis scale").clicked()
                    && let Some((_, w, h)) = self.cache_view
                {
                    self.document.viewport.square(w as f64, h as f64);
                }
                ui.separator();
                ui.label("Angle units");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.document.degrees, false, "Radians");
                    ui.radio_value(&mut self.document.degrees, true, "Degrees");
                });
            });
        self.settings = open;
    }
    fn table_window(&mut self, ctx: &egui::Context) {
        let Some(index) = self.table_dialog else {
            return;
        };
        let mut open = true;
        let mut generate = false;
        egui::Window::new("Generate a table from an equation").open(&mut open).default_width(400.0).show(ctx,|ui| {
            ui.label(format!("Source: expression {}",index+1));
            if let Some(row)=self.document.rows.get(index){typeset::show(ui,&row.text,22.0);}
            ui.label("Set the x range, or the parameter range for polar and parametric curves.");
            egui::Grid::new("table_range").show(ui,|ui| {for (label,value) in [("Start",&mut self.table_start),("End",&mut self.table_end),("Step",&mut self.table_step)] {ui.label(label);ui.add(egui::DragValue::new(value).speed(0.1).max_decimals(12));ui.end_row();}});
            ui.small("Up to 2048 samples. Undefined values and points outside domain restrictions are skipped. Values are stored at full floating-point precision.");
            if ui.button("Generate table").clicked(){generate=true;}
        });
        if generate {
            match self.generate_from_row(index, self.table_start, self.table_end, self.table_step) {
                Ok(()) => open = false,
                Err(e) => self.status = e,
            }
        }
        if !open {
            self.table_dialog = None;
        }
    }
    fn points_window(&mut self, ctx: &egui::Context) {
        let mut open = self.point_inspector;
        egui::Window::new("Notable points").open(&mut open).default_width(420.0).show(ctx,|ui| {
            if let Some(point)=&self.selected_point {
                ui.heading(feature_name(point.kind));typeset::show(ui,&coordinate_text(point.position,self.document.exact_values),25.0);
                ui.label(format!("Expressions: {}",point.rows.iter().map(|r|(r+1).to_string()).collect::<Vec<_>>().join(", ")));
                ui.small("Position refined numerically. ≈ denotes an approximation, including recognized π or radical forms.");ui.separator();
            }
            if self.feature_points.is_empty(){ui.label("No notable points found in this view. Enable markers in Graph settings and let the graph settle.");}
            egui::ScrollArea::vertical().max_height(360.0).show(ui,|ui| {
                for point in &self.feature_points {
                    let label=format!("{} · {}",feature_name(point.kind),coordinate_text(point.position,self.document.exact_values));
                    if ui.selectable_label(self.selected_point.as_ref().is_some_and(|p|p.kind==point.kind&&p.position==point.position),label).clicked(){self.selected_point=Some(point.clone());}
                }
            });
            ui.separator();ui.small("Markers use bounded numerical analysis. Closely spaced or very small features may require zooming. Explicit, implicit, polar and parametric curves are supported; densely packed features may be missed by finite sampling.");
        });
        self.point_inspector = open;
    }
    fn help_window(&mut self, ctx: &egui::Context) {
        let mut open = self.help;
        egui::Window::new("Welcome to Rust Graph").open(&mut open).default_width(550.0).show(ctx,|ui| {
            ui.heading("A little space to explore mathematics.");ui.label("Enter expressions on the left. Every graph updates as you type.");ui.add_space(12.0);
            egui::Grid::new("help_examples").striped(true).spacing(Vec2::new(18.0,9.0)).show(ui,|ui| {
                for (label,sample) in [("Functions","y = sin(x),  f(u) = u^2 + 1"),("Implicit relations","x^2 + y^2 = 25"),("Inequalities","y < x^2"),("Domain restrictions","y = sqrt(x) {0 < x < 9}"),("Piecewise","y = {x < 0: -x, x^2}"),("Polar","r = 4*cos(3*theta)"),("Parametric","(3*cos(t), 2*sin(t))"),("Points","(2, 3)"),("Sliders","a = 1, then y = a*sin(x)"),("Tables","Add → Table; one x, y pair per line"),("Arithmetic","2x, pi, e, ^, !, sqrt, abs, ln, log"),("Math notation",r"\sqrt{x}, \frac{1}{2}, x^{2}"),("Analysis","Click marked turning points and intersections"),("Regression","Choose a model in a table, then Fit"),("Generate table","Expression … menu → Generate table"),("Exact values","Settings → Exact values or decimal approximations")]
                {ui.label(label);ui.monospace(sample);ui.end_row();}
            });ui.add_space(12.0);ui.label("Right-click a color dot to change it. Use Enter to add another expression. Ctrl+Z / Ctrl+Shift+Z undo and redo. Save/open .rustgraph files through File. PNG and SVG exports contain the current graph view.");
            ui.separator();ui.small("Independent open-source implementation inspired by Desmos. Advanced list algebra, symbolic solving, audio tracing, images and actions are outside this version.");
        });
        self.help = open;
    }
}
impl eframe::App for GraphApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.ui(ctx);
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        if let Some(path) = &self.autosave {
            let _ = self.document.write(path);
        }
    }
}
fn data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".local/share")
        })
        .join("rust-graph")
}
fn slider_range(ui: &mut egui::Ui, row: &mut Row) {
    let (mut min, mut max, mut step) = (row.min, row.max, row.step);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(RichText::new("min").small().weak());
        ui.add(egui::DragValue::new(&mut min).speed(0.1).max_decimals(4));
        ui.label(RichText::new("max").small().weak());
        ui.add(egui::DragValue::new(&mut max).speed(0.1).max_decimals(4));
        ui.label(RichText::new("step").small().weak());
        ui.add(egui::DragValue::new(&mut step).speed(0.01).max_decimals(4));
    });
    if valid_range(min, max, step) {
        row.min = min;
        row.max = max;
        row.step = step;
    }
}
fn screen(v: Viewport, p: [f64; 2], rect: Rect) -> Pos2 {
    let p = v.to_screen(p, f64::from(rect.width()), f64::from(rect.height()));
    Pos2::new(rect.left() + p[0] as f32, rect.top() + p[1] as f32)
}
fn paint_grid(painter: &egui::Painter, rect: Rect, v: Viewport) {
    let w = f64::from(rect.width());
    let h = f64::from(rect.height());
    let xs = grid_step(v.xmax - v.xmin, w);
    let ys = grid_step(v.ymax - v.ymin, h);
    if v.grid {
        for (vertical, step, min, max) in [(true, xs, v.xmin, v.xmax), (false, ys, v.ymin, v.ymax)]
        {
            let minor = step / 5.0;
            let start = (min / minor).ceil() as i64;
            let end = (max / minor).floor() as i64;
            for n in start..=end {
                let value = n as f64 * minor;
                let shade = if n % 5 == 0 {
                    Color32::from_rgb(200, 204, 209)
                } else {
                    Color32::from_rgb(234, 236, 238)
                };
                let p = screen(v, if vertical { [value, 0.0] } else { [0.0, value] }, rect);
                let (a, b) = if vertical {
                    (Pos2::new(p.x, rect.top()), Pos2::new(p.x, rect.bottom()))
                } else {
                    (Pos2::new(rect.left(), p.y), Pos2::new(rect.right(), p.y))
                };
                painter.line_segment([a, b], Stroke::new(1.0_f32, shade));
            }
        }
    }
    if v.axes {
        let origin = screen(v, [0.0, 0.0], rect);
        painter.line_segment(
            [
                Pos2::new(origin.x, rect.top()),
                Pos2::new(origin.x, rect.bottom()),
            ],
            Stroke::new(1.4_f32, Color32::from_gray(90)),
        );
        painter.line_segment(
            [
                Pos2::new(rect.left(), origin.y),
                Pos2::new(rect.right(), origin.y),
            ],
            Stroke::new(1.4_f32, Color32::from_gray(90)),
        );
        for (vertical, step, min, max) in [(true, xs, v.xmin, v.xmax), (false, ys, v.ymin, v.ymax)]
        {
            let start = (min / step).ceil() as i64;
            let end = (max / step).floor() as i64;
            for n in start..=end {
                if n == 0 {
                    continue;
                }
                let value = n as f64 * step;
                let p = screen(v, if vertical { [value, 0.0] } else { [0.0, value] }, rect);
                let pos = if vertical {
                    Pos2::new(
                        p.x + 3.0,
                        origin.y.clamp(rect.top() + 5.0, rect.bottom() - 25.0) + 5.0,
                    )
                } else {
                    Pos2::new(
                        origin.x.clamp(rect.left() + 35.0, rect.right() - 40.0) - 7.0,
                        p.y - 3.0,
                    )
                };
                painter.text(
                    pos,
                    if vertical {
                        egui::Align2::LEFT_TOP
                    } else {
                        egui::Align2::RIGHT_BOTTOM
                    },
                    format_number(value),
                    FontId::proportional(12.0),
                    Color32::from_gray(95),
                );
            }
        }
    }
}

fn color_dot(ui: &mut egui::Ui, index: usize, visible: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
    let center = rect.center();
    if visible {
        ui.painter().circle_filled(center, 7.0, color(index));
    } else {
        ui.painter()
            .circle_stroke(center, 7.0, Stroke::new(2.0_f32, color(index)));
    }
    response.on_hover_text("Toggle graph · right-click for color")
}

fn brand_icon(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
    ui.painter().rect_filled(rect, 7.0, GREEN);
    let points = (0..=28)
        .map(|i| {
            let u = i as f32 / 28.0;
            Pos2::new(
                rect.left() + 5.0 + 22.0 * u,
                rect.center().y - 8.0 * (u * std::f32::consts::TAU).sin(),
            )
        })
        .collect();
    ui.painter().add(egui::Shape::line(
        points,
        Stroke::new(2.4_f32, Color32::from_rgb(155, 227, 204)),
    ));
}
enum GraphIcon {
    Settings,
    ZoomIn,
    ZoomOut,
    Home,
}
fn graph_button(ui: &mut egui::Ui, icon: GraphIcon) -> egui::Response {
    let response = ui.add_sized([37.0, 34.0], egui::Button::new(""));
    let c = response.rect.center();
    let painter = ui.painter();
    let stroke = Stroke::new(1.8_f32, Color32::from_rgb(75, 83, 87));
    let label = match icon {
        GraphIcon::ZoomIn | GraphIcon::ZoomOut => {
            painter.line_segment([c + Vec2::new(-7.0, 0.0), c + Vec2::new(7.0, 0.0)], stroke);
            if matches!(icon, GraphIcon::ZoomIn) {
                painter.line_segment([c + Vec2::new(0.0, -7.0), c + Vec2::new(0.0, 7.0)], stroke);
                "Zoom in"
            } else {
                "Zoom out"
            }
        }
        GraphIcon::Home => {
            let points = [
                [-9., -1.],
                [0., -9.],
                [9., -1.],
                [6., -1.],
                [6., 8.],
                [2., 8.],
                [2., 2.],
                [-2., 2.],
                [-2., 8.],
                [-6., 8.],
                [-6., -1.],
                [-9., -1.],
            ]
            .iter()
            .map(|p| c + Vec2::new(p[0], p[1]))
            .collect();
            painter.add(egui::Shape::line(points, stroke));
            "Reset view"
        }
        GraphIcon::Settings => {
            let points = (0..=32)
                .map(|i| {
                    let angle = i as f32 * std::f32::consts::TAU / 32.0;
                    let radius = if matches!(i % 4, 1 | 2) { 9.0 } else { 6.8 };
                    c + Vec2::new(angle.cos() * radius, angle.sin() * radius)
                })
                .collect();
            painter.add(egui::Shape::line(points, stroke));
            painter.circle_stroke(c, 3.0, stroke);
            "Graph settings"
        }
    };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    response.on_hover_text(label)
}

const REGRESSION_CHOICES: &[(&str, &str)] = &[
    ("linear", "Linear"),
    ("quadratic", "Quadratic"),
    ("cubic", "Cubic"),
    ("poly4", "Polynomial degree 4"),
    ("poly5", "Polynomial degree 5"),
    ("poly6", "Polynomial degree 6"),
    ("exponential", "Exponential"),
    ("logarithmic", "Logarithmic"),
    ("power", "Power"),
    ("sinusoidal", "Sinusoidal"),
];
fn model_from_key(key: &str) -> RegressionModel {
    match key {
        "quadratic" => RegressionModel::Quadratic,
        "cubic" => RegressionModel::Cubic,
        "poly4" => RegressionModel::Polynomial(4),
        "poly5" => RegressionModel::Polynomial(5),
        "poly6" => RegressionModel::Polynomial(6),
        "exponential" => RegressionModel::Exponential,
        "logarithmic" => RegressionModel::Logarithmic,
        "power" => RegressionModel::Power,
        "sinusoidal" => RegressionModel::Sinusoidal,
        _ => RegressionModel::Linear,
    }
}
fn model_label(key: &str) -> &str {
    REGRESSION_CHOICES
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, label)| *label)
        .unwrap_or("Linear")
}
fn key_label(key: &str) -> &str {
    match key {
        "sqrt(" => "√",
        "pi" => "π",
        "*" => "×",
        "/" => "÷",
        "^" => "xⁿ",
        _ => key,
    }
}
fn feature_name(kind: FeatureKind) -> &'static str {
    match kind {
        FeatureKind::Maximum => "Maximum / apex",
        FeatureKind::Minimum => "Minimum / turning point",
        FeatureKind::XIntercept => "x-intercept",
        FeatureKind::YIntercept => "y-intercept",
        FeatureKind::Intersection => "Intersection",
    }
}
fn coordinate_text(point: [f64; 2], exact_mode: bool) -> String {
    let x = exact::display(point[0], exact_mode);
    let y = exact::display(point[1], exact_mode);
    format!(
        "≈ ({}, {})",
        x.trim_start_matches("≈ "),
        y.trim_start_matches("≈ ")
    )
}
fn math_font(ctx: &egui::Context, size: f32) -> FontId {
    let family = egui::FontFamily::Name("math".into());
    FontId::new(
        size,
        if ctx.fonts(|f| f.families().contains(&family)) {
            family
        } else {
            egui::FontFamily::Proportional
        },
    )
}
fn nearest_feature(
    points: &[FeaturePoint],
    v: Viewport,
    rect: Rect,
    pos: Pos2,
) -> Option<&FeaturePoint> {
    points
        .iter()
        .filter(|p| screen(v, p.position, rect).distance(pos) < 10.0)
        .min_by(|a, b| {
            let priority = |kind: FeatureKind| match kind {
                FeatureKind::Intersection => 0,
                FeatureKind::Minimum | FeatureKind::Maximum => 1,
                _ => 2,
            };
            let da = screen(v, a.position, rect).distance(pos);
            let db = screen(v, b.position, rect).distance(pos);
            if (da - db).abs() < 0.5 {
                priority(a.kind).cmp(&priority(b.kind))
            } else {
                da.total_cmp(&db)
            }
        })
}
