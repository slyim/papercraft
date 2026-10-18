# Tool rail plan

18 October 2026, Europe/Istanbul. Proposed sequence for the full tool rail. Nothing here is implemented beyond what [implementation process](implementation-process.md) and [verification](verification.md) already record. Each milestone follows the change workflow in the implementation process: evidence, behavior contract, original implementation, verification, stop.

## Target rail

The target is a user-supplied screenshot of Paper's tool strip (18 October 2026), with three groups. The screenshot shows icons only; it carries no labels, tooltips or behavior.

| # | Icon (observed) | Identification | Basis | Paper shortcut (D3) | PaperCraft today | Phosphor glyph |
|---|---|---|---|---|---|---|
| 1 | Arrow cursor | Select / Move | Documented tool + icon | V | Implemented | `CURSOR` |
| 2 | Hand | Hand / pan | Documented tool + icon | H, Space held | Implemented (H, Space, middle button) | `HAND` |
| 3 | Dashed corners | Frame | Documented tool + icon | F; ⇧F wraps selection | Missing | `FRAME_CORNERS` |
| 4 | Square | Rectangle | Documented tool + icon | R | Implemented | `SQUARE` |
| 5 | Pen nib | Pen | Documented tool + icon | P | Missing | `PEN_NIB` |
| 6 | "Aa" | Text | Documented tool + icon | T | Implemented | `TEXT_AA` |
| 7 | Speech shape with plus | Comment | **Inference** from shape; comments documented in build log | C | Missing | `CHAT_TEARDROP` (+ plus overlay) |
| 8 | Diamond in square | Shaders | **Inference**; shaders menu documented | S | Missing | `DIAMOND` |
| 9 | Image with sparkle | Create image (AI) | **Inference**; panel documented | ⇧⌘I | Missing | `IMAGE` + `SPARKLE` overlay |
| 10 | Frame with sparkle | Create SVG (AI) | **Inference**; panel documented | ⇧⌘J | Missing | `BOUNDING_BOX` + `SPARKLE` overlay |

**Unknown:** identities 7–10 are not confirmed. Hovering each button in the installed Paper app to read its tooltip would settle them; that is a runtime observation to record before milestone T6.

D3 is a refetch of the [support](https://paper.design/docs/support), [SVG](https://paper.design/docs/svg) and [build log](https://paper.design/build-log) pages on the date above, all HTTP 200. Paper's docs disagree on the Move shortcut (support: V; SVG cheatsheet: M); PaperCraft keeps V. The docs contain no Components or "insert" tool (build log: "coming soon"), no ellipse/line tool and no toolbar ordering.

## Documented behavior per tool (D3) and gaps

| Tool | Documented | Undocumented, so a PaperCraft decision |
|---|---|---|
| Select | ⌘-click deep select; Enter selects children / edits text / edits path; Esc selects parent; ⌘A siblings; Tab cycles siblings; ⌥ hover shows distances; ⌥-drag duplicates; double-click handle fits content | Marquee rules, hit order inside frames |
| Hand | H persists; Space is temporary; arrows pan with no selection | — |
| Frame | F adds; ⇧F wraps selection; clip toggle ⌥C; constraints panel; flex via ⇧A | Click vs drag, default size, nesting when drawn inside a frame |
| Rectangle | R; per-corner radius | Default fill, ⇧/⌥ while drawing |
| Pen | Click to start a path; inside an SVG adds to it, outside creates a container; Enter/double-click edits; Esc or empty double-click exits; add/delete/move nodes, add curvature; half-pixel snapping | Handle-drag gesture, close-path gesture, modifiers |
| Text | T; Enter edits; new layers reuse last text style; weight/size/line-height/spacing/align shortcuts | Click (auto width) vs drag (fixed width) |
| Comment | C or button | Storage, threading, anchoring |
| Shaders | S opens a menu of named effects; presets; PNG copy; video export | Placement gesture, menu contents |
| Create image / SVG | Prompt panels backed by hosted models; selection as context | Everything local |

## Shared prerequisites

These are model changes several tools need. They come before the tools that depend on them.

- **P1 Tool registry.** Replace the hard-coded rail groups in `src/app.rs` (`tool_rail`) with one table: tool, glyph, label, shortcut, enabled state, creation kind. Shortcut handling and the rail read the same table. Unimplemented tools are absent, not shown disabled, so the rail never advertises a missing feature.
- **P2 Hierarchy (format version 3).** Nodes gain an optional parent. Frames own ordered children; child geometry is relative to the parent. Needs upgrade from v1/v2 (flat list becomes children of the artboard), refusal by older readers, hit testing and paint order through the tree, and one-step undo for reparenting. Already ranked first in the [gap assessment](gap-assessment.md).
- **P3 Strokes.** Pen paths need stroke color/width; add it to rectangles and frames at the same time so the inspector stays uniform. Color tokens bind strokes like fills.
- **P4 Image nodes and PNG export.** Decoded size limits, embedded or sidecar storage, missing-file behavior. Needed by Create image and by shader export to HTML/SVG.

## Milestones

| Milestone | Scope | Depends on | Cost | Acceptance |
|---|---|---|---|---|
| T1 Rail and core-tool refinements (implemented, see [verification](verification.md#tool-rail-t1-18-october-2026)) | P1; Rectangle ⇧ square and ⌥ from center; Text click = auto width, drag = fixed width, Enter to edit on canvas, last-used style; Select marquee | — | Small | Shortcuts and rail stay in sync; each creation is one undo step at multiple zooms; marquee selects only visible unlocked layers |
| T2 Frame | P2; `NodeKind::Frame` with fill, radius, clip; click places default size, drag sizes; drawing inside a frame nests the result; ⇧F wraps selection; Enter/Esc/⌘-click navigation; layer list becomes a tree | P2 | Large | v1/v2 files open unchanged after upgrade; moving a frame moves children; clipping matches in canvas, HTML and SVG; wrap/unwrap is one undo step |
| T3 Pen | P3; `NodeKind::Path` with points (position, in/out handles, corner/smooth) and open/closed flag; click adds corner, drag pulls symmetric handles, click first point closes, Enter/Esc finishes; edit mode moves, adds and deletes points; tessellate via epaint; SVG `path d`, HTML inline `<svg>` | P3 (P2 for SVG containers) | Large | Path survives save/reopen exactly; drawing is one undo step, each edit-mode drag one step; hit test on stroke and fill; exported SVG renders the same outline |
| T4 Comments | Pins anchored to a layer or canvas point, text body, resolved flag; side list; not exported; no accounts or sync | P2 for anchoring to children | Medium | Pins follow anchored layers; deleting a layer keeps or orphans pins by a stated rule; comments do not enter HTML/SVG |
| T5 Images | P4; upload/place image (⇧⌘K and drag-drop), fill/fit/crop modes, PNG export of the artboard | P2 | Large | Pixel size independent of zoom; replace is one undo step; missing source reported without losing the layer |
| T6 Shaders | Shader fill on rectangles/frames: original procedural effects (start with linear/radial/mesh gradient and grain) with parameters in the inspector; render through an eframe wgpu callback; rasterize for HTML/SVG/PNG | T5 (raster export), runtime tooltip check | Large | Same parameters render the same image across reopen; export embeds a raster at artboard resolution; no third-party shader code without a license check |
| T7 Create image / Create SVG | Prompt panel; Create SVG output goes through T3 path nodes, Create image through T5 image nodes | T3, T5, **scope decision** | Medium after decision | Generated output is ordinary editable layers; failed or cancelled requests change nothing; one undo step per insertion |

Order: T1 → T2 → T3 → T5 → T4 → T6 → T7. T4 can run in parallel with T3 once P2 lands, since it touches separate model fields.

## Decisions needed

1. **Generation services (T7).** The current constraints exclude cloud accounts and generation services. Options: keep the two AI buttons out of the rail; add a user-supplied provider key (network, credential storage, cost); or call a local model. Until decided, the rail omits them and SVG import plus image placement cover the non-AI part.
2. **Shader scope (T6).** Which effects, and whether video export is in scope.
3. **Comments (T4).** Local-only notes are the only form that fits current constraints; confirm that is useful without collaboration.

Paper source, shader code, icons and branding are not used. Glyphs come from `egui-phosphor` 0.14.0; the sparkle and plus overlays are composed from Phosphor glyphs.
