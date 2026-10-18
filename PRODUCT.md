# PaperCraft

<!-- impeccable:product-schema 1 -->

## Platform

Native desktop. macOS is the initial development and verification host; other platforms are unverified.

## Stack

User-selected: all application code in Rust, with egui for the interface. REA's existing Node-based tools remain development-only research tooling.

## Product Purpose

Create a free alternative to Paper for visual interface design. Use observed behavior to build an original local application. Preserve a path to agent-assisted design.

## Capabilities and Constraints

- Rust and egui own the application, including its document model and editor.
- Use Phosphor Icons. The native integration is `egui-phosphor`; no React runtime is required.
- Implemented behavior covers one artboard, text and rectangle nodes, layer controls, selection, drag/resize, a property inspector, undo/redo, local documents and HTML/SVG export. SVG retains explicit text lines; fonts and automatic wrapping differ between renderers.
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
