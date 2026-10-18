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
- Implemented behavior covers one artboard, text and rectangle nodes, layer controls, multi-selection with shared moves and align/distribute, named color tokens bound to layer, artboard and page colors, drag/resize, a property inspector, undo/redo, local documents and HTML/SVG export. SVG retains explicit text lines; fonts and automatic wrapping differ between renderers.
- Documents with color tokens use format version 2; documents without them remain version 1. See `docs/document-format.md`. Spacing, radius and typography tokens, theme modes, marquee selection, group resize and snapping are not implemented.
- Native egui painting is not a browser CSS engine. Layout fidelity, editable HTML/SVG import, vector/image layers, rich text, multiple artboards and MCP editing require later milestones.
- No backend service or account is required for the first local editing workflow.

## Brand Commitments

The name is PaperCraft. Paper is the explicit functional and interface reference. Use original code and content; preserve no Paper source, branding, fonts or assets in application code. User-facing messages are concise and factual.

## Evidence on Hand

The initial investigation is in `research/paper/analysis.md`. Its compact evidence index is `research/paper/evidence-summary.json`. The packaged shell is understood at a static level; the hosted editor's runtime behavior remains unverified. A screenshot of the public site's illustrative editor is retained only as private research material.

## Open Decisions

Audience beyond the requesting user, platform release targets, collaboration and detailed agent workflows remain undecided.

## Ecosystem and License

Original PaperCraft code, documentation and the generated paper-crane logo use MIT, an option also offered by the native Craft apps. PaperCraft remains independently maintained. The phased plan in `docs/artcraft.md` records SVG fixture imports into VectorCraft/FigCraft, the pinned macOS `craft-quit` integration, and proposed raster/automation work. No native-format adapter or launcher enrollment is implemented.
