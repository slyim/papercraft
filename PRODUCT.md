# PaperCraft

<!-- impeccable:product-schema 1 -->

## Platform

Native desktop. Release builds and automated Rust tests cover Linux x86_64/ARM64, macOS Apple Silicon/Intel and Windows x86_64. Native renderer checks cover Linux and macOS; Windows native-window interaction and physical-input checks remain unverified. See `docs/releasing.md` and `docs/verification.md`.

## Stack

User-selected: all application code in Rust, with egui for the interface. REA's existing Node-based tools remain development-only research tooling.

## Product Purpose

Create a free alternative to Paper for visual interface design. Use observed behavior to build an original local application. Preserve a path to agent-assisted design.

## Capabilities and Constraints

- Rust and egui own the application, including its document model and editor.
- Use Phosphor Icons. The native integration is `egui-phosphor`; no React runtime is required.
- Implemented behavior covers one artboard, nested Frame containers (`NodeKind::Frame` with `parent` and `clip`), swappable shape nodes (`rectangle`, `ellipse`, `triangle`), vector path nodes (`NodeKind::Path` with normalized `points` and SVG path parsing), procedural shader fills (`LinearGradient`, `RadialGradient`, `Mesh`, `Grain`, `Halftone`), strokes (`stroke`, `stroke_token`, `stroke_width`), text nodes, independent document tabs, inline text and layer-name editing, text line height and alignment, layer controls, marquee/multi-selection, shared moves and align/distribute, named color tokens bound to layer fill/stroke, artboard and page colors, eight-way and group resize, edge/center snapping with guides, a right-hand dock with **Inspect**, **Directives** (spatial agent pins), and **Agent** (ACP Agent Studio), undo/redo, local documents, HTML/SVG/PNG export, and a local/live MCP server. SVG retains explicit text lines; fonts and automatic wrapping differ between renderers.
- Documents with color tokens use format version 2; documents with line height, text alignment, ellipse/triangle shapes, frames, paths, strokes, shaders or directives use version 3; documents without them remain version 1. See `docs/document-format.md` and `docs/format-v3.md`. Spacing, radius and typography tokens, theme modes, and equal-gap/pixel snapping remain unimplemented.
- Native egui painting is not a browser CSS engine. Layout fidelity, editable HTML/SVG import, vector/image layers, richer typography, multiple artboards and CLI/MCP editing require later milestones. Current source parity and agent-ready issues are tracked in `docs/gap-assessment.md`; download capabilities are bounded by their release revision.
- No backend service or account is required for the first local editing workflow.

## Brand Commitments

The name is PaperCraft. Paper is the explicit functional and interface reference. Use original code and content; preserve no Paper source, branding, fonts or assets in application code. User-facing messages are concise and factual.

## Evidence on Hand

The initial investigation is in `research/paper/analysis.md`. Its compact evidence index is `research/paper/evidence-summary.json`. The packaged shell is understood at a static level; the hosted editor's runtime behavior remains unverified. A screenshot of the public site's illustrative editor is retained only as private research material.

## Open Decisions

Audience beyond the requesting user, collaboration and detailed agent workflows remain undecided. The release targets are Linux x86_64/ARM64, macOS Apple Silicon/Intel and Windows x86_64.

## Ecosystem and License

Original PaperCraft code, documentation and the generated paper-crane logo use MIT, an option also offered by the native Craft apps. PaperCraft remains independently maintained. The phased plan in `docs/artcraft.md` records SVG fixture imports into VectorCraft/FigCraft, the pinned macOS `craft-quit` integration, and proposed raster/automation work. No native-format adapter or launcher enrollment is implemented.
