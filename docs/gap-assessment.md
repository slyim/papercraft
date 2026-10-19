# PaperCraft gap assessment

## Current parity — 19 October 2026

Reviewed source: `6065d02` on `main`. Published v0.1.2: `40765dfc92fb85e721dda0c4ecbc2e4875bb2cd7`, 12 commits behind that source. The downloads do **not** include the later tool/tab, canvas/snapping and combined-integration work. This review changes documentation and removes an implementation-only test gate; it does not add editor features.

Paper comparison uses the retained [workflow evidence](../research/paper/workflow-evidence.md): E1 describes the packaged shell, D1–D3 are vendor documentation, and R1 is read-only connector observation. No authenticated Paper edit/undo/export comparison was performed. A full-parity percentage would not be supportable.

| Area | Current source capability | Remaining boundary |
|---|---|---|
| Documents and preservation | Independent tabs retain document/history/selection/path/view/text style; existing-file activation, dirty-close/Quit decisions, validated atomic writes; legacy `.crumbs` support | One artboard and a flat layer list per document, not multiple pages/artboards |
| Core tools and text | V/H/R/T registry; constrained/centered rectangle creation; click/drag text placement, inline editing and layer renaming | Plain text/font size/color only; no authored family, weight, line-height or alignment; glyph coverage/IME remain limited or unverified |
| Selection and geometry | Multi-selection, marquee, shared move/nudge, align/distribute, eight-way/group resize and aspect lock | Group resize is temporary selection geometry, not persistent groups or parent/child transforms |
| Snapping | Artboard/layer edge-center matches, guides, screen-space tolerance, toggle and bypass | No creation snapping, equal-gap guides or pixel grid; exact Paper heuristics unknown |
| Theme | Named color tokens, bindings, propagation, detach/delete preservation, undo/redo; conditional v1/v2 persistence | No other token types, theme modes or token duplication |
| Exchange | HTML and SVG export from the authoritative model; bounded SVG fixture exchange previously checked in VectorCraft/FigCraft | No PNG/PDF export or image/path nodes/import; SVG explicit lines do not reproduce automatic wrapping or font metrics |
| Layout and styling | Absolute geometry, fill, uniform rectangle radius | No frames/flex, rotation/flip, strokes, per-corner radii, blend/effects or reusable components |
| Agent interface | GUI document-path opening | No headless inspection/export, live edit protocol or local MCP |

Source anchors: [`Document`/`Node` and exporters](../src/document.rs), [tabs, tools, canvas and inline editing](../src/app.rs), [resize/snapping geometry](../src/snap.rs). Behavioral coverage is in [`tests/tools.rs`](../tests/tools.rs), [`tests/canvas.rs`](../tests/canvas.rs), [`tests/arrangement.rs`](../tests/arrangement.rs) and the tab checks in `src/app.rs`.

Current macOS checks and limits are in [the review verification record](verification.md#parity-and-branch-review-19-october-2026). A successful native `text` invocation did not visibly exercise text placement, so it is not counted as new proof of native text editing.

Confirmed reliability defects are filed, **not fixed by this review**: cross-tab inline undo imports another document's text ([#12](https://github.com/slyim/papercraft/issues/12)); valid-length text is discarded when automatic geometry exceeds the limit ([#10](https://github.com/slyim/papercraft/issues/10)); deleted inherited token IDs can bind new text to an unrelated replacement ([#11](https://github.com/slyim/papercraft/issues/11)). All three were reproduced through real app/model paths with simulated egui input. Prioritize these before new feature work and the next preview.

### Agent-ready backlog

Each issue records evidence, entry points, scope/non-goals, dependencies and observable acceptance criteria. These are proposed improvements, **not implemented features**.

| Priority | Issue | Ownership / dependency |
|---|---|---|
| P1 | [#12 Isolate document-local inline undo state](https://github.com/slyim/papercraft/issues/12) | Stable per-document widget identity; no schema change |
| P1 | [#10 Preserve text at automatic geometry limits](https://github.com/slyim/papercraft/issues/10) | Bounded layout and one-step history; no validation suppression |
| P2 | [#11 Detach deleted inherited token bindings](https://github.com/slyim/papercraft/issues/11) | Preserve literal color through token deletion/history/ID reuse |
| P1 | [#1 Native scenarios must prove their interaction](https://github.com/slyim/papercraft/issues/1) | Verification example; reject unknown names, establish text/marquee state |
| P1 | [#2 Automatic PR/main Rust gates](https://github.com/slyim/papercraft/issues/2) | CI; current workflow is manual release-only |
| P1 | [#3 Publish the integrated editor preview](https://github.com/slyim/papercraft/issues/3) | After #1/#2 and #10/#11/#12; existing five-platform packaging, no updater |
| P1 | [#5 Artboard PNG export](https://github.com/slyim/papercraft/issues/5) | Existing primitives only; no schema or image-node dependency |
| P1 | [#7 Nested frames and persistent grouping](https://github.com/slyim/papercraft/issues/7) | Own next schema/transform/upgrade contract; no flex or scene-engine rewrite |
| P1 | [#9 Plain-text line-height and alignment](https://github.com/slyim/papercraft/issues/9) | Coordinate strict-schema changes with #7; no rich text/font-picker expansion |
| P2 | [#4 Headless inspect and HTML/SVG export](https://github.com/slyim/papercraft/issues/4) | Reuse `Document`; no live mutation, daemon or MCP |
| P2 | [#6 Creation snapping](https://github.com/slyim/papercraft/issues/6) | Reuse `Snapper`; no equal-gap or pixel-grid expansion |
| P2 | [#8 Portable local image placement](https://github.com/slyim/papercraft/issues/8) | After #7 and #5 storage/raster contracts; no network/AI |

PNG, CLI, creation snapping and CI can be developed independently, with one integration owner for shared export/UI files. Frame, typography and image work must not independently choose conflicting format versions. Preserve one authoritative `Document`/`Editor`, strict validation, transactional history and atomic output.

Deferred beyond this queue: licensed-font/fallback work, non-color tokens, flex, editable vectors/import, multiple artboards, PDF and live MCP. Comments, shaders and generation retain the evidence/scope prerequisites in the [tool rail plan](toolbar-plan.md). No cloud or collaboration expansion is authorized by this assessment.

## Historical baseline and increments — 18 October 2026

The dated sections below retain the original baseline, rankings and outcomes. They are not the current missing-feature list; the snapshot and linked issues above supersede those stage-specific statements.

18 October 2026. Baseline: PaperCraft commit `bc3c52bf75491510a7d5a7515a5dce5c0bb12f1d`, before this task's changes. Paper behavior is [vendor documented or explicitly observed](../research/paper/workflow-evidence.md), not inferred from screenshots. Craft source revisions and license/architecture findings are in [ArtCraft interoperability](artcraft.md). Results added during this task belong in [verification](verification.md); a selected contract is not proof that its checks passed.

### Existing capability and boundary

At the baseline, `src/document.rs` has one version-1 document, one artboard, ordered rectangle/text nodes, one selected node, validated JSON load/save, atomic replacement, snapshot history and HTML export. `src/app.rs` connects the layer list, inspector, visibility/locking/order, create/select/drag/resize, pan/zoom, dirty-state prompts and export. `src/recents.rs` preserves the original Crumbs storage location. These are implemented capabilities with [recorded checks and explicit manual/platform gaps](verification.md).

Partial parity: text has one plain style and a font-size field; layer order is flat; canvas geometry is absolute; HTML is bounded output rather than a live CSS renderer. Baseline contains no group hierarchy, image/vector nodes, multiple artboards, layout engine, token binding, CLI/MCP, PNG/PDF export or Craft-native-file import.

### Ranked work

Cost is relative to this repository: small = current model/UI only; medium = several connected behaviors; large = schema, rendering or infrastructure design. Impact is the consequence for a local authoring workflow, not a claim about measured demand.

| Priority / gap | Impact | Evidence | Cost / prerequisites | Acceptance boundary |
|---|---|---|---|---|
| 0. Preservation, history, input and transform defects | Critical: lost edits or incorrect output undermine every later feature | Existing dirty/commit/gesture/export paths; Paper's documented redo and Unicode fixes are context, not proof of a PaperCraft bug | Investigate before editing; focused reproduction required | Rejected loads/writes preserve current/saved data; focused text and complete pointer gestures form coherent undo steps; coordinate checks at multiple zooms. Only reproduced defects are reported as fixed. |
| 1. Bounded SVG export, selected slice | High: existing work gains a vector handoff without changing documents | Paper SVG docs and SVG-export build-log entry; VectorCraft/FigCraft format paths in [Craft record](artcraft.md) | Small/medium: reuse Document validation, current text layout where practical and atomic writes; actual target inspection required for compatibility claims | Existing primitives, order, alpha, radii, clipping, hidden-layer omission, Unicode/XML escaping; current font/wrapping limits stated. This adds exchange, not vector editing. |
| 2. Multi-selection and alignment (implemented, see outcome below), then grouping/hierarchy | High: move and arrange real compositions as units | Paper selection, frame, align and distribute shortcuts; Craft scene/selection models | Medium for multi-selection/alignment; large for hierarchy. Decide stable IDs, parent transforms, selection and one-batch undo before schema migration | Canvas/layers/inspector agree; group transforms preserve child geometry; old v1 files still load and rollback is explicit. |
| 3. Typography controls and text rendering | High: usable design text needs line-height, alignment, family/weight and reliable Unicode | Paper typography support/build log; current plain text and default egui font | Medium; confirm installed font licensing, shaping/fallback and export semantics before new fields | Same authored values survive history/save/reopen and export; differences in glyph metrics/fallback remain explicit. |
| 4. PNG export and image placement | High: immediate PhotoCraft/LightCraft and title-card workflows | Craft raster/still workflows; Paper HTML image import | Medium export, large placement; choose bounded image storage, decode limits, path policy and migration first | Artboard pixel size and alpha independent of viewport; replace image in one undo step; preserve placement on reopen. |
| 5. Snapping and guides | Medium/high: reliable alignment at different zooms | Paper pixel snapping and arrange controls | Medium; build on verified screen/document transform and selection | Document-unit geometry; screen-space tolerance; reversible toggle; no jump when zoom changes. |
| 6. Multiple artboards | Medium/high: screens and export sets | Paper multiple-frame PDF and artboard MCP context | Large; migrate one authoritative model and define coordinates, active artboard, hierarchy/history/export selection | Legacy documents become one artboard without data loss; combined/single export contracts tested. |
| 7. Flex layout, styling and non-color tokens (color tokens implemented) | Medium/high: responsive composition and repeatable styles | Paper CSS/flex and tokens documentation | Large; agree supported subset, intrinsic sizing and migration; no attempt at arbitrary browser CSS | Nested resize/reflow and shared style changes remain deterministic across canvas/history/persistence/export. |
| 8. CLI and local MCP | Medium: repeatable inspection/export and agent edits | Paper CLI/MCP plus actual Craft per-app command surfaces | Medium after reliable Editor transactions; start with inspect/export then bounded edit batches | UI/CLI use the same model; malformed commands leave state unchanged; protocol/file boundaries tested. |
| 9. PDF, rich vector editing and native-format adapters | Lower for the present slice | Craft conversion routes; Paper vector editing | Large; use SVG/PNG exchange and inspect actual targets before adopting another engine | State whether output is flattened/editable; never claim a document round trip from an image export. |

### Compatibility and reuse decisions

No format revision is needed for bounded SVG export or reliability fixes. Keep version 1, both document extensions, node IDs and existing recents. Future hierarchy/images/artboards require an explicit upgrade and rollback design before writing a new format.

Keep the existing authoritative Document/Editor and standard atomic save path. The Craft analysis may justify small shared crates when a concrete missing requirement exists; license compatibility alone is insufficient reason to add them. No Paper assets or recovered implementation are suitable for adoption. PaperCraft remains an independent MIT companion with original paper-crane branding.

Unverified until separately recorded: authenticated Paper edit/undo/save/export behavior, exact typography, accessibility with a real screen reader, large-document performance, and full Craft application interoperability. Read-only Paper connector access does not resolve these gaps.

### Outcome of this increment

Implemented: demonstrated UTF-8 duplication, gesture release/shortcut sequencing and minimized-close preservation fixes; pinned macOS Quit integration; HTML/SVG export choices and validated atomic SVG output. The committed SVG fixture was actually imported, inspected and rendered in installed VectorCraft 0.13.1 and FigCraft 0.7.0. The document schema and recent-history paths are unchanged.

Partial: SVG preserves explicit lines and editable plain text but does not reproduce native automatic wrapping or guarantee font/glyph metrics. PaperCraft's bundled native font still shows CJK placeholder glyphs in this fixture; text bytes survive save/export and target import. Hierarchy, alignment, richer typography, image placement, multiple artboards, layout and automation remain proposals in the ranked table. See [verification](verification.md) for checked behavior and remaining limits.

### Outcome of the token and arrangement increment

Implemented: color tokens end to end (Theme tab, inspector binding/detach/create-from-color, propagation, deletion that preserves colors, history, version 2 persistence with an upgrade prompt, HTML custom properties and resolved SVG colors). Also implemented: multi-selection from the canvas and layer list, shared drag, nudge, select-all, align and distribute with defined reference bounds, and hidden/locked exclusion. One defect was found and fixed during native inspection: a bound token chip overflowed and widened the inspector. Evidence and checks are in [verification](verification.md#color-tokens-and-arrangement-18-october-2026).

Remaining, re-ranked:

1. Grouping/hierarchy. Requires a schema and transform design.
2. Marquee selection and multi-layer resize.
3. Snapping/guides, building on the shared bounds now available.
4. Typography controls, then typography/spacing/radius tokens. These need new node fields, so add them only with those properties.
5. Theme modes. Paper lists these as roadmap.

PNG export, multiple artboards, layout and CLI/MCP keep their earlier ranks.

### Outcome of the canvas and performance increment

Implemented: marquee selection, eight-way and group resize, aspect-locked corner resize, and edge/center snapping with guides (remaining items 2 and 3 above). Per-frame work no longer grows with layers × selection; measurements are in [verification](verification.md#canvas-and-performance-18-october-2026). The document format is unchanged.

Remaining, re-ranked:

1. Grouping/hierarchy. Requires a schema and transform design.
2. Typography controls, then typography/spacing/radius tokens.
3. Snapping during layer creation, spacing (equal-gap) guides and a pixel grid. Paper's snapping heuristics remain undocumented.
4. Theme modes.
