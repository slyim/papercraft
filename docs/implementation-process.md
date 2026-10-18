# PaperCraft implementation process

Date: 18 October 2026, Europe/Istanbul.

Build an original, local design editor in Rust with egui. Paper research informs behavior; it does not supply application code. The first slice is one local artboard, rectangle/text layers, selection and ordering, an inspector, drag/resize, pan/zoom, undo/redo, `.papercraft` JSON save/open (also accepting legacy `.crumbs` files), and HTML export. This document defines acceptance checks, not a claim that they have passed.

The first slice is implemented. The [verification record](verification.md) lists completed checks and remaining manual/platform checks separately. The milestones below remain the acceptance checklist for further work.

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
| Save/open | E1 `src/lib/storage.ts:12–24,33–47,76–126` proves shell JSON storage only | Versioned `.crumbs` JSON stores the original document. Validate before replacing the open document; failed writes preserve existing files. Paper document compatibility is unknown and out of scope. |
| HTML export | Code export [documented](https://paper.design/docs/support) | Export supported primitives, geometry, colors and escaped text as a standalone file. Browser typography may differ from egui; arbitrary CSS and import round trips deferred. |
| Dashboard and editor chrome | User-supplied Paper screenshots (dashboard, empty editor), 18 October 2026; runtime behavior behind account/team/MCP controls unknown | PaperCraft decisions: a Dashboard tab and one document tab; recents store local paths only and tolerate missing or invalid lists; opening, creating or closing via tabs/cards uses the unsaved-changes prompt; V/H/R/T tools place a layer by click or drag inside the artboard, as one history step, then return to Select; the page color is stored in the document (`canvas`, optional for earlier version 1 files) and exported as the HTML body background. Theme tokens and multiple pages are shown disabled; account, sharing, MCP, vector, frame, component and image-generation controls are omitted. |
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

Later planning may cover tokens, nested layouts, images/SVG, imports and local MCP. Each requires separate behavior proof and acceptance checks. Full CSS layout, Paper file compatibility and multiplayer remain outside this slice.

The [ArtCraft interoperability plan](artcraft.md) defines proposed cross-application work separately from Paper parity. PaperCraft uses MIT, an option also offered by the native Craft apps; license alignment does not imply implemented adapters or official suite membership. New files use `.papercraft`; legacy `.crumbs` documents remain compatible with the unchanged version 1 schema.
