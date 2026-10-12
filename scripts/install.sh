#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "${1:-}" != "--no-build" ]]; then cargo build --release --locked; fi
binary="${CARGO_TARGET_DIR:-target}/release/rust-graph"
install -Dm755 "$binary" "$HOME/.local/bin/rust-graph"
install -Dm644 assets/rust-graph.svg "$HOME/.local/share/icons/hicolor/scalable/apps/rust-graph.svg"
install -Dm644 assets/rust-graph.png "$HOME/.local/share/icons/hicolor/256x256/apps/rust-graph.png"
mkdir -p "$HOME/.local/share/applications"
cat > "$HOME/.local/share/applications/rust-graph.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Rust Graph
Comment=Offline graphing calculator
Exec="$HOME/.local/bin/rust-graph" %f
Icon=rust-graph
Terminal=false
Categories=Education;Math;
StartupWMClass=rust-graph
Keywords=graph;calculator;plot;math;desmos;
MimeType=application/x-rustgraph;
DESKTOP
install -Dm644 assets/application-x-rustgraph.xml "$HOME/.local/share/mime/packages/application-x-rustgraph.xml"
if command -v update-mime-database >/dev/null; then update-mime-database "$HOME/.local/share/mime"; fi
if command -v xdg-mime >/dev/null; then xdg-mime default rust-graph.desktop application/x-rustgraph; fi
if command -v update-desktop-database >/dev/null; then update-desktop-database "$HOME/.local/share/applications"; fi
if command -v gtk-update-icon-cache >/dev/null; then gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" >/dev/null 2>&1 || true; fi
printf 'Installed Rust Graph to %s/.local/bin/rust-graph\n' "$HOME"
