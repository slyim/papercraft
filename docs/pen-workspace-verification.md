# Editable Frames and authored Pen — verification

29 October 2026 UTC. This implements the bounded A/B/C scope in [the handoff](pen-workspace-handoff.md), after the preserved selection/navigation checkpoint `9a5d705`. It is an original Rust/egui implementation, not an Illustrator/Figma/Paper parity claim. No proprietary source/assets or embedded provider runner are added; roadmap stages 2–9 are not collectively complete.

## Delivered behavior

- Startup's **A place to begin** and GUI New's blank Frame are ordinary unlocked `NodeKind::Frame` layers. The sample's existing layers become children; the legacy backing is transparent, with no stationary duplicate caption. Frame labels are selectable and the virtualized layer list presents parents before indented children.
- Shared `Document::{mutation_roots,translation_ids,translate_nodes,set_geometry,delete_nodes}` support coherent subtree movement, inspector/MCP geometry, keyboard nudging, arrangement, duplicate/delete. An unlocked ancestor carries hidden/locally locked descendants once. Resizing changes the Frame container/clip, not child scale/reflow. Mixed ancestor/child resizing rejects. Absolute world coordinates and legacy default export crops are unchanged; Frame/selection PNG uses the selected world crop.
- `src/app/pen.rs` owns transient multi-click drafts and point previews: click corners, drag symmetric handles, Enter/double-click finish open, click first anchor close, Backspace remove a draft anchor, Escape/focus loss cancel. A finish is one transaction. Paths support point selection/Shift addition, movement/nudge, segment insertion, point deletion, closed/open inspector editing, Corner/Smooth/Symmetric modes and independent Alt-dragged handles. Snapping alone cannot synthesize handles.
- `src/vector.rs` provides canonical normalized anchors, world-preserving rebasing, cubic extrema, de Casteljau splitting, bounded adaptive subdivision and real SVG `C` output. Native/PNG rendering and picking share contour geometry; even-odd fills use `lyon_tessellation` and match concave/self-intersecting picking. Editable source is never flattened on save.
- [v4](format-v4.md) adds anchors without extending frozen v3. Existing v1–v3 fixtures, lowest-version writing, strict/default-key rejection, explicit upgrade consent, atomic destinations and rollback remain covered. MCP accepts/inspects canonical anchors and rejects ambiguous geometry inputs atomically. Live document tools stay busy through a drawing draft or point-edit mode.

## Automated gates — PC-PEN-01

All pass on macOS arm64, Rust 1.99.1, package version 0.3.0 (no new release/tag):

```sh
cargo fmt --check
cargo test --locked                         # 165 passed; zero failed/ignored
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release --bins --examples
git diff --check
```

Logs: `/tmp/papercraft-pen-verified-{tests,clippy,build}.log`.

New regressions: `tests/frames.rs` (5), `tests/pen.rs` (9), `tests/vector.rs` (6). They cover subtree movement/locked children/duplicate/delete/history, GUI New, pointer versus keyboard/MCP translation, authored draft isolation/cancel/redo/focus loss, anchor preview/move/insert/delete/undo, multiple zooms, two-anchor curve closure, snapped corner placement, draft nudges isolated from the previously selected Frame, inspector Smooth conversion exposing usable endpoint handles, cubic extrema/splitting/zero spans, modes, v4 serialization/default-field rejection, atomic malformed batches and upgrade consent, zero stroke and concave pixels/picking. Existing selection expectations now include even-odd interiors; text-tab fixtures account for the real starter Frame rather than assuming text is always node/ID 1.

Regressions first reproduced accidental handles from snapping (`/tmp/papercraft-pen-snap-red.log`) and mixed curve/line vertex-budget bypass (`/tmp/papercraft-pen-budget-red.log`); both are fixed. A further regression (`/tmp/papercraft-pen-draft-nudge-red.log`) proves draft arrow keys cannot move the previously selected Frame. The inspector endpoint regression (`/tmp/papercraft-pen-endpoint-red.log`) also reproduced collapsed Smooth handles on a two-anchor straight line; endpoint tangent initialization is fixed. The vertex fix enforces the same 32,768-vertex cap across curved and straight segments.

## Real native event loop — PC-PEN-02

```sh
# Repeat at both sizes:
target/release/examples/verify_native target/verification/pen-v4/final/<scenario>-<size> <scenario> <size>
# scenarios: starter-frame, pen-v4, pen-cancel
# sizes: 1380x860, 960x680

target/release/examples/verify_live target/verification/pen-v4/final/live
```

All six new native runs passed, including a rerun after the draft-nudge fix. After the final MCP ambiguity guard (rejecting simultaneous `points`/`svg_path`), another native repeat could not start: macOS reported `CGSSessionScreenIsLocked=Yes`, focus stayed false and the script remained at frame 0. OS activation did not bypass the lock; no focused-input flag or test assertion was weakened. Logs: `target/verification/pen-v4/focus-failure/` and the last starter-frame `run.log`. Earlier native captures/proofs retain their original source scope. The final MCP guard and subsequent Smooth-endpoint fix have automated regression coverage in the 165-test gates; they were not freshly exercised in an unlocked native window. A final visible installed interaction still requires an unlocked session. Native screenshots were opened and inspected: moving the starter Frame carries its hidden/locally locked children, the title/selection follow it, and Pen anchors plus two independent handles are visible beside the point-mode controls. The harness checks document/history postconditions and save/reopen/PNG/SVG/HTML artifacts, not just capture existence.

Native regression runs at 960×680 also pass for `selection-1b`, `marquee-1b`, `creation-1b`, `navigation-1b`, `text-tabs` and `quit`. The two-tab capture was inspected; text and tab-local selection remain intact. The Quit check uses simulated menu/minimize dispatch and preserves unsaved work.

The real external stdio → authenticated IPC → GUI live check rejects an edit **between Pen clicks, with no pointer held**, cancels the draft without a revision change, creates two world-space Frames plus a cubic path, moves/undoes a negative-world Frame and its curve, rejects stale/invalid batches, renders both bounded Frame PNGs, saves/reopens v4 and exercises shared undo/redo. The fitted native capture was inspected. Its Frames explicitly disable clipping for supported SVG/HTML; clipped vector exports still reject with a PNG alternative.

These are scripted egui events in an actual native window, not physical keyboard/mouse or native file-dialog automation. Focus is requested from the OS, never fabricated.

## Browser exports — PC-PEN-03

Headless installed Chromium rendered the actual `interaction.svg` and `interaction.html` from the native Pen run, using isolated temporary profiles and no application server. Both screenshots were opened: the cubic contour agrees visually with the native outline; no polyline substitution is used in the source exports. Logs/captures are under `target/verification/pen-v4/browser/`. Chromium emitted allocator/task-policy diagnostics but returned success and produced both expected captures; this is not a console-cleanliness claim.

Selected SHA-256 evidence (paths relative to `target/verification/pen-v4/`):

| Artifact | SHA-256 |
|---|---|
| `final/pen-v4-1380x860/interaction.papercraft` | `2033d69ede1ffa44f5db2a4c3870eca313e9ddef7c58d89eaf7fb80af4b70699` |
| `final/pen-v4-1380x860/interaction.svg` | `366c99aeaaa960bdfb3994e39a1a967797b33288ac14baf39f00bbd134d403be` |
| `final/pen-v4-1380x860/interaction.html` | `9e5ef5ad0cdddda9a0b2a818967cba4fb757ab2e2b4358db8be334e67bc1b479` |
| `final/pen-v4-1380x860/papercraft-pen-v4-1380x860.png` | `39870e0d7ed4c2f1728035757820f013c71368fcd9e321ce86af16b8f893c7db` |
| `browser/curve-svg.png` | `1d404966e6bd2cbebc681336cdb2e41b32fc677d4799e719a95fd7f323eb190e` |
| `browser/curve-html.png` | `0e60d4ddff23f47943f1ffb3c8bde9db3ea367b8a9500486af056dc329f4285a` |

## Delivery and boundaries

Publication/installation are separately gated: reviewed normal push to current `main`, inspect exact-commit CI, package the tested executable with dependency notices, verify signatures, stage a complete bundle, retain a dated previous-app backup, compare installed/staged bytes and smoke-test the installed bundle. Machine-specific commit/checksum/backup and installed-smoke results belong in the ignored `target/verification/pen-v4/delivery.json` ledger. Do not infer installed provenance from package version alone.

Not claimed: full hierarchy/reparent/unwrap/groups, local transforms, auto layout/pages, vector networks/booleans/multiple subpaths, arbitrary SVG/curve import, exact stroke joins/antialias/glyph picking, proprietary native-file compatibility, deep-hierarchy performance certification, physical-input/IME/accessibility/file-dialog coverage, or Windows/Linux runtime proof from this Mac. External CI status must be read from the published commit, not inferred from local gates. Ad-hoc signing is not Developer ID signing/notarization. Original v1–v3 files remain the rollback source; do not silently downgrade editable curves.
