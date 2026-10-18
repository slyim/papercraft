# PaperCraft gap assessment

18 October 2026. Baseline: PaperCraft commit `bc3c52bf75491510a7d5a7515a5dce5c0bb12f1d`, before this task's changes. Paper behavior is [vendor documented or explicitly observed](../research/paper/workflow-evidence.md), not inferred from screenshots. Craft source revisions and license/architecture findings are in [ArtCraft interoperability](artcraft.md). Results added during this task belong in [verification](verification.md); a selected contract is not proof that its checks passed.

## Existing capability and boundary

At the baseline, `src/document.rs` has one version-1 document, one artboard, ordered rectangle/text nodes, one selected node, validated JSON load/save, atomic replacement, snapshot history and HTML export. `src/app.rs` connects the layer list, inspector, visibility/locking/order, create/select/drag/resize, pan/zoom, dirty-state prompts and export. `src/recents.rs` preserves the original Crumbs storage location. These are implemented capabilities with [recorded checks and explicit manual/platform gaps](verification.md).

Partial parity: text has one plain style and a font-size field; layer order is flat; canvas geometry is absolute; HTML is bounded output rather than a live CSS renderer. Baseline contains no group hierarchy, image/vector nodes, multiple artboards, layout engine, token binding, CLI/MCP, PNG/PDF export or Craft-native-file import.

## Ranked work

Cost is relative to this repository: small = current model/UI only; medium = several connected behaviors; large = schema, rendering or infrastructure design. Impact is the consequence for a local authoring workflow, not a claim about measured demand.

| Priority / gap | Impact | Evidence | Cost / prerequisites | Acceptance boundary |
|---|---|---|---|---|
| 0. Preservation, history, input and transform defects | Critical: lost edits or incorrect output undermine every later feature | Existing dirty/commit/gesture/export paths; Paper's documented redo and Unicode fixes are context, not proof of a PaperCraft bug | Investigate before editing; focused reproduction required | Rejected loads/writes preserve current/saved data; focused text and complete pointer gestures form coherent undo steps; coordinate checks at multiple zooms. Only reproduced defects are reported as fixed. |
| 1. Bounded SVG export, selected slice | High: existing work gains a vector handoff without changing documents | Paper SVG docs and SVG-export build-log entry; VectorCraft/FigCraft format paths in [Craft record](artcraft.md) | Small/medium: reuse Document validation, current text layout where practical and atomic writes; actual target inspection required for compatibility claims | Existing primitives, order, alpha, radii, clipping, hidden-layer omission, Unicode/XML escaping; current font/wrapping limits stated. This adds exchange, not vector editing. |
| 2. Multi-selection and alignment, then grouping/hierarchy | High: move and arrange real compositions as units | Paper selection, frame, align and distribute shortcuts; Craft scene/selection models | Medium for multi-selection/alignment; large for hierarchy. Decide stable IDs, parent transforms, selection and one-batch undo before schema migration | Canvas/layers/inspector agree; group transforms preserve child geometry; old v1 files still load and rollback is explicit. |
| 3. Typography controls and text rendering | High: usable design text needs line-height, alignment, family/weight and reliable Unicode | Paper typography support/build log; current plain text and default egui font | Medium; confirm installed font licensing, shaping/fallback and export semantics before new fields | Same authored values survive history/save/reopen and export; differences in glyph metrics/fallback remain explicit. |
| 4. PNG export and image placement | High: immediate PhotoCraft/LightCraft and title-card workflows | Craft raster/still workflows; Paper HTML image import | Medium export, large placement; choose bounded image storage, decode limits, path policy and migration first | Artboard pixel size and alpha independent of viewport; replace image in one undo step; preserve placement on reopen. |
| 5. Snapping and guides | Medium/high: reliable alignment at different zooms | Paper pixel snapping and arrange controls | Medium; build on verified screen/document transform and selection | Document-unit geometry; screen-space tolerance; reversible toggle; no jump when zoom changes. |
| 6. Multiple artboards | Medium/high: screens and export sets | Paper multiple-frame PDF and artboard MCP context | Large; migrate one authoritative model and define coordinates, active artboard, hierarchy/history/export selection | Legacy documents become one artboard without data loss; combined/single export contracts tested. |
| 7. Flex layout, styling and tokens | Medium/high: responsive composition and repeatable styles | Paper CSS/flex and tokens documentation | Large; agree supported subset, intrinsic sizing and migration; no attempt at arbitrary browser CSS | Nested resize/reflow and shared style changes remain deterministic across canvas/history/persistence/export. |
| 8. CLI and local MCP | Medium: repeatable inspection/export and agent edits | Paper CLI/MCP plus actual Craft per-app command surfaces | Medium after reliable Editor transactions; start with inspect/export then bounded edit batches | UI/CLI use the same model; malformed commands leave state unchanged; protocol/file boundaries tested. |
| 9. PDF, rich vector editing and native-format adapters | Lower for the present slice | Craft conversion routes; Paper vector editing | Large; use SVG/PNG exchange and inspect actual targets before adopting another engine | State whether output is flattened/editable; never claim a document round trip from an image export. |

## Compatibility and reuse decisions

No format revision is needed for bounded SVG export or reliability fixes. Keep version 1, both document extensions, node IDs and existing recents. Future hierarchy/images/artboards require an explicit upgrade and rollback design before writing a new format.

Keep the existing authoritative Document/Editor and standard atomic save path. The Craft analysis may justify small shared crates when a concrete missing requirement exists; license compatibility alone is insufficient reason to add them. No Paper assets or recovered implementation are suitable for adoption. PaperCraft remains an independent MIT companion with original paper-crane branding.

Unverified until separately recorded: authenticated Paper edit/undo/save/export behavior, exact typography, accessibility with a real screen reader, large-document performance, and full Craft application interoperability. Read-only Paper connector access does not resolve these gaps.

## Outcome of this increment

Implemented: demonstrated UTF-8 duplication, gesture release/shortcut sequencing and minimized-close preservation fixes; pinned macOS Quit integration; HTML/SVG export choices and validated atomic SVG output. The committed SVG fixture was actually imported, inspected and rendered in installed VectorCraft 0.13.1 and FigCraft 0.7.0. The document schema and recent-history paths are unchanged.

Partial: SVG preserves explicit lines and editable plain text but does not reproduce native automatic wrapping or guarantee font/glyph metrics. PaperCraft's bundled native font still shows CJK placeholder glyphs in this fixture; text bytes survive save/export and target import. Hierarchy, alignment, richer typography, image placement, multiple artboards, layout and automation remain proposals in the ranked table. See [verification](verification.md) for checked behavior and remaining limits.
