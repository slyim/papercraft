# Verification record

18 October 2026. Host: Apple Silicon macOS. Rust 1.99.1; eframe/egui 0.36.2; egui-phosphor 0.14.0; REA 6.7.0.

Entries below retain the capability and verification state at each earlier change. The final **Reliability and SVG exchange** entry records the current increment; earlier statements that SVG was deferred describe the earlier code.

## Passed

- `cargo fmt --check` and `cargo clippy --locked --all-targets -- -D warnings`.
- `cargo test --locked`: three integration tests. Coverage includes creation, editing, duplication, ordering, deletion, undo/redo, save/open equality, escaped Unicode/text export, hidden-layer omission, locked-layer guards, invalid geometry/version/IDs, invalid JSON, and preservation of an existing file after a rejected save.
- Simulated egui input at 1380×860 and 960×640: rectangle shortcut, drawing output, canvas selection, drag and bottom-right resize at each fitted zoom. One undo reverts each complete gesture; redo restores it. Delete during an active gesture does not remove its target.
- `cargo run --locked --example verify_native`: an actual native OpenGL window rendered successfully; its public screenshot event produced visible content. The example also saved/reopened the starter `.crumbs` document and wrote its HTML export.
- Native screenshot inspected after correcting layer-row label/button clipping. `target/verification/crumbs-desktop.png` contains the final reviewed layout.
- Export opened through the available Chrome connection from a loopback server. The DOM reports a 960×640 artboard and zero script elements; no browser warning/error logs were recorded. The HTML request returned 200; the optional favicon request returned 404. There was no browser connection failure. `target/verification/crumbs-html.png` records its appearance. Browser and egui font metrics differ visibly.
- `npm test`: the REA integration check connects using the project configuration, verifies the pinned server version, and verifies scoped analysis and integrity-mismatch handling. A separate live MCP session check also returned version 6.7.0.
- Phosphor and egui-phosphor MIT notices retained in `licenses/`. Paper research remains outside application code.

Generated verification files are ignored by Git. Regenerate native evidence using the example above. Browser inspection is a separate check against its generated HTML file.

## Dashboard and editor chrome (18 October 2026)

Host: Linux aarch64 container, Rust 1.97.0, Xvfb with Mesa OpenGL. The macOS host was not used for this pass.

- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings` and `cargo test --locked` pass: 3 unit and 9 integration tests.
- New focused tests: recent-file ordering, deduplication, cap, persistence and rejection of invalid lists; relative edit labels; hexadecimal color parsing; page color compatibility with files that omit it and its HTML export; positioned layer creation (rounding, clamping, one history step); click and drag creation with the rectangle/text tools, Escape and outside-artboard starts; side-panel width stability over frames at 1380×860 and 960×640; dashboard search, opening a recent file through the unsaved-changes prompt (Cancel and Discard), and closing the document tab; page color hex editing as one undo step without deleting layers.
- Native screenshots through `examples/verify_native.rs` at 1470×922 and 960×640: editor, selected layer with row hover controls, active rectangle tool with rail hover, dashboard grid with card hover, list view with row hover, empty dashboard without a document tab. Compared with the supplied references for panel widths (240/180/40/280 pt), tab strip height (41 pt), colors and row metrics.
- Account, team, invitation, learning, sharing, MCP, vector, frame, component and image-generation controls in the references are absent: Crumbs has no backend or those model features. Theme and adding pages are shown disabled with explanations.

## Not verified

macOS title-bar integration (tab strip inset beside the window controls, window dragging from the strip; the window controls stay at the system position and are not vertically centered on the 41 pt strip), native file-dialog clicks, unsaved-change dialog choices, a complete manual editing session, screen-reader interaction, and every keyboard shortcut have not been exercised end to end. Model-level file safety and supported pointer gestures have automated coverage.

Windows and Intel macOS builds, Linux release builds, large-document performance, font coverage for every writing system, and exact browser typography are unverified. A prior Linux aarch64 development rendering check is recorded above.

Authenticated Paper editing and runtime parity remain unknown. Static evidence established the shipped shell and bridge, not the hosted editor internals. Nested layouts, images, SVG, multiple artboards, arbitrary CSS editing, Paper document compatibility, collaboration and a local MCP interface are deferred.

## First prerelease (18 October 2026)

Host: Apple Silicon macOS 27.0.1, Rust 1.99.1. Version: 0.1.0, marked as a GitHub prerelease.

- Formatting, all 3 unit and 9 integration tests, Clippy with warnings rejected, and the research tooling integration test pass. The research test now starts the pinned package directly and does not require a checkout-specific agent configuration.
- `cargo build --release --locked` succeeds. The executable is Mach-O arm64, links only system libraries/frameworks, and declares macOS 11.0 as its minimum target. Runtime compatibility was checked only on macOS 27.0.1.
- The native verification example rendered the editor at 1380×860 and dashboard at 960×640. Screenshots were inspected. Its save/reopen and HTML export assertions pass.
- `Crumbs.app` uses the original release executable and `packaging/macos/Info.plist`. Launch Services opened the packaged app. Ad-hoc signature validation passes; Apple Developer ID signing and notarization are not configured.
- The package includes notices for 137 normal/build dependency packages, bundled font/icon notices, and Rust standard-library notices. Package metadata and recovered upstream notice references accompany the license files. The application license was undecided at initial publication; the later license decision is recorded below.
- Local agent configuration, raw research, dependencies and build output are excluded from Git.

Native file-dialog interactions, a complete manual editing session, screen-reader interaction and macOS approval prompts on a separate downloaded installation remain unverified. Finder document associations and automatic updates are not configured.

## License and ecosystem alignment (18 October 2026)

Original Crumbs code and documentation, including original code in v0.1.0, are now offered under `MIT OR Apache-2.0`. Third-party notices retain their own terms. Crumbs remains an independent companion project; no official catalog inclusion is claimed.

Offline Cargo metadata, npm package/lockfile metadata, license texts and relative documentation links were checked. Application code, application dependencies and the existing release archive are unchanged. No additional application tests were run for this documentation and metadata change.

The [integration plan](artcraft.md) cites checked upstream revisions and separates documented interfaces from proposed implementations. No cross-app editing, rendering, automation or installation was tested; adapters remain unimplemented.

## PaperCraft rename (18 October 2026)

Version 0.1.1 renames the app and Rust package to PaperCraft/papercraft. The current source, documentation and original generated paper-crane logo use MIT; the previously published v0.1.0 dual-license grant is retained with that release. Historical entries above describe the names and licenses at the time of each check.

- Formatting, Clippy with warnings rejected, all 4 unit and 10 integration tests, the research integration test, and the optimized native build pass.
- The legacy-file regression loads the original v0.1.0 starter fixture, saves it as `.papercraft`, verifies document equality, and confirms that the original `.crumbs` bytes remain unchanged. The schema stays at version 1. Existing Save keeps its path; Save As defaults to `.papercraft`. Both extensions are accepted by Open.
- Recent-file storage deliberately retains the old Crumbs folders, avoiding a destructive migration or split history between application versions.
- The embedded PNG decodes at 256×256 for window/dashboard use. The original generated logo and standard macOS ICNS sizes are retained in `assets/`, with the final generation prompt and MIT terms documented there.
- Native editor and dashboard screenshots were inspected at 1380×860 and 960×640. The dashboard shows the new logo/name, and the starter artwork names PaperCraft. Save/reopen and HTML export assertions pass.
- Native rendering, ICNS packaging and the research test require macOS services unavailable in the sandbox. Their checks pass outside it; those sandbox errors are separate from application failures.
- The final `PaperCraft.app` bundle launches without diagnostics. Ad-hoc signature verification passes before and after ZIP extraction, and the extracted executable matches the staged app. The app includes the MIT license, generated ICNS icon and the retained 137 dependency notice sets. The archive's SHA-256 is published in `SHA256SUMS.txt`.

Native file-dialog clicks and a full manual editing session remain unverified. Other-platform release builds, notarization and Craft app interoperability remain outside this rename.

## Reliability and SVG exchange (18 October 2026)

Baseline `bc3c52bf75491510a7d5a7515a5dce5c0bb12f1d`; branch `codex/reliability-svg-exchange`. Host: Apple Silicon macOS, Rust 1.99.1, eframe/egui 0.36.2. The existing worktree was clean before the branch was created. No release, repository transfer or affiliation change was made.

### Defects established before correction

| Defect / cause | Focused proof and correction |
|---|---|
| Valid Unicode layer names could fail duplication: the old name truncation counted characters while document validation limits UTF-8 bytes. | `duplicating_a_valid_unicode_name_keeps_a_valid_name_and_one_history_step` failed with an 85-character/255-byte CJK name. Duplication now reserves the suffix's byte budget and truncates at a valid character boundary. |
| Drag/resize omitted motion delivered with pointer release: only `response.dragged()` applied geometry. | `dragging_and_resizing_include_the_release_position_at_multiple_zoom_levels` demonstrated a partial movement. Release now applies final coordinates before committing; move/resize undo/redo is checked at 1380×860 and 960×640 fitted zooms. |
| Release-frame shortcuts ran before the active gesture committed. | `release_frame_undo_runs_after_the_completed_gesture` failed before correction. Shortcuts on that frame now run after the gesture. A separate Delete regression verifies no stale-target panic and independent move/delete undo/redo steps. |
| Hidden/minimized close bypassed dirty checks: the decision ran only in UI, which eframe skips while hidden. Winit's default macOS Quit also sends `terminate:` directly. | A logic-only minimized close test failed before correction. `App::logic` now cancels dirty close, restores/focuses the window and invokes the existing prompt. macOS uses inspected/pinned `craft-quit` 0.1.0 at `82a24b006eefa036b43c09a67fb9a3fbd17ffe63`, with native Quit interception. The default-Quit cause is source verified; physical Cmd-Q input was not exercised. |

No additional malformed-file, recent-history or selection invalidation defect was demonstrated. Existing validation/preservation regressions continue to pass. Full-document cloning in the inspector and linear dirty comparisons remain potential large-document costs; no performance claim or unrelated refactor was made.

### Completed checks

- `cargo fmt --check` passes.
- `cargo test --locked` passes: 4 unit tests and 18 integration tests (10 existing, 6 reliability/UI regressions, 2 SVG regressions). Coverage retains legacy `.crumbs`/version-1 round trips, recents, rejected file preservation, invalid geometry/version/IDs, canvas gestures and property transactions. New SVG coverage checks visible order, dimensions, alpha, radius clamping, XML escaping, Unicode, clipping references, hidden omission, unchanged source/save/reopen and rejection before replacing an existing export.
- `cargo clippy --locked --all-targets -- -D warnings` passes after the final example changes.
- `cargo build --release --locked` passes; `target/release/papercraft` is Mach-O arm64. No package or release was published.
- Native example runs succeeded for `exchange` at 1380×860 and `export`/`quit` at 960×640. Their actual OpenGL renderer screenshots were inspected: `target/verification/exchange/papercraft-exchange-1380x860.png`, `target/verification/export/papercraft-export-960x640.png`, `target/verification/quit/papercraft-quit-960x640.png`. The example disables animations for stable captures only; application animation behavior is unchanged.
- The `export` scenario uses **simulated egui pointer events** to open the real native menu. The `quit` scenario uses **simulated menu/minimize dispatch** through the real native event loop, delegates to PaperCraft's `App::logic`, and confirms unsaved document state survives while the visible Save/Discard/Cancel prompt is rendered. It restores the saved state before exiting. This is not a physical keyboard/menu-click test.
- The example's separate **model preflight** exercises create → edit Unicode text → undo → redo → atomic save → reopen/equality → HTML and SVG export. These file operations are model calls, not native dialog clicks. Existing simulated egui tests separately cover creation, inspector edits, gestures and dashboard replacement prompts.
- Generated SVG parses as XML, has a 640×400 viewBox, four visible layers in order, two editable text elements, two clip paths, escaped names/text and no script elements. `starter.svg` SHA-256: `cad886453add242c799e46e5229d6bc00f96f092104460cf68642b84cb140071`.
- The available Chromium browser connection opened the generated SVG and HTML from an isolated loopback server. Both rendered and were visually inspected; Unicode, overlap/alpha and clipping were visible. The server returned 200 for both files. Browser access succeeded. This pass did not collect browser-console logs.
- Independent read-only diff review found no further actionable correctness/safety regression. `git diff --check` passes. License notice for adopted `craft-quit` is retained in `licenses/craft-libs-MIT.txt`; original MIT/crane branding remains intact.

### Actual cross-application check

The committed `tests/fixtures/svg-exchange.papercraft` was exported by PaperCraft and imported through the real installed applications' opt-in loopback control servers. These were **actual importer, document-model, native-window and renderer checks**, not replayed source tests or simulated pointer input.

- VectorCraft **0.13.1** and FigCraft **0.7.0** accepted the 640×400 SVG. Inspected model data and images preserve visible ordering, base/overlap geometry, radii 28/70, alpha ≈0.502, hidden-layer omission, multiline Unicode strings as editable text, and clipping of the long text box to 180×28.
- Native windows and 640×400 offscreen output were inspected: `target/verification/exchange/{vectorcraft,figcraft}-{window,import}.png`. Target font choices and metrics differ. FigCraft uses SVG IDs as layer names; VectorCraft retains the exported labels. CJK/Greek render in both target images, while native PaperCraft's bundled font shows placeholder glyphs for CJK.
- Exact installed executable hashes, source revisions, fixture hash, invocation/inspection methods and retained JSON/image hashes are in [the Craft runtime record](craft-source-review.md#runtime-boundary). A source Git revision is not asserted to identify the installed executable. Only disposable target documents/processes were used; both owned target PIDs exited.

### Evidence, compatibility and remaining limits

Paper 0.5.16/build 261015tx3mukqib still matches E1's ASAR SHA-256 `912bad3fe41e226c67fd4e27352e2b4456d69d19589f4583cba5944932fb78b2`; the recorded analysis was reused. Current official docs plus authenticated read-only connector checks support the [workflow evidence](../research/paper/workflow-evidence.md). Existing Paper content was untouched. No Paper editing/history/export runtime parity is claimed. The [gap assessment](gap-assessment.md) ranks the next bounded increments.

No document migration is required: `.crumbs` and `.papercraft` remain version 1; fields, IDs and recent-file paths are unchanged. Export is derived output and does not mark the source saved. The old application can still read source documents created by this increment; rollback requires no data conversion.

SVG keeps explicit text lines and clips overflow; automatic native/HTML wrapping, exact baselines/fonts and full glyph coverage are not reproduced. Root clipping of objects extending outside the artboard, target editing/resave, full cross-app round trips, PhotoCraft/DesignCraft runtime workflows, native file-dialog clicks, physical Cmd-Q, actual screen-reader use, large-document performance and other-platform builds remain unverified. Grouping/hierarchy, snapping/alignment, richer typography, image/vector layers, multiple artboards, layout and CLI/MCP are still gaps. The upstream Touch Bar teardown issue was not reproduced on this hardware.

**Environment failures:** sandboxed native rendering produced macOS Launch Services/XPC errors and stalled; the owned verification PID was stopped. The same native checks succeeded outside the sandbox. Loopback binding was denied inside the sandbox and succeeded outside it. Dependency fetching needed access to Cargo's cache outside the writable roots and then succeeded at the pinned revision. These are sandbox/service restrictions, not application test failures. Research tooling was unchanged; `npm test` was not rerun for this increment.
