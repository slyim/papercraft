# PaperCraft 0.3.0 preview

- **Complete 7-Tool Design Rail**: Select (`V`), Hand (`H`), Frame (`F`, `⇧F` wrap selection in frame, `⇧1` fit artboard), Swappable Shape (`R` — Rectangle, Ellipse, Triangle), Vector Pen (`P` — normalized paths & SVG `d` string import), Text (`T`), and Procedural Shader (`S`).
- **Brand Orange Canvas & Selection (`#FF6B2B`)**: Pixel-straddled selection outlines, white-filled / orange-bordered resize handles, hover outlines, and a solid orange `W × H` dimension pill.
- **Nested Frames & Clipping**: Container frames (`kind: "frame"`) with `parent` hierarchy, automatic child parenting when drawing inside a frame, descendant move/delete propagation, and ancestor `clip` masking across canvas, PNG, HTML, and SVG exports.
- **Vector Paths & Strokes**: Editable `NodeKind::Path` layers with open/closed polylines, SVG path parsing, and token-bound strokes (`stroke`, `stroke_token`, `stroke_width`) across frames, shapes, and paths.
- **Procedural Shaders**: Five built-in procedural effects (`LinearGradient`, `RadialGradient`, `Mesh`, `Grain`, `Halftone`) with secondary color, angle, and intensity controls in the Inspector and MCP.
- **Agent Directives & ACP Agent Studio Dock**: Right-hand sidebar tabs (`Inspect` · `Directives` · `Agent`) featuring spatial `#id` directive pins anchored to layers or canvas coordinates, plus an ACP Agent Studio panel for multi-agent workflows and live MCP actions.
- **MCP v2 Ergonomics**: Token binding by name or ID, headless text measurement (`measured_width`, `measured_height`, `line_count`, `clipped`), `fit_text: true`, and batch `create_frame`, `create_path`, `wrap_in_frame`, `create_directive`, `resolve_directive`, `delete_directive`, `select_layers`, `align_layers`, `distribute_layers`, and `duplicate_layers`.

Five native targets build and test before release publication. Download the matching installer or portable archive and verify it using `SHA256SUMS.txt`. macOS remains ad-hoc signed/not notarized; Windows is unsigned. See [agent interface](https://github.com/slyim/papercraft/blob/main/docs/agent-interface.md) and [live MCP documentation](https://github.com/slyim/papercraft/blob/main/docs/live-mcp.md).
