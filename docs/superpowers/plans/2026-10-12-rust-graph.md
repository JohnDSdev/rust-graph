# Rust Graph Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build, test, install and publish Rust Graph.
**Architecture:** Standalone Rust math core, document compiler, graph sampler and native egui frontend. Geometry is reused for screen rendering and SVG/PNG export.
**Tech Stack:** Rust 2024, eframe/egui 0.33, serde, rfd, resvg.
**Spec:** docs/superpowers/specs/2026-10-12-rust-graph-design.md

## Global Constraints
- Offline native Linux application, Rust 1.88 or newer.
- Repository JohnDSdev/rust-graph; binary rust-graph; MIT.
- Never replace the current graph on an invalid file.
- Bound expression recursion and sampler workloads.

## Review Focus
- Asymptotes must not create vertical connecting lines.
- Forward references and cycles must resolve or produce visible errors.
- Nonfinite coordinates never reach drawing operations.
- Invalid view bounds and slider ranges must not crash.
- Corrupt session files must preserve the current document.

### Task 1: Math engine
Files: src/math.rs, tests/math.rs, Cargo.toml, src/lib.rs.
Produces: Expr::parse(&str)->Result<Expr,String>, Expr::eval(&Env)->Result<f64,String>, Env variables/functions/degrees.
- [x] Write tests for precedence, multiplication, trig, piecewise, malformed syntax, and recursion; run cargo test and observe missing behavior.
- [x] Implement tokenization, Pratt AST, constrained evaluation and builtins; cargo test must pass.

### Task 2: Documents and geometry
Files: src/document.rs, src/graph.rs, tests/graph.rs.
Consumes Expr and Env. Produces Document::compile()->Scene, Scene::geometry(Viewport,width,height)->Geometry; Document::load/save; Viewport transforms/zoom.
- [x] Write tests for curves, circles, points, tables, restrictions, inequalities, poles, forward definitions, file validation and zoom invariance; observe failure.
- [x] Implement compiler, bounded sampling and serialization; cargo test must pass.

### Task 3: Desktop UI and export
Files: src/app.rs, src/main.rs, src/export.rs, tests/ui.rs.
Consumes Document, Scene, Viewport, Geometry. Produces eframe App, SVG/PNG exports and local document workflows.
- [x] Write headless egui tests for expression editing, undo/redo, viewport panning and exports; observe failure.
- [x] Implement native UI, sliders, tables, notes, keypad, history, cache, dialogs and autosave.
- [x] Verify cargo fmt --check, cargo clippy --all-targets -- -D warnings, cargo test, cargo build --release.
- [x] Launch native app and visually verify layout and plotted examples.

### Task 4: Install and publish
Files: README.md, LICENSE, assets/rust-graph.svg, scripts/install.sh, .github/workflows/ci.yml.
- [x] Install user-local executable, icon and desktop entry; verify desktop entry and installed binary.
- [ ] Commit source and create public GitHub repo, push main, verify remote and CI.
