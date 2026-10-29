# Original handoff: editable starter Frame and authored Pen

**Current source status:** the bounded update is implemented; see [verification](pen-workspace-verification.md) and [v4](format-v4.md). Findings and the section 3 prompt below are the original pre-implementation record, not a request to redo completed work. Publication/install have separate exact-commit gates.

**Status at planning time: proposed behavior and implementation handoff, not implemented fixes.** The user prioritizes a familiar Illustrator/Figma-style Pen, an editable first Frame in an infinite workspace, Graphify, then a verified push to main and local install. The existing Rust/egui editor and visual direction remain authoritative. These are PaperCraft design decisions, not newly verified Paper/Illustrator/Figma parity claims.

## 1. Findings and evidence

The planning checkout is `main` at `97e833e51e6d2056eeb7691f17fd5cfcfb008d03`, with substantial **pre-existing uncommitted slice 1b** changes. Preserve those changes, including `src/geometry.rs` and `tests/selection.rs`. Do not start from a clean remote checkout and silently omit them. The current [editor roadmap](editor-completion-plan.md) outranks historical toolbar claims that Pen/hierarchy are already complete. This request changes priority: the bounded starter/frame fix comes first, then authored Pen; it does not authorize the rest of roadmap stages 2–9.

### Observed in source / checks

| Finding | Source / proof at planning time |
|---|---|
| Startup loads `Document::example()`. "A place to begin" is the **document name**, not a Frame node. All six starter layers are unlocked, flat rectangle/text nodes. | `src/app.rs:532–537`; `src/document.rs:581–686` |
| The large outer surface is document-level width/height/background. Its caption is painted text, and it has no selectable node ID. The inspector can edit its name, dimensions and background when no layer is selected, but it cannot be moved/resized as a normal layer. | `src/document.rs:528–556`; `src/app.rs:3040–3079,3801–3813`; `src/render.rs:210–227` |
| Ordinary GUI New creates an empty legacy document, not a blank starter Frame. | `src/app.rs:1119` |
| World-space creation and painting, negative positions, tab-local pan/zoom, fit content/selection/artboard, draft cancellation and screen-space selection tolerance already exist in this checkout. | `src/app.rs:3708–4050`; `src/render.rs:210–244`; `tests/workspace.rs`; `tests/selection.rs` |
| Frame mutation is incomplete: pointer drag updates only the supplied IDs; keyboard nudge separately expands descendants and skips directly locked children. Inspector and MCP field updates have further paths. Creating a starter Frame alone would expose inconsistent behavior. | `src/app.rs:3629–3705`; `src/document.rs:2331–2349`; `src/app.rs:2590–2620`; `src/mcp.rs:737–812` |
| `P` maps to `Tool::Pen`, currently honestly labeled **Path preset**. It follows generic rectangular placement and inserts fixed normalized zig-zag vertices, not pointer-authored anchors. | `src/app.rs:86–146,3959–4085`; `src/document.rs:291–293,2161–2227` |
| Persisted v3 paths are normalized straight vertices. The parser now rejects relative, curve and multiple-contour SVG commands. Closed native fills are convex-only; point/handle editing is missing. | `src/document.rs:247–253,297–369`; `src/geometry.rs:77–130,221–258`; `docs/format-v3.md` |
| Baseline regression suite passes: **145 tests, zero failed/ignored**, macOS arm64, Rust 1.99.1. | `cargo test --locked`; `/tmp/papercraft-pen-plan-baseline.log` |

The existing 1b native selection capture was opened during planning: `target/verification/selection1b/selection-1b/papercraft-selection-1b-1380x860.png`. It illustrates the current shell and off-artboard painting; it is **previous evidence**, not a new interactive reproduction of the reported starter issue.

Installed `/Applications/PaperCraft.app/Contents/MacOS/papercraft` SHA-256 at planning: `3d85985ba82785c51f37ccb12314fe0ab21e2a3bddfe049d849873d1e42dabf2`. Current `target/release/papercraft`: `ebc44a1690164425a97c218d55b3fece85f22f9568e2a766d93cb310af2496d7`. Different bytes do not alone establish stale source (signing/build settings can differ). Verify the actual running bundle and its provenance before diagnosing an installed-app regression.

### Inferences

- The reported uneditable first "frame" is chiefly a model/interaction mismatch: a legacy artboard looks like a Frame but is not one. Renaming its caption or adding an unlock button will not solve that.
- Infinite-canvas behavior should be verified and preserved, not rebuilt. A different installed build or incomplete frame semantics may explain the perceived canvas limit; neither is established as a runtime cause yet.
- Familiar Pen behavior requires a multi-click draft and point-edit state, not another preset or a better SVG text field. Existing `Document` / `Editor`, geometry, snapping, history and live-MCP seams should be extended.

### Unknown / needs explicit verification

No new physical-input reproduction, installed-build provenance, Illustrator/Figma runtime comparison, or all-platform interaction proof was performed in this planning pass. Exact modifier parity is unverified. Public/source research must distinguish observations, inferences and unknowns; use `docs/implementation-process.md` and the project REA skill if new Paper artifact analysis is needed. Do not adopt proprietary code or assets.

## 2. Recommended bounded delivery

### A — Make the starter a real Frame; retain the world canvas

1. Startup's sample has one ordinary, unlocked top-level `NodeKind::Frame` named **A place to begin**, at `(0,0)`, initially `960 × 640`, containing the existing sample layers. A normal GUI New starts with one blank editable Frame. Keep `Document::default()` and the existing explicit MCP empty-document contract usable; add a narrowly named starter/template constructor rather than seeding every internal empty document.
2. Reuse stable IDs and existing parent links. In v1–v3, all node x/y remain **absolute world coordinates**, including children. Do not silently migrate old files or reinterpret them as parent-local coordinates.
3. Select a Frame through its layer row, visible border or following name label; text double-click and normal child selection must still work. The layer row exposes its children coherently. Renaming the Frame must not silently rename the document. X/Y/W/H, fill, clip, hide/lock, duplicate, delete and undo must actually operate on it.
4. Use one shared subtree-translation rule for pointer movement, arrow nudge, inspector X/Y, arrangement and MCP frame-position updates. Include hidden and locally locked descendants when an unlocked ancestor moves; their local locks prevent direct editing, not following their container. A locked ancestor prevents direct descendant geometry edits. Deduplicate a selected ancestor + descendant so the child moves once. Clamp/reject the entire move as a unit; no partial subtree move. Exclude the whole moving subtree from snap targets.
5. Frame resize changes the container rectangle/clip only; it does **not** scale/reflow children in this increment. Position changes still translate descendants. Keep group-resize rules explicit when both a Frame and its children are selected; reject an unsupported combination rather than double-transforming it. Audit duplicate/delete/parent validation so the starter cannot leave dangling references or duplicate only its empty shell.
6. The legacy artboard is a compatibility/default-export region, not an authoring boundary. For **new starter documents**, prefer a transparent legacy backing with the real Frame providing the visible surface. Frame labels follow Frame geometry; do not leave a second white rectangle or misleading stationary title after moving the starter. Existing opened documents retain their background, dimensions and geometry. Do not identify a starter by a magic name or coordinate heuristic.
7. Preserve off-artboard drawing/selection, Space/Hand/middle pan, pointer-anchored zoom and Shift+1/2/0 fit behavior. The workspace is practically infinite, within the existing finite numeric/resource limits—not unlimited floating-point geometry. Moving or deleting a starter must not leave an inaccessible fixed white page behind.
8. Preserve the documented artboard-bounded default HTML/SVG export contract. Explain its crop in the UI; offer the existing bounded Frame PNG rendering path so a moved Frame can be exported intentionally. Do not silently change old-file export regions. If a persisted default-Frame reference/workspace mode is genuinely needed, document it in the same new-format contract as Pen, not as a heuristic or an extension of v3.

**A gate:** select/rename/edit the starter and its text; move via pointer/nudge/inspector/MCP; resize without child scaling; duplicate/delete/undo a subtree; draw a second Frame at negative coordinates; pan/zoom/fit both; save/reopen; export an intentional Frame crop. Test hidden/locked descendants, nested frames, mixed ancestor/descendant selection, limits and failed batches. No full local-transform rewrite, pages, auto layout or general-purpose scene framework.

### B — Authored straight paths, then point editing

- Replace generic preset insertion with an explicit Pen draft. `P` selects Pen; click places a corner anchor, subsequent clicks extend a rubber-band preview. Click the first anchor (screen-space close tolerance) closes a contour with at least three distinct anchors. Enter or an end double-click finishes an open path with at least two anchors; double-click must not insert an accidental duplicate. Escape discards an uncommitted contour. A one-anchor draft never creates a saved node/history entry.
- Keep the whole draft outside authoritative `Editor.document`; finish commits one validated creation/history step. Backspace or Undo during drawing removes the last draft anchor without changing committed history/redo. Tool change explicitly finishes a valid draft or discards an invalid one; tab switch/focus loss discards uncommitted drafting and consumes a held release. Saving/closing while drafting must resolve the draft visibly, never silently save an incomplete shape.
- Between anchors, Space/Hand pan and anchored zoom may change the view without moving stored world anchors. Freeze navigation while an anchor/handle press owns a drag. Draft parent selection is deliberate and validated; no accidental reparenting as the pointer crosses a Frame.
- Enter/double-click on a selected Path enters point edit mode. Distinct point markers and a short status hint make editing visible. Select/Shift-select anchors, drag/nudge them, insert on a segment, delete points and toggle open/closed. Define endpoint and below-two-point deletion behavior before coding. A point drag/nudge or topology action is one shared undo step; Escape cancels an active drag, otherwise exits edit mode without undoing completed edits.
- Straight paths can reuse the released `points` representation. Preserve exact world anchor locations when recomputing bounds/normalization, including horizontal/vertical paths with a one-unit minimum box. Do not round all anchor coordinates through rectangle placement.
- Correct non-convex closed fills as part of usable authored contours. Use one documented fill rule (recommend even-odd) consistently in canvas, PNG, picking and SVG/HTML. Never feed concave geometry into egui's convex-fill API. Existing epaint `PathShape` documents convex fills only; evaluate a small maintained Rust tessellator such as `lyon_tessellation` instead of writing a custom general tessellator. Add a dependency only with a license/size/resource review and locked tests.

**B gate:** author arbitrary open/closed contours rather than the zig-zag, edit their points, undo/redo, save/reopen and export; test concave fill/hits, transparent fill/stroke-only selection, offboard/nested clips, cancellation/redo and minimum/maximum geometry. Do not label a straight-only interim milestone as completed Bézier Pen.

### C — Cubic Bézier handles: the major Pen completion gate

- Drag while placing an anchor creates opposing symmetric handles. In edit mode, drag anchors or individual handles; support corner, smooth (collinear, independent lengths) and symmetric modes. Alt/Option breaks a handle pair or converts a point; Shift constrains segment/handle angles to 45° increments. Expose the same point-mode choices in the inspector so modifier-only gestures are not the sole access path.
- Insert on a cubic segment by de Casteljau splitting so insertion does not change the curve. Close/finish/edit gestures use stable screen-space hit targets independent of zoom. Do not reuse rectangle edge/center snapping in a way that distorts unrelated handles; state which anchor snapping rules are supported.
- Define **v4 or later** before adding persisted handles or point modes. v3 is frozen. Recommended direction: optional canonical Bézier anchor records (anchor, incoming/outgoing handles, mode), mutually exclusive with nonempty legacy `points`, reusing the existing closure flag. Specify coordinate space, handle limits, defaults and zero-span bounds; handles may extend outside an anchor box. Do not store an editable `d` string or duplicate authoritative straight/curve representations. Exact wire fields remain the implementation owner's documented decision.
- Load old v1/v2/v3 files unchanged. Convert a legacy path to the new representation only on an edit that needs it; preserve its outline and stroke. Writer still emits the lowest representable version. Never silently downgrade curves to straight lines, flatten editable source, extend frozen v3, or overwrite an older file without existing upgrade consent/Save As.
- One canonical geometry path serves native painting, PNG, fill/stroke picking, marquee, bounds and exports. Use adaptive bounded flattening for raster/picking and analytic cubic extrema for accurate bounds; define error, subdivision/vertex budgets and failure behavior. SVG/HTML serialize actual cubic `C` segments, not the raster approximation. Keep legacy fallback strokes backward compatible; do not impose that fallback on a new path explicitly styled without a stroke.
- MCP creates/updates/inspects the same canonical anchors/handles and accepts them through atomic batches and live shared history. Publish schemas and capability limits together. Preserve stale-revision checks, typed batch-local refs, failed-batch rollback (selection/revisions/history/redo), GUI-thread authority and busy rejection throughout a multi-click draft—not just while the pointer is held.
- SVG curve **output** is required. General SVG import is not: retain explicit rejection of unsupported input rather than silently flattening it. Multiple subpaths/holes, Figma vector networks, booleans, self-intersection topology editing, pressure/brushes, arbitrary SVG import, variable stroke widths, rotation, images and provider execution are out of scope. Self-intersecting single contours must either obey the chosen bounded fill rule or reject explicitly, never render silently wrong.

**C gate:** a native click–drag–click–close curve workflow, point/handle editing, independent/mirrored handles, shape-preserving split, exact save/reopen, shared GUI/headless/live state and genuine SVG/HTML curves. Test both small and large zooms, zero spans, extreme/invalid controls, concave/self-intersecting policy, bounded tessellation, clipping, tokens/strokes, cancel/focus/tab changes and one-step history. Freeze the v4 contract only when this gate is verified.

## 3. Ready-to-paste agent prompt

```text
Implement PaperCraft's next update using docs/pen-workspace-handoff.md.
This is implementation work, not another planning-only response.

Read AGENTS.md, PRODUCT.md, DESIGN.md, docs/implementation-process.md,
docs/editor-completion-plan.md, docs/document-format.md, docs/format-v3.md,
docs/code-graph.md, docs/releasing.md and this handoff. Read the relevant Rust,
regressions and native/live examples before changing behavior. Historical claims
that the Pen preset or full frame hierarchy are complete are not acceptance proof.

The planning checkout is /Users/slimy/Desktop/papercraft on main at 97e833e with
substantial uncommitted 1b work. Start with git status/diff/log and inspect both
tracked and untracked files. Preserve that baseline, especially src/geometry.rs,
tests/selection.rs, the existing docs and verification harness changes. Retain a
recovery checkpoint including untracked source before restructuring. Do not reset,
clean, overwrite another agent's work, or branch from origin/main while omitting
pending changes. Do not blindly stage everything; separate unrelated work and
resolve uncertain ownership before publishing.

Use Graphify first for cross-file localization, confirm in source, and refresh it
using the documented key-free code-only commands. Raw research, credentials,
assets and generated output stay excluded. Graph links are not runtime proof.

Implement A, then B, then C from the handoff as independently tested milestones:
1. Real editable starter Frame and coherent subtree movement in the existing
   finite world workspace; no duplicate stationary artboard; old files preserved.
2. Authored straight-path draft and point editing, with correct bounded closed fill.
3. Cubic Bezier placement/handles/editing and a coordinated v4+ persistence contract.
Do not stop at a new icon, preset, SVG text box or cosmetic unlock control.
The priority override is bounded; do not implement the remaining general editor
roadmap, pages, auto layout, vector networks, providers or a renderer rewrite.

All application code is original Rust/egui/eframe with egui-phosphor. Reuse the
Document/Editor, shared geometry/rendering, snapping, history, strict validation,
atomic file writes and live MCP architecture. If UI gesture code needs a dedicated
Pen module, extract only that coherent responsibility; no speculative scene engine.
One owner coordinates schema work. Write the exact v4 contract and migration/
rollback/export rules before serialized edits. Do not change v3 coordinate meaning,
extend v3 again, silently upgrade files or flatten editable curves on save.

Write failing behavioral regressions before fixes. Verify startup AND GUI New,
frame subtree pointer/nudge/inspector/MCP agreement, ancestor/descendant selection,
hide/lock/clipping, duplicate/delete, undo/redo and offboard navigation. Pen proof
must exercise real multi-click/drag input, not only constructing a Node in a test.
Draft/point/handle edits must preserve document, redo and revisions when cancelled
or rejected. Multi-click drafts must block conflicting live mutations. Keep native
focus failures visible; never forge focus or filter physical input to pass a check.

Run cargo fmt --check, cargo test --locked,
cargo clippy --locked --all-targets -- -D warnings,
cargo build --locked --release --bins --examples, and git diff --check.
Extend examples/verify_native.rs and examples/verify_live.rs for starter-frame and
Pen workflows. Exercise 1380x860 and 960x680 windows, several zooms, a second
negative-world Frame, point/handle edits, cancel, undo/redo, save/reopen and exports.
Open actual native captures, frame/region PNGs and vector exports. Verify SVG/HTML
curves in a real renderer as well as their serialized commands. Keep all previous
compatibility, destination-preservation, atomicity and resource-budget tests.
Report actual test counts, logs/hashes, failures and checks/platforms not performed.

After A/B/C pass, review the complete diff, update capability/format/verification
docs, refresh Graphify and commit the coherent milestones with Conventional Commits.
Fetch origin; integrate with current main without dropping remote or pending local
work. Re-run gates if integration changes the source. Push the tested main normally
(no force push, no history rewrite, no --no-verify). Confirm origin/main resolves to
the tested commit and check the available main CI result. Do not create a release,
tag, or replace published release assets as part of this request.

Then build/package that exact main commit for this local Apple Silicon Mac using
existing packaging/package.py and docs/releasing.md. The current native target is
aarch64-apple-darwin; the existing command is:
  python3 packaging/package.py macos-arm64 aarch64-apple-darwin
Use the resulting verified PaperCraft.app bundle, not a loose executable copied
inside an old signed bundle. Confirm the package version/build metadata and keep
the tested commit and staged-binary checksum in the install record.

Install locally at /Applications/PaperCraft.app only after checking that no running
instance or unsaved work will be disrupted. Keep a dated recoverable backup of the
old bundle. Stage the complete candidate beside the destination, verify its ad-hoc
signature, replace safely, and restore the backup on failure. Do not use blanket
process kills, delete user files/preferences, or bypass permissions/Gatekeeper.
If access needs permission, stop at that specific install blocker.
Verify codesign --verify --deep --strict before/after installation; compare the
installed executable to the staged signed executable (signing may alter raw build
bytes). Launch the installed bundle, not cargo run, and complete starter editing,
offboard drawing, Pen curve creation/editing and save/reopen/export smoke checks.
Preserve user's files and leave backup/rollback instructions.

Stop after this verified update. If a milestone, CI, push or install is blocked,
report the exact blocker and last verified state; do not claim success or publish
an incomplete advertised Pen update. Final report: changes, format/compatibility,
actual verification, commit and remote status, installed bundle path/checksum,
backup path and remaining limitations.
```

## 4. Planning-pass completion

Done in this pass: source diagnosis, a passing 145-test baseline, local Graphify setup with privacy exclusions and health caveats, and this proposed handoff. No application fix, new format, native interaction certification, commit, push, release or installed-app replacement was performed. The shipping/install actions above are expressly gated on the implementation agent's fixes and proof.
