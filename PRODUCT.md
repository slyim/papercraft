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
- The first benchmark is an externally agent-built multi-screen interface, not illustration parity. Retain the native interface direction, Document/Editor model and live MCP architecture; no browser-engine migration.
- Working bounded workflows include rectangle/plain-text editing, independent tabs, tokens, arrangement, geometry-aware marquee/eight-way resize, creation/move/resize snapping, history and atomic local files. The canvas supports off-artboard geometry, shape/straight-stroke selection with transparent-fill rules, tab-local fit/anchored zoom/press-owned pan and cancellable geometry drafts. Existing top-level Frame nodes support screen composition; parent coordinates remain absolute. Frame hierarchy/movement/clipping semantics, strokes, procedural fills and directives remain partial.
- External headless/live MCP provides atomic batches, typed batch-local references, revisions, bounds/text measurements, bounded region/frame PNG rendering, refinement and save. The **Agent** dock is local notes and presets, **not an ACP/provider runner**. Core authoring does not require toolbar simulation.
- Paths currently store normalized straight vertices. The legacy Pen entry is labeled **Path preset**, not an authored contour tool. Relative/curve/multiple-subpath SVG input rejects; authored Pen/point/Bézier editing is later work. HTML/SVG are artboard-bounded handoff outputs, not arbitrary CSS or a production app; shaders and ancestor clipping reject instead of silently approximating. SVG keeps explicit text lines; fonts and wrapping differ.
- Writers use the lowest representable native format. v0.2.0 and v0.3.0 shipped different v3 subsets; preserve the actual v0.3.0 superset and legacy .crumbs/.papercraft files without coordinate reinterpretation. This workspace increment adds no native schema fields/version. Future persisted changes require v4 or later. See `docs/document-format.md` and `docs/format-v3.md`.
- Native egui painting is not a browser CSS engine. Selection uses bounded native contours and text layout boxes, not glyph/curve/pixel accuracy; closed path interiors are convex-only. Full hierarchy, richer text/appearance, authored vectors, embedded images, layout, components and pages remain incomplete or missing. The single current roadmap and source-backed inventory is `docs/editor-completion-plan.md`; older gap/release claims are historical.
- No backend service or account is required for the first local editing workflow.

## Brand Commitments

The name is PaperCraft. Paper is the explicit functional and interface reference. Use original code and content; preserve no Paper source, branding, fonts or assets in application code. User-facing messages are concise and factual.

## Evidence on Hand

The initial investigation is in `research/paper/analysis.md`. Its compact evidence index is `research/paper/evidence-summary.json`. The packaged shell is understood at a static level; the hosted editor's runtime behavior remains unverified. A screenshot of the public site's illustrative editor is retained only as private research material.

## Open Decisions

External MCP clients come first; embedded agent/provider execution is separate scope. No cloud accounts, paid generation or collaboration services. Paper/Pencil inform documented behavior; FigCraft code is only selectively reusable after revision pinning, owner/test/dependency review and license/asset checks. The release targets remain Linux x86_64/ARM64, macOS Apple Silicon/Intel and Windows x86_64.

## Ecosystem and License

Original PaperCraft code, documentation and the generated paper-crane logo use MIT, an option also offered by the native Craft apps. PaperCraft remains independently maintained. The phased plan in `docs/artcraft.md` records SVG fixture imports into VectorCraft/FigCraft, the pinned macOS `craft-quit` integration, and proposed raster/automation work. No native-format adapter or launcher enrollment is implemented.
