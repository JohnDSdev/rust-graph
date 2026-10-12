# Mathematical Tools Implementation Plan

> **For agentic workers:** Use independent parallel domains and integrate/test inline.

**Goal:** Upgrade the installed and published graphing app with mathematical notation and numerical tools.
**Architecture:** Three independent pure-Rust numerical modules plus native typesetting/UI integration.
**Tech Stack:** Existing Rust/egui/serde stack; bundled STIX Two Math.
**Spec:** docs/superpowers/specs/2026-10-12-math-tools-design.md

## Global Constraints
- Preserve existing graph files and user's current session.
- Numerical guesses must not be mislabeled as proven exact values.
- Bounded sampling, refinement and table sizes.
- Root owns app/math/document/graph/typeset/lib/Cargo/CI changes; independent agents only own assigned modules/tests.

## Interfaces
- analysis.rs: `FeatureKind { Maximum, Minimum, XIntercept, YIntercept, Intersection }`, `FeaturePoint { position:[f64;2], kind:FeatureKind, rows:Vec<usize> }`, `features(scene:&Scene,view:Viewport,geometry:&Geometry)->Vec<FeaturePoint>`.
- regression.rs: `RegressionModel {Linear,Quadratic,Cubic,Polynomial(u8),Exponential,Logarithmic,Power,Sinusoidal}`, `Fit { equation:String, r_squared:f64, rmse:f64, parameters:Vec<f64> }`, `fit(points:&[[f64;2]],model:RegressionModel)->Result<Fit,String>`, `generate_table(scene:&Scene,row:usize,start:f64,end:f64,step:f64)->Result<String,String>`.
- exact.rs: `decimal(value:f64)->String`, `recognize(value:f64)->Option<String>`, `display(value:f64,prefer_exact:bool)->String`, `exact_expr(expr:&Expr,env:&Env)->Option<String>`; recognize/display mark irrational numeric inference approximate.
- typeset.rs: `show(ui:&mut egui::Ui,text:&str,size:f32)->egui::Response` renders bounded AST layout using STIX fonts.

## Review Focus
- Tangent intersections and poles must not be confused.
- Numeric exact-form recognition must not make false symbolic claims.
- Regression domains, repeated x and large shifted inputs must be handled.
- Generated tables stop at limits and retain enough precision.
- Existing files deserialize with defaults for new settings.

### Task 1: Independent numerical modules
- [x] Write failing tests, implement, and verify each assigned module in isolated /tmp test manifests.
### Task 2: Notation and UI
- [x] Write failing LaTeX parsing/presentation and UI/serialization tests.
- [x] Implement native math layout, persisted display/marker settings, feature inspector, regression selector and table-generation dialog.
### Task 3: Validate and deliver
- [x] Run complete tests, fmt, clippy and release build; visually verify native UI and font symbols.
- [x] Review module integration, update docs, install new binary, push repository and verify CI.
