# Rust Graph design

Build an offline native Linux graphing calculator in Rust for John, inspired by the expression sidebar and interactive grid inspected at https://www.desmos.com/calculator using headless Chrome. Publish to JohnDSdev/rust-graph and install for this user. The user approved the feature proposal and explicitly requested implementation.

## Architecture
Use eframe/egui with the Glow renderer and Wayland/X11 support. Separate a bounded Pratt parser and evaluator (`math.rs`), document/compiler (`document.rs`), viewport/sampler/export (`graph.rs`, `export.rs`), and desktop UI (`app.rs`). Parse expressions once per document change; cache geometry until expressions or viewport change. Evaluate definitions in dependency order, with recursion and work limits. All computations are local.

## Behavior
Expression rows support arithmetic, implicit multiplication, powers, factorials, trig/inverse trig/hyperbolic functions, logarithms, roots, absolute values, min/max, and named functions. Graph y=f(x), x=f(y), implicit relations, inequalities, polar r=f(theta), parametric (x(t),y(t)), points, and domain restrictions in braces. Parameters have editable ranges, step sizes and animation. Tables plot CSV points and can fit a line. Include notes, visibility, colors, keypad, undo/redo, radians/degrees, examples, tracing, pan/zoom, reset, grid/axis switches and editable viewport bounds.

Native dialogs save/open versioned JSON graph documents and export SVG/PNG. Autosave to the user's data directory, recover at startup. Reject corrupt or unsupported documents with a visible error and preserve the current graph. Render discontinuities without joining across poles; implicit contours use marching squares. Inequality fills use bounded screen-space sampling. Bound workload so invalid expressions do not lock the UI. Keep user-facing error messages near the affected expression.

## Desktop and release
Name: Rust Graph. Executable: rust-graph. Source directory: /home/john/rust-graph. Desktop entry and icon installed under ~/.local/share; executable under ~/.local/bin. Repository defaults to public with MIT license and CI. Independent implementation; no Desmos code/assets or accounts required. Explicitly document that advanced Desmos features such as symbolic algebra, arbitrary list operations, statistics/regression models beyond linear fitting, image overlays, actions and audio tracing are outside this version.

## Validation
Math tests pin precedence, implicit multiplication, functions, degree mode, domain/piecewise behavior, malformed input and recursion limits. Graph tests pin circles, explicit functions, poles, inequalities, coordinate transforms and zoom invariance. Document tests cover forward definitions, circular dependencies, save/open validation and undo. Headless egui input tests cover editing/adding expressions and viewport interaction; launch the actual native app on this Wayland desktop and capture screenshots. Run fmt, clippy, all tests, release build, install verification, and verify the published remote.
