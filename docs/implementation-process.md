# PaperCraft implementation process

Date: 18 October 2026, Europe/Istanbul.

Build an original, local design editor in Rust with egui. Paper research informs behavior; it does not supply application code. The first slice is one local artboard, rectangle/text layers, selection and ordering, an inspector, drag/resize, pan/zoom, undo/redo, `.papercraft` JSON save/open (also accepting legacy `.crumbs` files), and HTML export. This document defines acceptance checks, not a claim that they have passed.

The first slice is implemented. The [verification record](verification.md) lists completed checks and remaining manual/platform checks separately. The milestones below remain the acceptance checklist for further work.

The current increment also implements bounded SVG export and the reliability corrections described below. The [gap assessment](gap-assessment.md) ranks remaining work; the [Craft source review](craft-source-review.md) and [Paper workflow evidence](../research/paper/workflow-evidence.md) separate source/documentation claims from runtime checks.

## Constraints

- All application logic and UI use Rust and egui. Existing npm/REA tooling remains research tooling.
- Use Phosphor Icons through upstream assets or a verified Rust integration. `@phosphor-icons/react` identifies the official React package; it is not an application dependency for this Rust UI. Retain Phosphor's [MIT notice](https://github.com/phosphor-icons/react/blob/master/LICENSE).
- Use original code, names, styling and document format. Do not import Paper source, branding, fonts or application assets into the product.
- Keep raw Paper evidence in ignored `research/private/`. Keep research claims separate from implementation decisions and test results.
- No cloud accounts, collaboration, generation services or monetization in this slice.
- egui rendering is not a browser layout engine. HTML export is a bounded output format; HTML/CSS editing, import round trips and Paper rendering parity are not promised.

## Evidence baseline

The saved [analysis](../research/paper/analysis.md) and [compact evidence](../research/paper/evidence-summary.json) describe Paper `0.5.16`, build `261015tx3mukqib`, inspected with REA `6.7.0`.

| Reference | Identity and boundary |
|---|---|
| E1 | Evidence ID `ev_1c175b88e83c79b3e8dcd0af6431ff71727231ad23616097a7247f60b36ef3f2` |
| Artifact | `/Applications/Paper.app/Contents/Resources/app.asar`; SHA-256 `912bad3fe41e226c67fd4e27352e2b4456d69d19589f4583cba5944932fb78b2` |
| Coverage | 60 selected files; 4,627 inventoried files unselected; graph coverage partial; no application code executed |
| Integrity | Unpacked `dist/cli-bin/darwin-arm64/paper` differs from ASAR-declared integrity metadata; its bytes remain `observed-untrusted` |

**Observed statically:** the desktop shell loads a hosted editor and forwards MCP calls to renderer handlers. E1 source paths: `src/window/paper-tab.ts:152–161,338`, `src/lib/resolve-environment.ts:1–26`, `src/mcp/server.ts:46–70`, `src/mcp/bridge.ts:193–250`.

**Inference:** an independent alternative needs its own model, editing behavior and persistence; reproducing the shell would not provide an independent editor.

**Unknown:** authenticated editor interactions, document format, undo batching, export fidelity, offline editing and performance. No runtime equivalence follows from E1.

## Feature contracts

| Area | Evidence status | First-slice contract and remaining gap |
|---|---|---|
| Artboard, rectangles, text | [Vendor documented](https://paper.design/docs/support); runtime unverified | Create and edit these primitives in an original model. Nested frames, rich text, images and vectors deferred. |
| Layers and inspector | Selection, ordering, position and sizing [documented](https://paper.design/docs/support) | Canvas/layer-list selection agrees; inspector edits the selected layer; ordering determines drawing and hit testing. Paper's exact selection rules remain unknown. |
| Drag/resize, pan/zoom | [Documented](https://paper.design/docs/support); runtime unverified | Pointer movement maps correctly between screen and artboard coordinates. Define zoom limits and minimum size locally. Paper transform/snapping rules remain unknown. |
| Undo/redo | Present in public [build-log references](https://paper.design/build-log); batching unknown | One completed drag/resize is one history step; redo restores the result; a new edit clears redo. This is a PaperCraft decision. |
| Save/open | E1 `src/lib/storage.ts:12–24,33–47,76–126` proves shell JSON storage only | Versioned `.papercraft` JSON stores the original document; legacy `.crumbs` files remain accepted. Validate before replacing the open document; failed writes preserve existing files. Paper document compatibility is unknown and out of scope. |
| HTML export | Code export [documented](https://paper.design/docs/support) | Export supported primitives, geometry, colors and escaped text as a standalone file. Browser typography may differ from egui; arbitrary CSS and import round trips deferred. |
| SVG exchange | Paper's [SVG documentation](https://paper.design/docs/svg) and build log, plus inspected VectorCraft/FigCraft importers | Implemented: export the current artboard background and visible rectangle/text nodes, preserving order, alpha, clamped radii, explicit lines and clipping. Export button/file menu offer HTML and SVG. Reuse validation and atomic writes. XML-incompatible controls reject export before writing; fonts and automatic wrapping differ. Actual fixture imports are recorded separately in verification. |
| Dashboard and editor chrome | User-supplied Paper screenshots (dashboard, empty editor), 18 October 2026; runtime behavior behind account/team/MCP controls unknown | PaperCraft decisions: a Dashboard tab and one document tab; recents store local paths only and tolerate missing or invalid lists; opening, creating or closing via tabs/cards uses the unsaved-changes prompt; V/H/R/T tools place a layer by click or drag inside the artboard, as one history step, then return to Select; the page color is stored in the document (`canvas`, optional for earlier version 1 files) and exported as the HTML body background. Multiple pages are shown disabled (the Theme tab became functional in the token increment below); account, sharing, MCP, vector, frame, component and image-generation controls are omitted. |
| Color tokens | Paper [tokens docs](https://paper.design/docs/tokens) and build log, D1/D2 in [workflow evidence](../research/paper/workflow-evidence.md); token deletion semantics, undo and page/artboard binding are undocumented | Implemented: named color tokens with stable IDs; bind/detach layer fill/text color, artboard and page; propagation; delete detaches and keeps colors; one undo step per operation; format version 2 written only when tokens exist; HTML custom properties and SVG resolved colors. Other token types and theme modes deferred. |
| Multi-selection and arrangement | Paper align/distribute shortcuts documented (D1/D2); multi-select modifiers, reference bounds and hidden/locked behavior undocumented | Implemented PaperCraft decisions: Shift-click (canvas/list), ⌘-click (list), shared drag, ⌘A, ⌥ align, ⌃⌥ distribute, hidden/locked excluded from arrangement, one undo step per operation. Marquee, group resize, hierarchy and snapping deferred. |
| Agent interface | E1 bridge sources above; [public MCP behavior](https://paper.design/docs/mcp) | Later milestone only. First establish one authoritative document model; protocol, mutation batching and screenshot semantics remain undecided. |

## Change workflow

1. **Evidence:** reuse the saved baseline. For a new Paper claim, record observation/inference/unknown, target hash, Evidence ID, exact source or public URL, coverage and limitations. Use the project REA skill only when shipped-artifact or requested runtime evidence is needed.
2. **Behavior contract:** write the trigger, visible result, state change, invalid-input behavior and acceptance check. Identify whether each rule comes from evidence or a PaperCraft design decision. Resolve only uncertainties that affect the current slice.
3. **Original implementation:** implement the smallest complete Rust change against the existing model. Canvas, inspector, history, persistence and export must use the same state. Never translate recovered Paper source into product code.
4. **Verification:** exercise the contract with focused model tests and a rendered native-app check. Record commands/results and untested behavior separately. A successful build does not establish interaction correctness.
5. **Stop:** complete the listed acceptance checks, document remaining gaps, and stop expanding scope. Add a later feature only with a new bounded contract.

## Milestones and acceptance checks

| Milestone | Acceptance checks |
|---|---|
| 1. Document and basic editor | Create rectangle/text; select from canvas and layer list; inspect/edit values; reorder overlapping layers; confirm rendering and hit testing follow the same order. Delete selected layers without retaining invalid selection. |
| 2. Interaction and history | At multiple zoom levels, drag/resize retains pointer alignment; pan/zoom does not alter document geometry; dimensions stay valid. Undo/redo restores creation, deletion, ordering, inspector edits and complete gestures. New edits invalidate redo. |
| 3. Local files and export | Save/open round trip preserves supported document values and ordering. Invalid/version-incompatible JSON leaves the current document intact. Write errors are visible and preserve prior files. Export text containing `<`, `&` and quotes safely; inspect the exported HTML in a browser. |
| 4. Release check for this slice | Rust formatting, compilation and focused tests pass; launch the egui app and complete one create→edit→undo→save→reopen→export session. Record supported platform, check results and rendering differences. Confirm icons are Phosphor and required license notices are present. |

Later planning may cover non-color tokens, nested layouts, image/vector layers, HTML/SVG imports and local MCP. Each requires separate behavior proof and acceptance checks. Full CSS layout, Paper file compatibility and multiplayer remain outside this slice.

The [ArtCraft interoperability plan](artcraft.md) defines proposed cross-application work separately from Paper parity. PaperCraft uses MIT, an option also offered by the native Craft apps; license alignment does not imply implemented adapters or official suite membership. New files use `.papercraft`; legacy `.crumbs` documents remain compatible with the unchanged version 1 schema.

## Reliability and SVG increment — 18 October 2026

Baseline: `bc3c52bf75491510a7d5a7515a5dce5c0bb12f1d`. Implemented on `codex/reliability-svg-exchange`.

- **Preservation:** close decisions run in `eframe::App::logic`, which runs while hidden/minimized. Dirty close cancels termination, restores/focuses the window and shows the existing Save/Discard/Cancel prompt. macOS installs `craft-quit`'s menu, pinned at `82a24b006eefa036b43c09a67fb9a3fbd17ffe63`, so Quit reaches that decision. The crate's compatible egui version and MIT option were checked before adoption; no other Craft architecture is imported.
- **Transactions:** release coordinates apply before the gesture commits. Shortcuts arriving with release run after that commit; Undo restores the complete gesture and Delete gets its own history step. Valid long Unicode layer names duplicate within the 256-byte UTF-8 limit.
- **Exchange:** choosing SVG flushes the current property transaction, serializes the authoritative Document and atomically replaces the destination only on success. Export does not clear the source dirty state or modify its history. The committed fixture covers overlap, transparency, radii, hidden layers, clipping and multilingual text; both installed target importers/renderers were exercised.
- **Compatibility/rollback:** no document fields, version, extension behavior or recent-file paths change. Existing `.crumbs` and `.papercraft` v1 files remain readable by the baseline implementation. SVG is derived output and can be discarded/regenerated; the source remains editable. Rolling back application code requires no document/data migration.

Acceptance: focused failing regressions first, then locked Rust tests, formatting, warnings-denied Clippy, optimized native build, actual native screenshots, model create→edit→undo/redo→save→reopen→HTML/SVG export, and target-app importer/model/render inspection. The [verification record](verification.md) states which input was simulated and which platform/file-dialog interactions remain untested. Work stops at this contract; hierarchy, images, richer typography and automation require later slices.

## Color tokens and arrangement increment — 18 October 2026

Baseline: uncommitted `codex/reliability-svg-exchange` work on `bc3c52b`, preserved unchanged and continued on `codex/theme-tokens-arrangement`. Evidence: [workflow evidence D2](../research/paper/workflow-evidence.md#color-tokens-and-arrangement-d2).

**Token contract (PaperCraft decisions unless cited).** Tokens have a stable `id`, a unique case-insensitive name (Unicode letters/digits, `-`, `_`; at most 64 bytes, so it is a valid CSS custom property suffix) and an RGBA value. Supported bindings are rectangle fill/text color, the artboard color and the page color. A bound property displays the token; its literal is kept synchronized for downgrade and detach. Detach keeps the current value (documented Paper behavior). Delete detaches every binding, including on locked layers, and keeps each color (Paper does not document deletion). Locked layers cannot be bound or detached directly. Renames validate before changing anything. Create, create-from-property (create + bind), rename, value edit, bind, detach and delete are each one history step; value and hex edits batch until the field or picker finishes. Canvas, inspector, Theme list, dashboard thumbnails, HTML and SVG resolve the color through one `Document` resolver; invalid references are rejected on commit and load.

**Arrangement contract (PaperCraft decisions).** The selection is an ordered set of layer IDs; the inspector edits properties only for a single selection. *Arrangeable* layers are selected, visible and unlocked. Shared drag, nudge, align and distribute act on those layers only. Align uses their union bounds; with exactly one arrangeable layer it uses the artboard. Distribution requires three, orders by leading edge (paint order breaks ties), keeps the first and last in place, and gives equal (possibly negative) gaps. Delete and duplicate act on unlocked selected layers, hidden included. Copies are appended in paint order and become the selection. Hide/lock toggles act on all selected layers. Undo/redo drops selection IDs that no longer exist. Resize remains single-layer.

Shared moves retain relative and fractional offsets, stopping the whole selection when a layer position reaches ±100,000. Align/distribute requests that would exceed the coordinate limit are rejected without changing the document or history.

**Persistence.** See [document format](document-format.md): the lowest representable version is written; an in-place save that would upgrade a version 1 file asks first and defaults to Save As; older readers reject version 2 without modifying it.

Acceptance and results: [verification](verification.md#color-tokens-and-arrangement-18-october-2026).

## Canvas and performance increment — 18 October 2026

Baseline: `1ecd3b8`. Evidence: Paper documents resizing and pixel snapping ([support](https://paper.design/docs/support), [workflow evidence](../research/paper/workflow-evidence.md)); its handles, marquee rules and snapping heuristics are undocumented. Every rule below is a PaperCraft decision.

**Marquee.** Dragging from a point with no unlocked visible layer (Select tool) draws a marquee. The selection becomes the visible, unlocked layers whose bounds intersect or touch it, updated live; with Shift held at the press, the earlier selection is kept and the marquee adds to it. Selection changes are not history.

**Resize.** Handles act on the arrangeable selection's bounds: one layer resizes alone, several scale together. Corners are hit within 4–12 screen points (smaller for small layers, so they stay movable); edges respond along their length when long enough. Moving edges advance by whole units and stop 1 unit from the opposite edge. Group resize maps each layer's bounds proportionally and rounds to whole units; text size is not scaled. Shift held after the press keeps a corner resize's starting proportions. Shift at the press keeps its existing meaning (extend the selection), so a Shift-press on a handle selects instead of resizing. One gesture is one history step.

**Snapping.** On by default; the tool-rail magnet toggles it for the session and ⌘/Ctrl held during a drag bypasses it. Targets are the left/center/right and top/middle/bottom lines of the artboard and every visible layer outside the gesture; hidden layers are excluded, locked layers are targets. A move snaps the nearest of the moving bounds' three lines per axis; a resize snaps each moving edge. The tolerance is 5 screen points, so snapping behaves the same at every zoom; a snapped value lands exactly on its target instead of being rounded. Guides span the snapped bounds and their targets.

**Performance.** Selection membership, gesture layer lookup and the unsaved-changes check are indexed (an `Editor` revision identifies each committed state, so undoing to the saved state is clean). The layer list builds only visible rows. Inspector and Theme panels snapshot only the fields they edit instead of cloning the document each frame. Selection outlines are one mesh. `examples/bench.rs` measures the result.

Acceptance and results: [verification](verification.md#canvas-and-performance-18-october-2026).
